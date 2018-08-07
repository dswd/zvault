use prelude::*;

use std::path::{Path, PathBuf};

pub trait ChunkRepositoryLayout {
    fn base_path(&self) -> &Path;

    fn index_path(&self) -> PathBuf;
    fn bundle_map_path(&self) -> PathBuf;
    fn local_locks_path(&self) -> PathBuf;
    fn remote_path(&self) -> PathBuf;
    fn remote_exists(&self) -> bool;
    fn remote_locks_path(&self) -> PathBuf;
    fn remote_bundles_path(&self) -> PathBuf;
    fn local_bundles_path(&self) -> PathBuf;
    fn remote_bundle_path(&self, bundle: &BundleId, count: usize) -> PathBuf;
    fn local_bundle_path(&self, bundle: &BundleId, count: usize) -> PathBuf;
    fn temp_bundles_path(&self) -> PathBuf;
    fn temp_bundle_path(&self) -> PathBuf;
    fn local_bundle_cache_path(&self) -> PathBuf;
    fn remote_bundle_cache_path(&self) -> PathBuf;
    fn dirtyfile_path(&self) -> PathBuf;


    fn config_path(&self) -> PathBuf;
    fn remote_readme_path(&self) -> PathBuf;
}


fn bundle_path(bundle: &BundleId, mut folder: PathBuf, mut count: usize) -> PathBuf {
    let file = bundle.to_string().to_owned() + ".bundle";
    {
        let mut rest = &file as &str;
        while count >= 100 {
            if rest.len() < 10 {
                break;
            }
            folder = folder.join(&rest[0..2]);
            rest = &rest[2..];
            count /= 250;
        }
    }
    folder.join(Path::new(&file))
}


impl ChunkRepositoryLayout for PathBuf {
    #[inline]
    fn base_path(&self) -> &Path {
        &self
    }

    #[inline]
    fn remote_exists(&self) -> bool {
        self.remote_bundles_path().exists() && self.remote_locks_path().exists()
    }

    #[inline]
    fn index_path(&self) -> PathBuf {
        self.join("index")
    }

    #[inline]
    fn bundle_map_path(&self) -> PathBuf {
        self.join("bundles.map")
    }

    #[inline]
    fn local_locks_path(&self) -> PathBuf {
        self.join("locks")
    }

    #[inline]
    fn remote_path(&self) -> PathBuf {
        self.join("remote")
    }

    #[inline]
    fn remote_locks_path(&self) -> PathBuf {
        self.join("remote/locks")
    }

    #[inline]
    fn remote_bundles_path(&self) -> PathBuf {
        self.join("remote/bundles")
    }

    #[inline]
    fn local_bundles_path(&self) -> PathBuf {
        self.join("bundles/cached")
    }

    #[inline]
    fn remote_bundle_path(&self, _bundle: &BundleId, count: usize) -> PathBuf {
        bundle_path(&BundleId::random(), self.remote_bundles_path(), count)
    }

    #[inline]
    fn local_bundle_path(&self, bundle: &BundleId, count: usize) -> PathBuf {
        bundle_path(bundle, self.local_bundles_path(), count)
    }

    #[inline]
    fn temp_bundles_path(&self) -> PathBuf {
        self.join("bundles/temp")
    }

    #[inline]
    fn temp_bundle_path(&self) -> PathBuf {
        self.temp_bundles_path().join(BundleId::random().to_string().to_owned() + ".bundle")
    }

    #[inline]
    fn local_bundle_cache_path(&self) -> PathBuf {
        self.join("bundles/local.cache")
    }

    #[inline]
    fn remote_bundle_cache_path(&self) -> PathBuf {
        self.join("bundles/remote.cache")
    }

    #[inline]
    fn dirtyfile_path(&self) -> PathBuf {
        self.join("dirty")
    }



    #[inline]
    fn config_path(&self) -> PathBuf {
        self.join("config.yaml")
    }

    #[inline]
    fn remote_readme_path(&self) -> PathBuf {
        self.join("remote/README.md")
    }

}