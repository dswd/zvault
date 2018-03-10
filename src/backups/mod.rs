pub mod mount;
mod backup_file;
mod inode;
mod tarfile;
mod backup;
mod integrity;
mod vacuum;

pub use self::backup::{BackupOptions, BackupError, DiffType};
pub use self::backup_file::{BackupFile, BackupFileError};
pub use self::inode::{Inode, FileData, FileType, InodeError};
pub use self::integrity::InodeIntegrityError;

use ::prelude::*;

use std::path::Path;
use std::collections::HashMap;
use std::sync::Arc;
use std::fs::{self, File};
use std::io::Write;


const DEFAULT_EXCLUDES: &[u8] = include_bytes!("../../docs/excludes.default");



pub struct BackupRepository {
    layout: Arc<RepositoryLayout>,
    crypto: Arc<Crypto>,
    repo: Repository
}

impl BackupRepository {
    pub fn create<P: AsRef<Path>, R: AsRef<Path>>(path: P, config: &Config, remote: R) -> Result<Self, RepositoryError> {
        let layout = Arc::new(RepositoryLayout::new(path.as_ref()));
        try!(fs::create_dir(layout.base_path()));
        try!(File::create(layout.excludes_path()).and_then(|mut f| {
            f.write_all(DEFAULT_EXCLUDES)
        }));
        try!(fs::create_dir(layout.keys_path()));
        let crypto = Arc::new(try!(Crypto::open(layout.keys_path())));
        Ok(BackupRepository {
            crypto: crypto.clone(),
            layout: layout.clone(),
            repo: try!(Repository::create(layout, config, crypto, remote))
        })
    }

    #[allow(unknown_lints, useless_let_if_seq)]
    pub fn open<P: AsRef<Path>>(path: P, online: bool) -> Result<Self, RepositoryError> {
        let layout = Arc::new(RepositoryLayout::new(path.as_ref()));
        let crypto = Arc::new(try!(Crypto::open(layout.keys_path())));
        Ok(BackupRepository {
            crypto: crypto.clone(),
            layout: layout.clone(),
            repo: try!(Repository::open(layout, crypto, online))
        })
    }

    pub fn import<P: AsRef<Path>, R: AsRef<Path>>(path: P, remote: R, key_files: Vec<String>) -> Result<Self, RepositoryError> {
        let config = Config::default();
        let mut repo = try!(Self::create(&path, &config, remote));
        for file in key_files {
            try!(repo.crypto.register_keyfile(file));
        }
        repo = try!(Self::open(&path, true));
        let mut backups: Vec<(String, BackupFile)> = try!(repo.get_all_backups()).into_iter().collect();
        backups.sort_by_key(|&(_, ref b)| b.timestamp);
        if let Some((name, backup)) = backups.pop() {
            tr_info!("Taking configuration from the last backup '{}'", name);
            repo.repo.set_config(backup.config);
            try!(repo.save_config())
        } else {
            tr_warn!(
                "No backup found in the repository to take configuration from, please set the configuration manually."
            );
        }
        Ok(repo)
    }

    #[inline]
    pub fn register_key(&mut self, public: PublicKey, secret: SecretKey) -> Result<(), RepositoryError> {
        try!(self.repo.write_mode());
        try!(self.crypto.register_secret_key(public, secret));
        Ok(())
    }


    #[inline]
    pub fn save_config(&mut self) -> Result<(), RepositoryError> {
        self.repo.save_config()
    }

    #[inline]
    pub fn set_encryption(&mut self, public: Option<&PublicKey>) {
        self.repo.set_encryption(public)
    }

    #[inline]
    pub fn get_inode(&mut self, chunks: &[Chunk]) -> Result<Inode, RepositoryError> {
        self.repo.get_inode(chunks)
    }

    pub fn get_config(&self) -> &Config {
        self.repo.get_config()
    }

    pub fn set_config(&mut self, config: Config) {
        self.repo.set_config(config);
    }

    pub fn get_layout(&self) -> &RepositoryLayout {
        &self.layout
    }

    pub fn info(&self) -> RepositoryInfo {
        self.repo.info()
    }

    #[inline]
    pub fn check_index(&mut self, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_index(repair)
    }

    #[inline]
    pub fn set_clean(&mut self) {
        self.repo.set_clean()
    }

    pub fn statistics(&self) -> RepositoryStatistics {
        self.repo.statistics()
    }

    #[inline]
    pub fn list_bundles(&self) -> Vec<&BundleInfo> {
        self.repo.list_bundles()
    }

    #[inline]
    pub fn get_bundle(&self, bundle: &BundleId) -> Option<&StoredBundle> {
        self.repo.get_bundle(bundle)
    }

    pub fn get_chunk(&mut self, hash: Hash) -> Result<Option<Vec<u8>>, RepositoryError> {
        self.repo.get_chunk(hash)
    }

    pub fn get_data(&mut self, chunks: &[Chunk]) -> Result<Vec<u8>, RepositoryError> {
        self.repo.get_data(chunks)
    }
}
