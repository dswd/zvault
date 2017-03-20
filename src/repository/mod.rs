mod config;
mod bundle_map;
mod integrity;
mod basic_io;
mod info;
mod metadata;
mod backup;
mod error;
mod vacuum;

use std::mem;
use std::cmp::max;
use std::path::{PathBuf, Path};
use std::fs;
use std::sync::{Arc, Mutex};

use super::index::Index;
use super::bundle::{BundleDb, BundleWriter};
use super::chunker::Chunker;
use ::util::*;

pub use self::error::RepositoryError;
pub use self::config::Config;
pub use self::metadata::{Inode, FileType};
pub use self::backup::Backup;
pub use self::integrity::RepositoryIntegrityError;
use self::bundle_map::BundleMap;


pub struct Repository {
    path: PathBuf,
    pub config: Config,
    index: Index,
    crypto: Arc<Mutex<Crypto>>,
    bundle_map: BundleMap,
    next_content_bundle: u32,
    next_meta_bundle: u32,
    bundles: BundleDb,
    content_bundle: Option<BundleWriter>,
    meta_bundle: Option<BundleWriter>,
    chunker: Chunker
}


impl Repository {
    pub fn create<P: AsRef<Path>>(path: P, config: Config) -> Result<Self, RepositoryError> {
        let path = path.as_ref().to_owned();
        try!(fs::create_dir(&path));
        try!(fs::create_dir(path.join("keys")));
        let crypto = Arc::new(Mutex::new(try!(Crypto::open(path.join("keys")))));
        let bundles = try!(BundleDb::create(
            path.join("bundles"),
            crypto.clone()
        ));
        let index = try!(Index::create(&path.join("index")));
        try!(config.save(path.join("config.yaml")));
        let bundle_map = BundleMap::create();
        try!(bundle_map.save(path.join("bundles.map")));
        try!(fs::create_dir(&path.join("backups")));
        Ok(Repository{
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
            crypto: crypto
        })
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RepositoryError> {
        let path = path.as_ref().to_owned();
        let config = try!(Config::load(path.join("config.yaml")));
        let crypto = Arc::new(Mutex::new(try!(Crypto::open(path.join("keys")))));
        let bundles = try!(BundleDb::open(
            path.join("bundles"),
            crypto.clone()
        ));
        let index = try!(Index::open(&path.join("index")));
        let bundle_map = try!(BundleMap::load(path.join("bundles.map")));
        let mut repo = Repository {
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
        };
        repo.next_meta_bundle = repo.next_free_bundle_id();
        repo.next_content_bundle = repo.next_free_bundle_id();
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
                self.bundle_map.set(self.next_content_bundle, bundle);
            }
            self.next_content_bundle = self.next_free_bundle_id()
        }
        if self.meta_bundle.is_some() {
            let mut finished = None;
            mem::swap(&mut self.meta_bundle, &mut finished);
            {
                let bundle = try!(self.bundles.add_bundle(finished.unwrap()));
                self.bundle_map.set(self.next_meta_bundle, bundle);
            }
            self.next_meta_bundle = self.next_free_bundle_id()
        }
        try!(self.save_bundle_map());
        Ok(())
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        self.flush().expect("Failed to write last bundles")
    }
}
