use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

use quick_error::ResultExt;

use util::*;
use super::*;


pub struct BundleDb {
    path: PathBuf,
    crypto: Arc<Mutex<Crypto>>,
    bundles: HashMap<BundleId, Bundle>,
    bundle_cache: LruCache<BundleId, Vec<u8>>
}


impl BundleDb {
    fn new(path: PathBuf, crypto: Arc<Mutex<Crypto>>) -> Self {
        BundleDb {
            path: path,
            crypto: crypto,
            bundles: HashMap::new(),
            bundle_cache: LruCache::new(5, 10)
        }
    }

    pub fn bundle_path(&self, bundle: &BundleId) -> (PathBuf, PathBuf) {
        let mut folder = self.path.clone();
        let mut file = bundle.to_string().to_owned() + ".bundle";
        let mut count = self.bundles.len();
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

    fn load_bundle_list(&mut self) -> Result<(), BundleError> {
        self.bundles.clear();
        let mut paths = Vec::new();
        paths.push(self.path.clone());
        while let Some(path) = paths.pop() {
            for entry in try!(fs::read_dir(path).map_err(BundleError::List)) {
                let entry = try!(entry.map_err(BundleError::List));
                let path = entry.path();
                if path.is_dir() {
                    paths.push(path);
                } else {
                    let bundle = try!(Bundle::load(path, self.crypto.clone()));
                    self.bundles.insert(bundle.id(), bundle);
                }
            }
        }
        Ok(())
    }

    #[inline]
    pub fn open<P: AsRef<Path>>(path: P, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleError> {
        let path = path.as_ref().to_owned();
        let mut self_ = Self::new(path, crypto);
        try!(self_.load_bundle_list());
        Ok(self_)
    }

    #[inline]
    pub fn create<P: AsRef<Path>>(path: P, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleError> {
        let path = path.as_ref().to_owned();
        try!(fs::create_dir_all(&path).context(&path as &Path));
        Ok(Self::new(path, crypto))
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

    pub fn get_chunk(&mut self, bundle_id: &BundleId, id: usize) -> Result<Vec<u8>, BundleError> {
        let bundle = try!(self.bundles.get(bundle_id).ok_or(BundleError::NoSuchBundle(bundle_id.clone())));
        let (pos, len) = try!(bundle.get_chunk_position(id));
        let mut chunk = Vec::with_capacity(len);
        if let Some(data) = self.bundle_cache.get(bundle_id) {
            chunk.extend_from_slice(&data[pos..pos+len]);
            return Ok(chunk);
        }
        let data = try!(bundle.load_contents());
        chunk.extend_from_slice(&data[pos..pos+len]);
        self.bundle_cache.put(bundle_id.clone(), data);
        Ok(chunk)
    }

    #[inline]
    pub fn add_bundle(&mut self, bundle: BundleWriter) -> Result<&Bundle, BundleError> {
        let bundle = try!(bundle.finish(&self));
        let id = bundle.id();
        self.bundles.insert(id.clone(), bundle);
        Ok(self.get_bundle(&id).unwrap())
    }

    #[inline]
    pub fn get_bundle(&self, bundle: &BundleId) -> Option<&Bundle> {
        self.bundles.get(bundle)
    }

    #[inline]
    pub fn list_bundles(&self) -> Vec<&Bundle> {
        self.bundles.values().collect()
    }

    #[inline]
    pub fn delete_bundle(&mut self, bundle: &BundleId) -> Result<(), BundleError> {
        if let Some(bundle) = self.bundles.remove(bundle) {
            fs::remove_file(&bundle.path).map_err(|e| BundleError::Remove(e, bundle.id()))
        } else {
            Err(BundleError::NoSuchBundle(bundle.clone()))
        }
    }

    #[inline]
    pub fn check(&self, full: bool) -> Result<(), BundleError> {
        for bundle in self.bundles.values() {
            try!(bundle.check(full))
        }
        Ok(())
    }
}
