mod config;
mod bundle_map;
mod layout;
mod error;
pub mod bundledb;
pub mod index;
pub mod chunking;
mod integrity;
mod basic_io;
mod info;
mod vacuum;

use prelude::*;

use std::mem;
use std::cmp::max;
use std::path::Path;
use std::fs::{self, File};
use std::sync::Arc;
use std::os::unix::fs::symlink;
use std::io::Write;

pub use self::error::RepositoryError;
pub use self::config::Config;
pub use self::layout::ChunkRepositoryLayout;
use self::bundle_map::BundleMap;
pub use self::integrity::{IntegrityError, ModuleIntegrityReport};
pub use self::info::{BundleAnalysis, RepositoryInfo, RepositoryStatistics};

const REPOSITORY_README: &[u8] = include_bytes!("../../docs/repository_readme.md");

const INDEX_MAGIC: [u8; 7] = *b"zvault\x02";
const INDEX_VERSION: u8 = 1;


#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Location {
    pub bundle: u32,
    pub chunk: u32
}
impl Location {
    pub fn new(bundle: u32, chunk: u32) -> Self {
        Location {
            bundle,
            chunk
        }
    }
}

impl index::Value for Location {}

impl index::Key for Hash {
    fn hash(&self) -> u64 {
        self.low
    }

    fn is_used(&self) -> bool {
        self.low != 0 || self.high != 0
    }

    fn clear(&mut self) {
        self.low = 0;
        self.high = 0;
    }
}


pub struct Repository {
    layout: Arc<ChunkRepositoryLayout>,
    config: Config,
    index: Index<Hash, Location>,
    crypto: Arc<Crypto>,
    bundle_map: BundleMap,
    next_data_bundle: u32,
    next_meta_bundle: u32,
    bundles: BundleDb,
    data_bundle: Option<BundleWriter>,
    meta_bundle: Option<BundleWriter>,
    chunker: Box<Chunker>,
    remote_locks: LockFolder,
    local_locks: LockFolder,
}


impl Repository {
    pub fn create<R: AsRef<Path>>(
        layout: Arc<ChunkRepositoryLayout>,
        config: &Config,
        crypto: Arc<Crypto>,
        remote: R,
    ) -> Result<Self, RepositoryError> {
        try!(fs::create_dir(layout.local_locks_path()));
        try!(symlink(remote, layout.remote_path()));
        try!(File::create(layout.remote_readme_path()).and_then(
            |mut f| {
                f.write_all(REPOSITORY_README)
            }
        ));
        try!(fs::create_dir_all(layout.remote_locks_path()));
        let mock_lock = Lock;
        try!(config.save(layout.config_path(), &mock_lock));
        try!(BundleDb::create(layout.clone()));
        try!(Index::<Hash, Location>::create(
            layout.index_path(),
            &INDEX_MAGIC,
            INDEX_VERSION
        ));
        try!(BundleMap::create().save(layout.bundle_map_path(), &mock_lock));
        Self::open(layout, crypto, true)
    }

    #[allow(unknown_lints, useless_let_if_seq)]
    pub fn open(layout: Arc<ChunkRepositoryLayout>, crypto: Arc<Crypto>, read_only: bool) -> Result<Self, RepositoryError> {
        if !layout.remote_exists() {
            return Err(RepositoryError::NoRemote);
        }
        let config = try!(Config::load(layout.config_path()));
        let remote_locks = LockFolder::new(layout.remote_locks_path());
        try!(fs::create_dir_all(layout.local_locks_path())); // Added after v0.1.0
        let local_locks = LockFolder::new(layout.local_locks_path());
        let _lock = try!(local_locks.lock(false));
        let mock_lock = Lock;
        let bundles = try!(BundleDb::open(layout.clone(), crypto.clone(), &mock_lock));
        let mut rebuild_index = false;
        //FIXME: why is this never set?
        let /*mut*/ rebuild_bundle_map = false;
        let index = match unsafe { Index::open(layout.index_path(), &INDEX_MAGIC, INDEX_VERSION) } {
            Ok(index) => index,
            Err(err) => {
                tr_error!("Failed to load local index:\n\tcaused by: {}", err);
                if read_only {
                    return Err(err.into());
                }
                try!(Index::create(layout.index_path(), &INDEX_MAGIC, INDEX_VERSION))
            }
        };
        let bundle_map = match BundleMap::load(layout.bundle_map_path(), &mock_lock) {
            Ok(bundle_map) => bundle_map,
            Err(err) => {
                tr_error!("Failed to load local bundle map:\n\tcaused by: {}", err);
                if read_only {
                    return Err(err.into());
                }
                BundleMap::create()
            }
        };
        let mut repo = Repository {
            layout,
            chunker: config.chunker.create(),
            config,
            index,
            crypto,
            bundle_map,
            next_data_bundle: 0,
            next_meta_bundle: 0,
            bundles,
            data_bundle: None,
            meta_bundle: None,
            remote_locks,
            local_locks
        };
        if rebuild_bundle_map {
            try!(repo.rebuild_bundle_map(&mock_lock));
            rebuild_index = true;
        }
        if rebuild_index {
            try!(repo.rebuild_index(&mock_lock));
        }
        repo.next_meta_bundle = repo.next_free_bundle_id();
        repo.next_data_bundle = repo.next_free_bundle_id();
        Ok(repo)
    }

    //FIXME: use or remove
    #[allow(dead_code)]
    pub fn synchronize(&mut self, lock: &OnlineMode) -> Result<(), RepositoryError> {
        let (new, gone) = try!(self.bundles.synchronize(lock));
        let mut save_bundle_map = false;
        if !gone.is_empty() {
            tr_info!("Removing {} old bundles from index", gone.len());
            for bundle in gone {
                try!(self.remove_gone_remote_bundle(&bundle, lock.as_localwrite()))
            }
            save_bundle_map = true;
        }
        if !new.is_empty() {
            tr_info!("Adding {} new bundles to index", new.len());
            for bundle in ProgressIter::new(tr!("adding bundles to index"), new.len(), new.into_iter()) {
                try!(self.add_new_remote_bundle(&bundle, lock))
            }
            save_bundle_map = true;
        }
        if save_bundle_map {
            try!(self.save_bundle_map(lock.as_localwrite()));
        }
        self.next_meta_bundle = self.next_free_bundle_id();
        self.next_data_bundle = self.next_free_bundle_id();
        Ok(())
    }

    #[inline]
    pub fn save_config(&mut self, lock: &LocalWriteMode) -> Result<(), RepositoryError> {
        try!(self.config.save(self.layout.config_path(), lock));
        Ok(())
    }

    #[inline]
    pub fn set_encryption(&mut self, public: Option<&PublicKey>) {
        if let Some(key) = public {
            if !self.crypto.contains_secret_key(key) {
                tr_warn!("The secret key for that public key is not stored in the repository.")
            }
            let mut key_bytes = Vec::new();
            key_bytes.extend_from_slice(&key[..]);
            self.config.encryption = Some((EncryptionMethod::Sodium, key_bytes.into()))
        } else {
            self.config.encryption = None
        }
    }

    #[inline]
    pub fn save_bundle_map(&self, lock: &LocalWriteMode) -> Result<(), RepositoryError> {
        try!(self.bundle_map.save(self.layout.bundle_map_path(), lock));
        Ok(())
    }

    #[inline]
    pub fn next_free_bundle_id(&self) -> u32 {
        let mut id = max(self.next_data_bundle, self.next_meta_bundle) + 1;
        while self.bundle_map.get(id).is_some() {
            id += 1;
        }
        id
    }

    pub fn flush(&mut self, lock: &BackupMode) -> Result<(), RepositoryError> {
        if self.data_bundle.is_some() {
            let mut finished = None;
            mem::swap(&mut self.data_bundle, &mut finished);
            {
                let bundle = try!(self.bundles.add_bundle(finished.unwrap(), lock));
                self.bundle_map.set(
                    self.next_data_bundle,
                    bundle.id.clone(),
                    lock.as_localwrite()
                );
            }
            self.next_data_bundle = self.next_free_bundle_id()
        }
        if self.meta_bundle.is_some() {
            let mut finished = None;
            mem::swap(&mut self.meta_bundle, &mut finished);
            {
                let bundle = try!(self.bundles.add_bundle(finished.unwrap(), lock));
                self.bundle_map.set(
                    self.next_meta_bundle,
                    bundle.id.clone(),
                    lock.as_localwrite()
                );
            }
            self.next_meta_bundle = self.next_free_bundle_id()
        }
        try!(self.bundles.flush(lock));
        try!(self.save_bundle_map(lock.as_localwrite()));
        Ok(())
    }

    fn add_new_remote_bundle(&mut self, bundle: &BundleInfo, lock: &OnlineMode) -> Result<(), RepositoryError> {
        if self.bundle_map.find(&bundle.id).is_some() {
            return Ok(());
        }
        tr_debug!("Adding new bundle to index: {}", bundle.id);
        let bundle_id = match bundle.mode {
            BundleMode::Data => self.next_data_bundle,
            BundleMode::Meta => self.next_meta_bundle,
        };
        let chunks = try!(self.bundles.get_chunk_list(&bundle.id, lock));
        self.bundle_map.set(bundle_id, bundle.id.clone(), lock.as_localwrite());
        if self.next_meta_bundle == bundle_id {
            self.next_meta_bundle = self.next_free_bundle_id()
        }
        if self.next_data_bundle == bundle_id {
            self.next_data_bundle = self.next_free_bundle_id()
        }
        for (i, (hash, _len)) in chunks.into_inner().into_iter().enumerate() {
            if let Some(old) = try!(self.index.set(
                &hash,
                &Location {
                    bundle: bundle_id as u32,
                    chunk: i as u32
                }
            ))
                {
                    // Duplicate chunk, forced ordering: higher bundle id wins
                    let old_bundle_id = try!(self.get_bundle_id(old.bundle));
                    if old_bundle_id > bundle.id {
                        try!(self.index.set(&hash, &old));
                    }
                }
        }
        Ok(())
    }

    fn remove_gone_remote_bundle(&mut self, bundle: &BundleInfo, lock: &LocalWriteMode) -> Result<(), RepositoryError> {
        if let Some(id) = self.bundle_map.find(&bundle.id) {
            tr_debug!("Removing bundle from index: {}", bundle.id);
            try!(self.bundles.delete_local_bundle(&bundle.id, lock));
            try!(self.index.filter(|_key, data| data.bundle != id));
            self.bundle_map.remove(id, lock);
        }
        Ok(())
    }

    pub fn get_chunk_location(&self, chunk: Hash) -> Option<Location> {
        self.index.get(&chunk)
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn get_crypto(&self) -> Arc<Crypto> {
        self.crypto.clone()
    }

    pub fn get_layout(&self) -> Arc<ChunkRepositoryLayout> {
        self.layout.clone()
    }
}


struct Lock;


/**
- Local: readonly, shared lock
- Remote: offline
**/
pub trait ReadonlyMode {}

impl ReadonlyMode for Lock {}


/**
- Local: writable, exclusive lock, dirty flag
- Remote: offline
**/
pub trait LocalWriteMode: ReadonlyMode {
    fn as_readonly(&self) -> &ReadonlyMode;
}

impl LocalWriteMode for Lock {
    fn as_readonly(&self) -> &ReadonlyMode {
        self
    }
}


/**
- Local: writable, exclusive lock, dirty flag
- Remote: readonly, shared lock
**/
pub trait OnlineMode: LocalWriteMode {
    fn as_localwrite(&self) -> &LocalWriteMode;
}

impl OnlineMode for Lock {
    fn as_localwrite(&self) -> &LocalWriteMode {
        self
    }
}


/**
- Local: writable, exclusive lock, dirty flag
- Remote: append-only, shared lock
**/
pub trait BackupMode: OnlineMode {
    fn as_online(&self) -> &OnlineMode;
}

impl BackupMode for Lock {
    fn as_online(&self) -> &OnlineMode {
        self
    }
}


/**
- Local: writable, exclusive lock, dirty flag
- Remote: writable, exclusive lock
**/
pub trait VacuumMode: BackupMode {
    fn as_backup(&self) -> &BackupMode;
}

impl VacuumMode for Lock {
    fn as_backup(&self) -> &BackupMode {
        self
    }
}


impl Repository {
    fn create_dirty_file(&mut self) -> Result<(), RepositoryError> {
        let dirtyfile = self.layout.dirtyfile_path();
        if !dirtyfile.exists() {
            try!(File::create(&dirtyfile));
            Ok(())
        } else {
            Err(RepositoryError::Dirty)
        }
    }

    fn delete_dirty_file(&mut self) -> Result<(), RepositoryError> {
        let dirtyfile = self.layout.dirtyfile_path();
        if dirtyfile.exists() {
            try!(fs::remove_file(&dirtyfile));
        }
        Ok(())
    }

    //FIXME: use or remove
    #[allow(dead_code)]
    pub fn readonly_mode<R, F: FnOnce(&mut Repository, &ReadonlyMode) -> Result<R, RepositoryError>> (&mut self, f: F) -> Result<R, RepositoryError> {
        let _local_lock = try!(self.local_locks.lock(false));
        f(self, &Lock)
    }

    pub fn localwrite_mode<R, F: FnOnce(&mut Repository, &LocalWriteMode) -> Result<R, RepositoryError>> (&mut self, f: F) -> Result<R, RepositoryError> {
        let _local_lock = try!(self.local_locks.lock(true));
        f(self, &Lock)
    }

    pub fn online_mode<R, F: FnOnce(&mut Repository, &OnlineMode) -> Result<R, RepositoryError>> (&mut self, f: F) -> Result<R, RepositoryError> {
        let _local_lock = try!(self.local_locks.lock(true));
        let _remote_lock = try!(self.remote_locks.lock(false));
        f(self, &Lock)
    }

    pub fn backup_mode<R, F: FnOnce(&mut Repository, &BackupMode) -> Result<R, RepositoryError>> (&mut self, f: F) -> Result<R, RepositoryError> {
        let _local_lock = try!(self.local_locks.lock(true));
        let _remote_lock = try!(self.remote_locks.lock(false));
        try!(self.create_dirty_file());
        let res = f(self, &Lock);
        try!(self.flush(&Lock));
        if res.is_ok() {
            try!(self.delete_dirty_file());
        }
        res
    }

    pub fn vacuum_mode<R, F: FnOnce(&mut Repository, &VacuumMode) -> Result<R, RepositoryError>> (&mut self, f: F) -> Result<R, RepositoryError> {
        let _local_lock = try!(self.local_locks.lock(true));
        let _remote_lock = try!(self.remote_locks.lock(true));
        try!(self.create_dirty_file());
        let res = f(self, &Lock);
        try!(self.flush(&Lock));
        if res.is_ok() {
            try!(self.delete_dirty_file());
        }
        res
    }
}
