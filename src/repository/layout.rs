use ::prelude::*;

use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct RepositoryLayout(PathBuf);

impl RepositoryLayout {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        RepositoryLayout(path.as_ref().to_path_buf())
    }

    #[inline]
    pub fn base_path(&self) -> &Path {
        &self.0
    }

    #[inline]
    pub fn config_path(&self) -> PathBuf {
        self.0.join("config.yaml")
    }

    #[inline]
    pub fn excludes_path(&self) -> PathBuf {
        self.0.join("excludes")
    }

    #[inline]
    pub fn index_path(&self) -> PathBuf {
        self.0.join("index")
    }

    #[inline]
    pub fn keys_path(&self) -> PathBuf {
        self.0.join("keys")
    }

    #[inline]
    pub fn bundle_map_path(&self) -> PathBuf {
        self.0.join("bundles.map")
    }

    #[inline]
    pub fn backups_path(&self) -> PathBuf {
        self.0.join("remote/backups")
    }

    #[inline]
    pub fn backup_path(&self, name: &str) -> PathBuf {
        self.backups_path().join(name)
    }

    #[inline]
    pub fn remote_path(&self) -> PathBuf {
        self.0.join("remote")
    }

    #[inline]
    pub fn remote_exists(&self) -> bool {
        self.remote_bundles_path().exists() && self.backups_path().exists() && self.remote_locks_path().exists()
    }

    #[inline]
    pub fn remote_readme_path(&self) -> PathBuf {
        self.0.join("remote/README.md")
    }

    #[inline]
    pub fn remote_locks_path(&self) -> PathBuf {
        self.0.join("remote/locks")
    }

    #[inline]
    pub fn remote_bundles_path(&self) -> PathBuf {
        self.0.join("remote/bundles")
    }

    #[inline]
    pub fn local_bundles_path(&self) -> PathBuf {
        self.0.join("bundles/cached")
    }

    fn bundle_path(&self, bundle: &BundleId, mut folder: PathBuf, mut count: usize) -> (PathBuf, PathBuf) {
        let mut file = bundle.to_string().to_owned() + ".bundle";
        while count >= 100 {
            if file.len() < 10 {
                break
            }
            folder = folder.join(&file[0..2]);
            file = file[2..].to_string();
            count /= 250;
        }
        (folder, file.into())
    }

    #[inline]
    pub fn remote_bundle_path(&self, count: usize) -> (PathBuf, PathBuf) {
        self.bundle_path(&BundleId::random(), self.remote_bundles_path(), count)
    }

    #[inline]
    pub fn local_bundle_path(&self, bundle: &BundleId, count: usize) -> (PathBuf, PathBuf) {
        self.bundle_path(bundle, self.local_bundles_path(), count)
    }

    #[inline]
    pub fn temp_bundles_path(&self) -> PathBuf {
        self.0.join("bundles/temp")
    }

    #[inline]
    pub fn temp_bundle_path(&self) -> PathBuf {
        self.temp_bundles_path().join(BundleId::random().to_string().to_owned() + ".bundle")
    }

    #[inline]
    pub fn local_bundle_cache_path(&self) -> PathBuf {
        self.0.join("bundles/local.cache")
    }

    #[inline]
    pub fn remote_bundle_cache_path(&self) -> PathBuf {
        self.0.join("bundles/remote.cache")
    }

    #[inline]
    pub fn dirtyfile_path(&self) -> PathBuf {
        self.0.join("dirty")
    }
}
