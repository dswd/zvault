pub mod mount;
mod backup_file;
mod inode;
mod tarfile;
mod backup;
mod integrity;
mod vacuum;
mod metadata;
mod layout;

pub use self::backup::{BackupOptions, BackupError, DiffType, RepositoryBackupIO};
pub use self::backup_file::{BackupFile, BackupFileError};
pub use self::inode::{Inode, FileData, FileType, InodeError};
pub use self::integrity::{InodeIntegrityError, RepositoryIntegrityIO, CheckOptions, IntegrityReport};
pub use self::layout::BackupRepositoryLayout;
pub use self::metadata::RepositoryMetadataIO;
pub use self::vacuum::RepositoryVacuumIO;
pub use self::tarfile::RepositoryTarfileIO;

use ::prelude::*;

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Arc;
use std::fs::{self, File};
use std::io::Write;


const DEFAULT_EXCLUDES: &[u8] = include_bytes!("../../docs/excludes.default");


pub struct BackupRepository(Repository);

impl BackupRepository {
    pub fn create<P: AsRef<Path>, R: AsRef<Path>>(path: P, config: &Config, remote: R) -> Result<Self, RepositoryError> {
        let layout: Arc<ChunkRepositoryLayout> = Arc::new(path.as_ref().to_owned());
        try!(fs::create_dir(layout.base_path()));
        try!(File::create(layout.excludes_path()).and_then(|mut f| {
            f.write_all(DEFAULT_EXCLUDES)
        }));
        try!(fs::create_dir_all(layout.backups_path()));
        try!(fs::create_dir(layout.keys_path()));
        let crypto = Arc::new(try!(Crypto::open(layout.keys_path())));
        Ok(BackupRepository(try!(Repository::create(layout, config, crypto, remote))))
    }

    #[allow(unknown_lints, useless_let_if_seq)]
    pub fn open<P: AsRef<Path>>(path: P, online: bool) -> Result<Self, RepositoryError> {
        let layout: Arc<ChunkRepositoryLayout> = Arc::new(path.as_ref().to_owned());
        let crypto = Arc::new(try!(Crypto::open(layout.keys_path())));
        Ok(BackupRepository(try!(Repository::open(layout, crypto, online))))
    }

    pub fn import<P: AsRef<Path>, R: AsRef<Path>>(path: P, remote: R, key_files: Vec<String>) -> Result<Self, RepositoryError> {
        let config = Config::default();
        let mut repo = try!(Self::create(&path, &config, remote));
        for file in key_files {
            try!(repo.0.get_crypto().register_keyfile(file));
        }
        repo = try!(Self::open(&path, true));
        let mut backups: Vec<(String, BackupFile)> = try!(repo.0.get_all_backups()).into_iter().collect();
        backups.sort_by_key(|&(_, ref b)| b.timestamp);
        if let Some((name, backup)) = backups.pop() {
            tr_info!("Taking configuration from the last backup '{}'", name);
            repo.0.set_config(backup.config);
            try!(repo.save_config())
        } else {
            tr_warn!(
                "No backup found in the repository to take configuration from, please set the configuration manually."
            );
        }
        Ok(repo)
    }

    #[inline]
    pub fn has_backup(&self, name: &str) -> bool {
        self.0.has_backup(name)
    }

    #[inline]
    pub fn get_backup(&self, name: &str) -> Result<BackupFile, RepositoryError> {
        self.0.get_backup(name)
    }

    #[inline]
    pub fn register_key(&mut self, public: PublicKey, secret: SecretKey) -> Result<(), RepositoryError> {
        try!(self.0.get_crypto().register_secret_key(public, secret));
        Ok(())
    }


    #[inline]
    pub fn save_config(&mut self) -> Result<(), RepositoryError> {
        self.0.localwrite_mode(|r, l| r.save_config(l))
    }

    #[inline]
    pub fn set_encryption(&mut self, public: Option<&PublicKey>) {
        self.0.set_encryption(public)
    }

    #[inline]
    pub fn get_config(&self) -> &Config {
        self.0.get_config()
    }

    #[inline]
    pub fn set_config(&mut self, config: Config) {
        self.0.set_config(config);
    }

    #[inline]
    pub fn get_layout(&self) -> Arc<ChunkRepositoryLayout> {
        self.0.get_layout()
    }

    #[inline]
    pub fn info(&self) -> RepositoryInfo {
        self.0.info()
    }

    #[inline]
    pub fn statistics(&self) -> RepositoryStatistics {
        self.0.statistics()
    }

    #[inline]
    pub fn list_bundles(&self) -> Vec<&BundleInfo> {
        self.0.list_bundles()
    }

    #[inline]
    pub fn get_bundle(&self, bundle: &BundleId) -> Option<&StoredBundle> {
        self.0.get_bundle(bundle)
    }

    #[inline]
    pub fn get_all_backups(&self) -> Result<HashMap<String, BackupFile>, RepositoryError> {
        self.0.get_all_backups()
    }

    #[inline]
    pub fn get_backups<P: AsRef<Path>>(&self, path: P) -> Result<HashMap<String, BackupFile>, RepositoryError> {
        self.0.get_backups(path)
    }

    #[inline]
    pub fn delete_backup(&mut self, name: &str) -> Result<(), RepositoryError> {
        self.0.backup_mode(|r, l| r.delete_backup(name, l))
    }

    #[inline]
    pub fn save_backup(&mut self, backup: &BackupFile, name: &str) -> Result<(), RepositoryError> {
        self.0.backup_mode(|r, l| r.save_backup(backup, name, l))
    }

    #[inline]
    pub fn prune_backups(&mut self, prefix: &str, daily: usize, weekly: usize, monthly: usize,
        yearly: usize, force: bool) -> Result<(), RepositoryError>
    {
        self.0.backup_mode(|r, l| r.prune_backups(prefix, daily, weekly, monthly, yearly, force, l))
    }

    #[inline]
    pub fn get_root_inode(&mut self, backup: &BackupFile) -> Result<Inode, RepositoryError> {
        self.0.online_mode(|r, l| r.get_inode(&backup.root, l))
    }

    #[inline]
    pub fn get_inode_children(&mut self, inode: &Inode) -> Result<Vec<Inode>, RepositoryError> {
        self.0.online_mode(|r, l| r.get_inode_children(inode, l))
    }

    #[inline]
    pub fn restore_inode_tree<P: AsRef<Path>>(&mut self, backup: &BackupFile, inode: Inode, path: P) -> Result<(), RepositoryError> {
        self.0.online_mode(|r, l| r.restore_inode_tree(backup, inode, path, l))
    }

    #[inline]
    pub fn create_backup<P: AsRef<Path>>(&mut self, path: P, name: &str, reference: Option<&BackupFile>,
        options: &BackupOptions) -> Result<BackupFile, RepositoryError>
    {
        self.0.backup_mode(|r, l| r.create_backup(path, name, reference, options,l))
    }

    #[inline]
    pub fn remove_backup_path<P: AsRef<Path>>(&mut self, backup: &mut BackupFile, name: &str, path: P
    ) -> Result<(), RepositoryError> {
        self.0.backup_mode(|r, l| r.remove_backup_path(backup, name, path, l))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn get_backup_path<P: AsRef<Path>>(&mut self, backup: &BackupFile, path: P) -> Result<Vec<Inode>, RepositoryError> {
        self.0.online_mode(|r, l| r.get_backup_path(backup, path, l))
    }

    #[inline]
    pub fn get_backup_inode<P: AsRef<Path>>(&mut self, backup: &BackupFile, path: P) -> Result<Inode, RepositoryError> {
        self.0.online_mode(|r, l| r.get_backup_inode(backup, path, l))
    }

    #[inline]
    pub fn find_differences(&mut self, inode1: &Inode, inode2: &Inode
    ) -> Result<Vec<(DiffType, PathBuf)>, RepositoryError> {
        self.0.online_mode(|r, l| r.find_differences(inode1, inode2, l))
    }

    #[inline]
    pub fn find_versions<P: AsRef<Path>>(&mut self, path: P
    ) -> Result<Vec<(String, Inode)>, RepositoryError> {
        self.0.online_mode(|r, l| r.find_versions(path, l))
    }

    #[inline]
    pub fn find_duplicates(&mut self, inode: &Inode, min_size: u64
    ) -> Result<Vec<(Vec<PathBuf>, u64)>, RepositoryError> {
        self.0.online_mode(|r, l| r.find_duplicates(inode, min_size, l))
    }

    #[inline]
    pub fn analyze_usage(&mut self) -> Result<HashMap<u32, BundleAnalysis>, RepositoryError> {
        self.0.online_mode(|r, l| r.analyze_usage(l))
    }

    #[inline]
    pub fn vacuum(&mut self, ratio: f32, combine: bool, force: bool) -> Result<(), RepositoryError> {
        self.0.vacuum_mode(|r, l| r.vacuum(ratio, combine, force, l))
    }

    pub fn mount_repository<P: AsRef<Path>>(&mut self, path: Option<&str>,
        mountpoint: P) -> Result<(), RepositoryError> {
        self.0.online_mode(|r, l| {
            let fs = try!(FuseFilesystem::from_repository(r, l, path));
            fs.mount(mountpoint)
        })
    }

    pub fn mount_backup<P: AsRef<Path>>(&mut self, backup: BackupFile,
        mountpoint: P) -> Result<(), RepositoryError> {
        self.0.online_mode(|r, l| {
            let fs = try!(FuseFilesystem::from_backup(r, l, backup));
            fs.mount(mountpoint)
        })
    }

    pub fn mount_inode<P: AsRef<Path>>(&mut self, backup: BackupFile, inode: Inode,
        mountpoint: P) -> Result<(), RepositoryError> {
        self.0.online_mode(|r, l| {
            let fs = try!(FuseFilesystem::from_inode(r, l, backup, inode));
            fs.mount(mountpoint)
        })
    }

    pub fn check(&mut self, options: CheckOptions) -> Result<IntegrityReport, RepositoryError> {
        if options.get_repair() {
            self.0.vacuum_mode(|r, l| {
                r.check_and_repair(options, l)
            })
        } else {
            self.0.online_mode(|r, l| {
                Ok(r.check(options, l))
            })
        }
    }

    #[inline]
    pub fn import_tarfile<P: AsRef<Path>>(&mut self, tarfile: P) -> Result<BackupFile, RepositoryError> {
        self.0.backup_mode(|r, l| r.import_tarfile(tarfile, l))
    }

    #[inline]
    pub fn export_tarfile<P: AsRef<Path>>(&mut self, backup: &BackupFile, inode: Inode, tarfile: P
    ) -> Result<(), RepositoryError> {
        self.0.online_mode(|r, l| r.export_tarfile(backup, inode, tarfile, l))
    }

}