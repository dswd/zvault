mod config;
mod bundle_map;
mod integrity;
mod basic_io;
mod info;
mod metadata;
mod backup;
mod error;
mod vacuum;
mod backup_file;
mod tarfile;
mod layout;

use ::prelude::*;

use std::mem;
use std::cmp::max;
use std::path::Path;
use std::fs::{self, File};
use std::sync::{Arc, Mutex};
use std::os::unix::fs::symlink;
use std::io::Write;

pub use self::error::RepositoryError;
pub use self::config::Config;
pub use self::metadata::{Inode, FileType, FileData, InodeError};
pub use self::backup::{BackupError, BackupOptions, DiffType};
pub use self::backup_file::{Backup, BackupFileError};
pub use self::integrity::IntegrityError;
pub use self::info::{RepositoryInfo, BundleAnalysis};
pub use self::layout::RepositoryLayout;
use self::bundle_map::BundleMap;


const REPOSITORY_README: &'static [u8] = include_bytes!("../../docs/repository_readme.md");
const DEFAULT_EXCLUDES: &'static [u8] = include_bytes!("../../docs/excludes.default");


pub struct Repository {
    pub layout: RepositoryLayout,
    pub config: Config,
    index: Index,
    crypto: Arc<Mutex<Crypto>>,
    bundle_map: BundleMap,
    next_data_bundle: u32,
    next_meta_bundle: u32,
    bundles: BundleDb,
    data_bundle: Option<BundleWriter>,
    meta_bundle: Option<BundleWriter>,
    chunker: Chunker,
    remote_locks: LockFolder,
    local_locks: LockFolder,
    lock: LockHandle,
    dirty: bool
}


impl Repository {
    pub fn create<P: AsRef<Path>, R: AsRef<Path>>(path: P, config: Config, remote: R) -> Result<Self, RepositoryError> {
        let layout = RepositoryLayout::new(path.as_ref().to_path_buf());
        try!(fs::create_dir(layout.base_path()));
        try!(File::create(layout.excludes_path()).and_then(|mut f| f.write_all(DEFAULT_EXCLUDES)));
        try!(fs::create_dir(layout.keys_path()));
        try!(fs::create_dir(layout.local_locks_path()));
        try!(symlink(remote, layout.remote_path()));
        try!(File::create(layout.remote_readme_path()).and_then(|mut f| f.write_all(REPOSITORY_README)));
        try!(fs::create_dir_all(layout.remote_locks_path()));
        try!(config.save(layout.config_path()));
        try!(BundleDb::create(layout.clone()));
        try!(Index::create(layout.index_path()));
        try!(BundleMap::create().save(layout.bundle_map_path()));
        try!(fs::create_dir_all(layout.backups_path()));
        Self::open(path)
    }

    #[allow(unknown_lints,useless_let_if_seq)]
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RepositoryError> {
        let layout = RepositoryLayout::new(path.as_ref().to_path_buf());
        if !layout.remote_exists() {
            return Err(RepositoryError::NoRemote)
        }
        let config = try!(Config::load(layout.config_path()));
        let remote_locks = LockFolder::new(layout.remote_locks_path());
        try!(fs::create_dir_all(layout.local_locks_path())); // Added after v0.1.0
        let local_locks = LockFolder::new(layout.local_locks_path());
        let lock = try!(local_locks.lock(false));
        let crypto = Arc::new(Mutex::new(try!(Crypto::open(layout.keys_path()))));
        let (bundles, new, gone) = try!(BundleDb::open(layout.clone(), crypto.clone()));
        let (index, mut rebuild_index) = match Index::open(layout.index_path()) {
            Ok(index) => (index, false),
            Err(err) => {
                error!("Failed to load local index:\n\tcaused by: {}", err);
                (try!(Index::create(layout.index_path())), true)
            }
        };
        let (bundle_map, rebuild_bundle_map) = match BundleMap::load(layout.bundle_map_path()) {
            Ok(bundle_map) => (bundle_map, false),
            Err(err) => {
                error!("Failed to load local bundle map:\n\tcaused by: {}", err);
                (BundleMap::create(), true)
            }
        };
        let dirty = layout.dirtyfile_path().exists();
        let mut repo = Repository {
            layout: layout,
            dirty: true,
            chunker: config.chunker.create(),
            config: config,
            index: index,
            crypto: crypto,
            bundle_map: bundle_map,
            next_data_bundle: 0,
            next_meta_bundle: 0,
            bundles: bundles,
            data_bundle: None,
            meta_bundle: None,
            lock: lock,
            remote_locks: remote_locks,
            local_locks: local_locks
        };
        if !rebuild_bundle_map {
            let mut save_bundle_map = false;
            if !new.is_empty() {
                info!("Adding {} new bundles to index", new.len());
                try!(repo.write_mode());
                for bundle in ProgressIter::new("adding bundles to index", new.len(), new.into_iter()) {
                    try!(repo.add_new_remote_bundle(bundle))
                }
                save_bundle_map = true;
            }
            if !gone.is_empty() {
                info!("Removig {} old bundles from index", gone.len());
                try!(repo.write_mode());
                for bundle in gone {
                    try!(repo.remove_gone_remote_bundle(bundle))
                }
                save_bundle_map = true;
            }
            if save_bundle_map {
                try!(repo.write_mode());
                try!(repo.save_bundle_map());
            }
        }
        repo.next_meta_bundle = repo.next_free_bundle_id();
        repo.next_data_bundle = repo.next_free_bundle_id();
        if rebuild_bundle_map {
            try!(repo.write_mode());
            try!(repo.rebuild_bundle_map());
            rebuild_index = true;
        }
        if rebuild_index {
            try!(repo.write_mode());
            try!(repo.rebuild_index());
        }
        repo.dirty = dirty;
        Ok(repo)
    }

    pub fn import<P: AsRef<Path>, R: AsRef<Path>>(path: P, remote: R, key_files: Vec<String>) -> Result<Self, RepositoryError> {
        let path = path.as_ref();
        let mut repo = try!(Repository::create(path, Config::default(), remote));
        for file in key_files {
            try!(repo.crypto.lock().unwrap().register_keyfile(file));
        }
        repo = try!(Repository::open(path));
        let mut backups: Vec<(String, Backup)> = try!(repo.get_backups()).into_iter().collect();
        backups.sort_by_key(|&(_, ref b)| b.date);
        if let Some((name, backup)) = backups.pop() {
            info!("Taking configuration from the last backup '{}'", name);
            repo.config = backup.config;
            try!(repo.save_config())
        } else {
            warn!("No backup found in the repository to take configuration from, please set the configuration manually.");
        }
        Ok(repo)
    }

    #[inline]
    pub fn register_key(&mut self, public: PublicKey, secret: SecretKey) -> Result<(), RepositoryError> {
        try!(self.write_mode());
        Ok(try!(self.crypto.lock().unwrap().register_secret_key(public, secret)))
    }

    #[inline]
    pub fn save_config(&mut self) -> Result<(), RepositoryError> {
        try!(self.write_mode());
        try!(self.config.save(self.layout.config_path()));
        Ok(())
    }

    #[inline]
    pub fn set_encryption(&mut self, public: Option<&PublicKey>) {
        if let Some(key) = public {
            if !self.crypto.lock().unwrap().contains_secret_key(key) {
                warn!("The secret key for that public key is not stored in the repository.")
            }
            let mut key_bytes = Vec::new();
            key_bytes.extend_from_slice(&key[..]);
            self.config.encryption = Some((EncryptionMethod::Sodium, key_bytes.into()))
        } else {
            self.config.encryption = None
        }
    }

    #[inline]
    fn save_bundle_map(&self) -> Result<(), RepositoryError> {
        try!(self.bundle_map.save(self.layout.bundle_map_path()));
        Ok(())
    }

    #[inline]
    fn next_free_bundle_id(&self) -> u32 {
        let mut id = max(self.next_data_bundle, self.next_meta_bundle) + 1;
        while self.bundle_map.get(id).is_some() {
            id += 1;
        }
        id
    }

    pub fn flush(&mut self) -> Result<(), RepositoryError> {
        let dirtyfile = self.layout.dirtyfile_path();
        if self.dirty && !dirtyfile.exists() {
            try!(File::create(&dirtyfile));
        }
        if self.data_bundle.is_some() {
            let mut finished = None;
            mem::swap(&mut self.data_bundle, &mut finished);
            {
                let bundle = try!(self.bundles.add_bundle(finished.unwrap()));
                self.bundle_map.set(self.next_data_bundle, bundle.id.clone());
            }
            self.next_data_bundle = self.next_free_bundle_id()
        }
        if self.meta_bundle.is_some() {
            let mut finished = None;
            mem::swap(&mut self.meta_bundle, &mut finished);
            {
                let bundle = try!(self.bundles.add_bundle(finished.unwrap()));
                self.bundle_map.set(self.next_meta_bundle, bundle.id.clone());
            }
            self.next_meta_bundle = self.next_free_bundle_id()
        }
        try!(self.bundles.finish_uploads());
        try!(self.save_bundle_map());
        try!(self.bundles.save_cache());
        if !self.dirty && dirtyfile.exists() {
            try!(fs::remove_file(&dirtyfile));
        }
        Ok(())
    }

    fn add_new_remote_bundle(&mut self, bundle: BundleInfo) -> Result<(), RepositoryError> {
        if self.bundle_map.find(&bundle.id).is_some() {
            return Ok(())
        }
        debug!("Adding new bundle to index: {}", bundle.id);
        let bundle_id = match bundle.mode {
            BundleMode::Data => self.next_data_bundle,
            BundleMode::Meta => self.next_meta_bundle
        };
        let chunks = try!(self.bundles.get_chunk_list(&bundle.id));
        self.bundle_map.set(bundle_id, bundle.id);
        if self.next_meta_bundle == bundle_id {
            self.next_meta_bundle = self.next_free_bundle_id()
        }
        if self.next_data_bundle == bundle_id {
            self.next_data_bundle = self.next_free_bundle_id()
        }
        for (i, (hash, _len)) in chunks.into_inner().into_iter().enumerate() {
            try!(self.index.set(&hash, &Location{bundle: bundle_id as u32, chunk: i as u32}));
        }
        Ok(())
    }

    fn rebuild_bundle_map(&mut self) -> Result<(), RepositoryError> {
        info!("Rebuilding bundle map from bundles");
        self.bundle_map = BundleMap::create();
        for bundle in self.bundles.list_bundles() {
            let bundle_id = match bundle.mode {
                BundleMode::Data => self.next_data_bundle,
                BundleMode::Meta => self.next_meta_bundle
            };
            self.bundle_map.set(bundle_id, bundle.id.clone());
            if self.next_meta_bundle == bundle_id {
                self.next_meta_bundle = self.next_free_bundle_id()
            }
            if self.next_data_bundle == bundle_id {
                self.next_data_bundle = self.next_free_bundle_id()
            }
        }
        self.save_bundle_map()
    }

    fn rebuild_index(&mut self) -> Result<(), RepositoryError> {
        info!("Rebuilding index from bundles");
        self.index.clear();
        for (num, id) in self.bundle_map.bundles() {
            let chunks = try!(self.bundles.get_chunk_list(&id));
            for (i, (hash, _len)) in chunks.into_inner().into_iter().enumerate() {
                try!(self.index.set(&hash, &Location{bundle: num as u32, chunk: i as u32}));
            }
        }
        Ok(())
    }

    fn remove_gone_remote_bundle(&mut self, bundle: BundleInfo) -> Result<(), RepositoryError> {
        if let Some(id) = self.bundle_map.find(&bundle.id) {
            debug!("Removing bundle from index: {}", bundle.id);
            try!(self.bundles.delete_local_bundle(&bundle.id));
            try!(self.index.filter(|_key, data| data.bundle != id));
            self.bundle_map.remove(id);
        }
        Ok(())
    }

    #[inline]
    fn write_mode(&mut self) -> Result<(), RepositoryError> {
        Ok(try!(self.local_locks.upgrade(&mut self.lock)))
    }

    #[inline]
    fn lock(&self, exclusive: bool) -> Result<LockHandle, RepositoryError> {
        Ok(try!(self.remote_locks.lock(exclusive)))
    }

    #[inline]
    pub fn set_clean(&mut self) {
        self.dirty = false;
    }
}


impl Drop for Repository {
    fn drop(&mut self) {
        self.flush().expect("Failed to write last bundles")
    }
}
