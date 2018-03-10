pub mod mount;

use ::prelude::*;

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Arc;


pub struct BackupRepository {
    layout: Arc<RepositoryLayout>,
    repo: Repository
}

impl BackupRepository {
    pub fn create<P: AsRef<Path>, R: AsRef<Path>>(path: P, config: &Config, remote: R) -> Result<Self, RepositoryError> {
        let layout = Arc::new(RepositoryLayout::new(path.as_ref()));
        Ok(BackupRepository {
            layout: layout.clone(),
            repo: try!(Repository::create(layout, config, remote))
        })
    }

    #[allow(unknown_lints, useless_let_if_seq)]
    pub fn open<P: AsRef<Path>>(path: P, online: bool) -> Result<Self, RepositoryError> {
        let layout = Arc::new(RepositoryLayout::new(path.as_ref()));
        Ok(BackupRepository {
            layout: layout.clone(),
            repo: try!(Repository::open(layout, online))
        })
    }

    pub fn import<P: AsRef<Path>, R: AsRef<Path>>(path: P, remote: R, key_files: Vec<String>) -> Result<Self, RepositoryError> {
        let layout = Arc::new(RepositoryLayout::new(path.as_ref()));
        Ok(BackupRepository {
            layout: layout.clone(),
            repo: try!(Repository::import(layout, remote, key_files))
        })
    }

    #[inline]
    pub fn register_key(&mut self, public: PublicKey, secret: SecretKey) -> Result<(), RepositoryError> {
        self.repo.register_key(public, secret)
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
    pub fn has_backup(&self, name: &str) -> bool {
        self.repo.has_backup(name)
    }

    pub fn get_backup(&self, name: &str) -> Result<Backup, RepositoryError> {
        self.repo.get_backup(name)
    }

    #[inline]
    pub fn get_backup_inode<P: AsRef<Path>>(&mut self, backup: &Backup, path: P) -> Result<Inode, RepositoryError> {
        self.repo.get_backup_inode(backup, path)
    }

    #[inline]
    pub fn get_inode(&mut self, chunks: &[Chunk]) -> Result<Inode, RepositoryError> {
        self.repo.get_inode(chunks)
    }

    pub fn get_all_backups(&self) -> Result<HashMap<String, Backup>, RepositoryError> {
        self.repo.get_all_backups()
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

    pub fn create_backup_recursively<P: AsRef<Path>>(&mut self, path: P, reference: Option<&Backup>, options: &BackupOptions) -> Result<Backup, RepositoryError> {
        self.repo.create_backup_recursively(path, reference, options)
    }

    pub fn import_tarfile<P: AsRef<Path>>(&mut self, tarfile: P) -> Result<Backup, RepositoryError> {
        self.repo.import_tarfile(tarfile)
    }

    pub fn save_backup(&mut self, backup: &Backup, name: &str) -> Result<(), RepositoryError> {
        self.repo.save_backup(backup, name)
    }

    pub fn export_tarfile<P: AsRef<Path>>(&mut self, backup: &Backup, inode: Inode, tarfile: P) -> Result<(), RepositoryError> {
        self.repo.export_tarfile(backup, inode, tarfile)
    }

    pub fn restore_inode_tree<P: AsRef<Path>>(&mut self, backup: &Backup, inode: Inode, path: P) -> Result<(), RepositoryError> {
        self.repo.restore_inode_tree(backup, inode, path)
    }

    pub fn remove_backup_path<P: AsRef<Path>>(&mut self, backup: &mut Backup, path: P) -> Result<(), RepositoryError> {
        self.repo.remove_backup_path(backup, path)
    }

    pub fn get_backups<P: AsRef<Path>>(&self, path: P) -> Result<HashMap<String, Backup>, RepositoryError> {
        self.repo.get_backups(path)
    }

    pub fn delete_backup(&mut self, name: &str) -> Result<(), RepositoryError> {
        self.repo.delete_backup(name)
    }

    pub fn prune_backups(&mut self, prefix: &str, daily: usize, weekly: usize, monthly: usize, yearly: usize, force: bool) -> Result<(), RepositoryError> {
        self.repo.prune_backups(prefix, daily, weekly, monthly, yearly, force)
    }

    pub fn info(&self) -> RepositoryInfo {
        self.repo.info()
    }

    pub fn vacuum(&mut self, ratio: f32, combine: bool, force: bool) -> Result<(), RepositoryError> {
        self.repo.vacuum(ratio, combine, force)
    }

    pub fn check_repository(&mut self, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_repository(repair)
    }

    #[inline]
    pub fn check_bundles(&mut self, full: bool, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_bundles(full, repair)
    }

    #[inline]
    pub fn check_index(&mut self, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_index(repair)
    }

    pub fn check_backup_inode(&mut self, name: &str, backup: &mut Backup, path: &Path, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_backup_inode(name, backup, path, repair)
    }

    #[inline]
    pub fn check_backup(&mut self, name: &str, backup: &mut Backup, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_backup(name, backup, repair)
    }

    pub fn check_backups(&mut self, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_backups(repair)
    }

    #[inline]
    pub fn set_clean(&mut self) {
        self.repo.set_clean()
    }

    pub fn statistics(&self) -> RepositoryStatistics {
        self.repo.statistics()
    }

    pub fn find_duplicates(&mut self, inode: &Inode, min_size: u64) -> Result<Vec<(Vec<PathBuf>, u64)>, RepositoryError> {
        self.repo.find_duplicates(inode, min_size)
    }

    pub fn analyze_usage(&mut self) -> Result<HashMap<u32, BundleAnalysis>, RepositoryError> {
        self.repo.analyze_usage()
    }

    #[inline]
    pub fn list_bundles(&self) -> Vec<&BundleInfo> {
        self.repo.list_bundles()
    }

    #[inline]
    pub fn get_bundle(&self, bundle: &BundleId) -> Option<&StoredBundle> {
        self.repo.get_bundle(bundle)
    }

    pub fn find_versions<P: AsRef<Path>>(&mut self, path: P) -> Result<Vec<(String, Inode)>, RepositoryError> {
        self.repo.find_versions(path)
    }

    #[inline]
    pub fn find_differences(&mut self, inode1: &Inode, inode2: &Inode) -> Result<Vec<(DiffType, PathBuf)>, RepositoryError> {
        self.repo.find_differences(inode1, inode2)
    }

    pub fn get_chunk(&mut self, hash: Hash) -> Result<Option<Vec<u8>>, RepositoryError> {
        self.repo.get_chunk(hash)
    }

    pub fn get_data(&mut self, chunks: &[Chunk]) -> Result<Vec<u8>, RepositoryError> {
        self.repo.get_data(chunks)
    }
}
