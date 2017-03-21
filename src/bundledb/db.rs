use ::prelude::*;
use super::*;

use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{Arc, Mutex};


pub fn bundle_path(bundle: &BundleId, mut folder: PathBuf, mut count: usize) -> (PathBuf, PathBuf) {
    let mut file = bundle.to_string().to_owned() + ".bundle";
    while count >= 100 {
        if file.len() < 10 {
            break
        }
        folder = folder.join(&file[0..2]);
        file = file[2..].to_string();
        count /= 100;
    }
    (folder, file.into())
}

pub fn load_bundles<P: AsRef<Path>>(path: P, bundles: &mut HashMap<BundleId, StoredBundle>) -> Result<(Vec<BundleId>, Vec<BundleInfo>), BundleError> {
    let mut paths = vec![path.as_ref().to_path_buf()];
    let mut bundle_paths = HashSet::new();
    while let Some(path) = paths.pop() {
        for entry in try!(fs::read_dir(path).map_err(BundleError::List)) {
            let entry = try!(entry.map_err(BundleError::List));
            let path = entry.path();
            if path.is_dir() {
                paths.push(path);
            } else {
                bundle_paths.insert(path);
            }
        }
    }
    let mut gone = vec![];
    for (id, bundle) in bundles.iter_mut() {
        if !bundle_paths.contains(&bundle.path) {
            gone.push(id.clone());
        } else {
            bundle_paths.remove(&bundle.path);
        }
    }
    let gone = gone.iter().map(|id| bundles.remove(id).unwrap().info).collect();
    let mut new = vec![];
    for path in bundle_paths {
        let bundle = StoredBundle {
            info: try!(Bundle::load_info(&path)),
            path: path
        };
        new.push(bundle.info.id.clone());
        bundles.insert(bundle.info.id.clone(), bundle);
    }
    Ok((new, gone))
}


#[derive(Clone, Default)]
pub struct StoredBundle {
    pub info: BundleInfo,
    pub path: PathBuf
}
serde_impl!(StoredBundle(u64) {
    info: BundleInfo => 0,
    path: PathBuf => 1
});

impl StoredBundle {
    #[inline]
    pub fn id(&self) -> BundleId {
        self.info.id.clone()
    }

    pub fn move_to<P: AsRef<Path>>(mut self, path: P) -> Result<Self, BundleError> {
        let path = path.as_ref();
        if fs::rename(&self.path, path).is_err() {
            try!(fs::copy(&self.path, path).context(path));
            try!(fs::remove_file(&self.path).context(&self.path as &Path));
        }
        self.path = path.to_path_buf();
        Ok(self)
    }

    pub fn copy_to<P: AsRef<Path>>(&self, path: P) -> Result<Self, BundleError> {
        let path = path.as_ref();
        try!(fs::copy(&self.path, path).context(path));
        let mut bundle = self.clone();
        bundle.path = path.to_path_buf();
        Ok(bundle)
    }
}


pub struct BundleDb {
    remote_path: PathBuf,
    local_path: PathBuf,
    crypto: Arc<Mutex<Crypto>>,
    local_bundles: HashMap<BundleId, StoredBundle>,
    remote_bundles: HashMap<BundleId, StoredBundle>,
    bundle_cache: LruCache<BundleId, (Bundle, Vec<u8>)>
}


impl BundleDb {
    fn new(remote_path: PathBuf, local_path: PathBuf, crypto: Arc<Mutex<Crypto>>) -> Self {
        BundleDb {
            remote_path: remote_path,
            local_path: local_path,
            crypto: crypto,
            local_bundles: HashMap::new(),
            remote_bundles: HashMap::new(),
            bundle_cache: LruCache::new(5, 10)
        }
    }

    fn load_bundle_list(&mut self) -> Result<(Vec<BundleId>, Vec<BundleInfo>), BundleError> {
        try!(load_bundles(self.local_path.join("cached"), &mut self.local_bundles));
        load_bundles(&self.remote_path, &mut self.remote_bundles)
    }

    pub fn temp_bundle_path(&self, id: &BundleId) -> PathBuf {
        self.local_path.join("temp").join(id.to_string().to_owned() + ".bundle")
    }

    #[inline]
    pub fn open<R: AsRef<Path>, L: AsRef<Path>>(remote_path: R, local_path: L, crypto: Arc<Mutex<Crypto>>) -> Result<(Self, Vec<BundleId>, Vec<BundleInfo>), BundleError> {
        let remote_path = remote_path.as_ref().to_owned();
        let local_path = local_path.as_ref().to_owned();
        let mut self_ = Self::new(remote_path, local_path, crypto);
        let (new, gone) = try!(self_.load_bundle_list());
        Ok((self_, new, gone))
    }

    #[inline]
    pub fn create<R: AsRef<Path>, L: AsRef<Path>>(remote_path: R, local_path: L, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleError> {
        let remote_path = remote_path.as_ref().to_owned();
        let local_path = local_path.as_ref().to_owned();
        try!(fs::create_dir_all(&remote_path).context(&remote_path as &Path));
        try!(fs::create_dir_all(local_path.join("cached")).context(&local_path as &Path));
        try!(fs::create_dir_all(local_path.join("temp")).context(&local_path as &Path));
        Ok(Self::new(remote_path, local_path, crypto))
    }

    #[inline]
    pub fn create_bundle(
        &self,
        mode: BundleMode,
        hash_method: HashMethod,
        compression: Option<Compression>,
        encryption: Option<Encryption>
    ) -> Result<BundleWriter, BundleError> {
        BundleWriter::new(mode, hash_method, compression, encryption, self.crypto.clone())
    }

    fn get_stored_bundle(&self, bundle_id: &BundleId) -> Result<&StoredBundle, BundleError> {
        if let Some(stored) = self.local_bundles.get(bundle_id).or_else(|| self.remote_bundles.get(bundle_id)) {
            Ok(stored)
        } else {
            Err(BundleError::NoSuchBundle(bundle_id.clone()))
        }
    }

    fn get_bundle(&self, stored: &StoredBundle) -> Result<Bundle, BundleError> {
        Bundle::load(stored.path.clone(), self.crypto.clone())
    }

    pub fn get_chunk(&mut self, bundle_id: &BundleId, id: usize) -> Result<Vec<u8>, BundleError> {
        if let Some(&mut (ref mut bundle, ref data)) = self.bundle_cache.get_mut(bundle_id) {
            let (pos, len) = try!(bundle.get_chunk_position(id));
            let mut chunk = Vec::with_capacity(len);
            chunk.extend_from_slice(&data[pos..pos+len]);
            return Ok(chunk);
        }
        let mut bundle = try!(self.get_stored_bundle(bundle_id).and_then(|s| self.get_bundle(s)));
        let (pos, len) = try!(bundle.get_chunk_position(id));
        let mut chunk = Vec::with_capacity(len);
        let data = try!(bundle.load_contents());
        chunk.extend_from_slice(&data[pos..pos+len]);
        self.bundle_cache.put(bundle_id.clone(), (bundle, data));
        Ok(chunk)
    }

    #[inline]
    pub fn add_bundle(&mut self, bundle: BundleWriter) -> Result<BundleInfo, BundleError> {
        let bundle = try!(bundle.finish(&self));
        let id = bundle.id();
        if bundle.info.mode == BundleMode::Meta {
            let (folder, filename) = bundle_path(&id, self.local_path.join("cached"), self.local_bundles.len());
            try!(fs::create_dir_all(&folder).context(&folder as &Path));
            let bundle = try!(bundle.copy_to(folder.join(filename)));
            self.local_bundles.insert(id.clone(), bundle);
        }
        let (folder, filename) = bundle_path(&id, self.remote_path.clone(), self.remote_bundles.len());
        try!(fs::create_dir_all(&folder).context(&folder as &Path));
        let bundle = try!(bundle.copy_to(folder.join(filename)));
        self.remote_bundles.insert(bundle.id(), bundle.clone());
        Ok(bundle.info)
    }

    #[inline]
    pub fn get_bundle_info(&self, bundle: &BundleId) -> Option<&BundleInfo> {
        self.get_stored_bundle(bundle).ok().map(|stored| &stored.info)
    }

    #[inline]
    pub fn list_bundles(&self) -> Vec<&BundleInfo> {
        self.remote_bundles.values().map(|b| &b.info).collect()
    }

    #[inline]
    pub fn delete_bundle(&mut self, bundle: &BundleId) -> Result<(), BundleError> {
        if let Some(bundle) = self.local_bundles.remove(bundle) {
            try!(fs::remove_file(&bundle.path).map_err(|e| BundleError::Remove(e, bundle.id())))
        }
        if let Some(bundle) = self.remote_bundles.remove(bundle) {
            fs::remove_file(&bundle.path).map_err(|e| BundleError::Remove(e, bundle.id()))
        } else {
            Err(BundleError::NoSuchBundle(bundle.clone()))
        }
    }

    #[inline]
    pub fn check(&mut self, full: bool) -> Result<(), BundleError> {
        for stored in self.remote_bundles.values() {
            let mut bundle = try!(self.get_bundle(stored));
            try!(bundle.check(full))
        }
        Ok(())
    }
}
