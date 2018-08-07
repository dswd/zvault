use ::repository::ChunkRepositoryLayout;

use std::path::PathBuf;

pub trait BackupRepositoryLayout {
    fn config_path(&self) -> PathBuf;
    fn keys_path(&self) -> PathBuf;
    fn excludes_path(&self) -> PathBuf;
    fn backups_path(&self) -> PathBuf;
    fn backup_path(&self, name: &str) -> PathBuf;
    fn remote_exists(&self) -> bool;
    fn remote_readme_path(&self) -> PathBuf;
}

impl<P: AsRef<ChunkRepositoryLayout>> BackupRepositoryLayout for P {
    fn config_path(&self) -> PathBuf {
        self.as_ref().base_path().join("config.yaml")
    }

    fn keys_path(&self) -> PathBuf {
        self.as_ref().base_path().join("keys")
    }

    fn excludes_path(&self) -> PathBuf {
        self.as_ref().base_path().join("excludes")
    }

    fn backups_path(&self) -> PathBuf {
        self.as_ref().base_path().join("remote/backups")
    }

    fn backup_path(&self, name: &str) -> PathBuf {
        self.backups_path().join(format!("{}.backup", name))
    }

    fn remote_exists(&self) -> bool {
        self.as_ref().remote_bundles_path().exists() && self.backups_path().exists() &&
            self.as_ref().remote_locks_path().exists()
    }

    fn remote_readme_path(&self) -> PathBuf {
        self.as_ref().base_path().join("remote/README.md")
    }

}
