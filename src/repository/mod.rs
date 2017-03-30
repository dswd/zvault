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

use ::prelude::*;

use std::mem;
use std::cmp::max;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::sync::{Arc, Mutex};
use std::os::unix::fs::symlink;
use std::io::Write;

pub use self::error::RepositoryError;
pub use self::config::Config;
pub use self::metadata::{Inode, FileType, FileContents};
pub use self::backup::{BackupError, BackupOptions, DiffType};
pub use self::backup_file::{Backup, BackupFileError};
pub use self::integrity::RepositoryIntegrityError;
pub use self::info::{RepositoryInfo, BundleAnalysis};
use self::bundle_map::BundleMap;


const DEFAULT_EXCLUDES: &'static [u8] = include_bytes!("../../excludes.default");


pub struct Repository {
    path: PathBuf,
    backups_path: PathBuf,
    pub excludes_path: PathBuf,
    pub config: Config,
    index: Index,
    crypto: Arc<Mutex<Crypto>>,
    bundle_map: BundleMap,
    next_content_bundle: u32,
    next_meta_bundle: u32,
    bundles: BundleDb,
    content_bundle: Option<BundleWriter>,
    meta_bundle: Option<BundleWriter>,
    chunker: Chunker,
    locks: LockFolder
}


impl Repository {
    pub fn create<P: AsRef<Path>, R: AsRef<Path>>(path: P, config: Config, remote: R) -> Result<Self, RepositoryError> {
        let path = path.as_ref().to_owned();
        try!(fs::create_dir(&path));
        let mut excludes = try!(File::create(path.join("excludes")));
        try!(excludes.write_all(DEFAULT_EXCLUDES));
        try!(fs::create_dir(path.join("keys")));
        let crypto = Arc::new(Mutex::new(try!(Crypto::open(path.join("keys")))));
        try!(symlink(remote, path.join("remote")));
        try!(fs::create_dir_all(path.join("remote/locks")));
        let locks = LockFolder::new(path.join("remote/locks"));
        let bundles = try!(BundleDb::create(
            path.join("remote/bundles"),
            path.join("bundles"),
            crypto.clone()
        ));
        let index = try!(Index::create(&path.join("index")));
        try!(config.save(path.join("config.yaml")));
        let bundle_map = BundleMap::create();
        try!(bundle_map.save(path.join("bundles.map")));
        try!(fs::create_dir_all(&path.join("remote/backups")));
        Ok(Repository {
            backups_path: path.join("remote/backups"),
            excludes_path: path.join("excludes"),
            path: path,
            chunker: config.chunker.create(),
            config: config,
            index: index,
            bundle_map: bundle_map,
            next_content_bundle: 1,
            next_meta_bundle: 0,
            bundles: bundles,
            content_bundle: None,
            meta_bundle: None,
            crypto: crypto,
            locks: locks
        })
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RepositoryError> {
        let path = path.as_ref().to_owned();
        let config = try!(Config::load(path.join("config.yaml")));
        let locks = LockFolder::new(path.join("remote/locks"));
        let crypto = Arc::new(Mutex::new(try!(Crypto::open(path.join("keys")))));
        let (bundles, new, gone) = try!(BundleDb::open(
            path.join("remote/bundles"),
            path.join("bundles"),
            crypto.clone()
        ));
        let index = try!(Index::open(&path.join("index")));
        let bundle_map = try!(BundleMap::load(path.join("bundles.map")));
        let mut repo = Repository {
            backups_path: path.join("remote/backups"),
            excludes_path: path.join("excludes"),
            path: path,
            chunker: config.chunker.create(),
            config: config,
            index: index,
            crypto: crypto,
            bundle_map: bundle_map,
            next_content_bundle: 0,
            next_meta_bundle: 0,
            bundles: bundles,
            content_bundle: None,
            meta_bundle: None,
            locks: locks
        };
        for bundle in new {
            try!(repo.add_new_remote_bundle(bundle))
        }
        for bundle in gone {
            try!(repo.remove_gone_remote_bundle(bundle))
        }
        try!(repo.save_bundle_map());
        repo.next_meta_bundle = repo.next_free_bundle_id();
        repo.next_content_bundle = repo.next_free_bundle_id();
        Ok(repo)
    }

    pub fn import<P: AsRef<Path>, R: AsRef<Path>>(path: P, remote: R, key_files: Vec<String>) -> Result<Self, RepositoryError> {
        let path = path.as_ref();
        let mut repo = try!(Repository::create(path, Config::default(), remote));
        for file in key_files {
            try!(repo.crypto.lock().unwrap().register_keyfile(file));
        }
        repo = try!(Repository::open(path));
        let mut backups: Vec<Backup> = try!(repo.get_backups()).into_iter().map(|(_, v)| v).collect();
        backups.sort_by_key(|b| b.date);
        if let Some(backup) = backups.pop() {
            repo.config = backup.config;
            try!(repo.save_config())
        }
        Ok(repo)
    }

    #[inline]
    pub fn register_key(&mut self, public: PublicKey, secret: SecretKey) -> Result<(), RepositoryError> {
        Ok(try!(self.crypto.lock().unwrap().register_secret_key(public, secret)))
    }

    pub fn save_config(&mut self) -> Result<(), RepositoryError> {
        try!(self.config.save(self.path.join("config.yaml")));
        Ok(())
    }

    #[inline]
    pub fn set_encryption(&mut self, public: Option<&PublicKey>) {
        if let Some(key) = public {
            let mut key_bytes = Vec::new();
            key_bytes.extend_from_slice(&key[..]);
            self.config.encryption = Some((EncryptionMethod::Sodium, key_bytes.into()))
        } else {
            self.config.encryption = None
        }
    }

    #[inline]
    fn save_bundle_map(&self) -> Result<(), RepositoryError> {
        try!(self.bundle_map.save(self.path.join("bundles.map")));
        Ok(())
    }

    #[inline]
    fn next_free_bundle_id(&self) -> u32 {
        let mut id = max(self.next_content_bundle, self.next_meta_bundle) + 1;
        while self.bundle_map.get(id).is_some() {
            id += 1;
        }
        id
    }

    pub fn flush(&mut self) -> Result<(), RepositoryError> {
        if self.content_bundle.is_some() {
            let mut finished = None;
            mem::swap(&mut self.content_bundle, &mut finished);
            {
                let bundle = try!(self.bundles.add_bundle(finished.unwrap()));
                self.bundle_map.set(self.next_content_bundle, bundle.id.clone());
            }
            self.next_content_bundle = self.next_free_bundle_id()
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
        try!(self.save_bundle_map());
        try!(self.bundles.save_cache());
        Ok(())
    }

    fn add_new_remote_bundle(&mut self, bundle: BundleInfo) -> Result<(), RepositoryError> {
        info!("Adding new bundle to index: {}", bundle.id);
        let bundle_id = match bundle.mode {
            BundleMode::Content => self.next_content_bundle,
            BundleMode::Meta => self.next_meta_bundle
        };
        let chunks = try!(self.bundles.get_chunk_list(&bundle.id));
        self.bundle_map.set(bundle_id, bundle.id);
        if self.next_meta_bundle == bundle_id {
            self.next_meta_bundle = self.next_free_bundle_id()
        }
        if self.next_content_bundle == bundle_id {
            self.next_content_bundle = self.next_free_bundle_id()
        }
        for (i, (hash, _len)) in chunks.into_inner().into_iter().enumerate() {
            try!(self.index.set(&hash, &Location{bundle: bundle_id as u32, chunk: i as u32}));
        }
        Ok(())
    }

    fn remove_gone_remote_bundle(&mut self, bundle: BundleInfo) -> Result<(), RepositoryError> {
        if let Some(id) = self.bundle_map.find(&bundle.id) {
            info!("Removing bundle from index: {}", bundle.id);
            try!(self.bundles.delete_local_bundle(&bundle.id));
            try!(self.index.filter(|_key, data| data.bundle != id));
            self.bundle_map.remove(id);
        }
        Ok(())
    }

    fn lock(&self, exclusive: bool) -> Result<LockHandle, RepositoryError> {
        Ok(try!(self.locks.lock(exclusive)))
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        self.flush().expect("Failed to write last bundles")
    }
}
