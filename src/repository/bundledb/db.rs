use ::prelude::*;
use super::*;

use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::Arc;
use std::io;
use std::mem;
use std::cmp::min;

quick_error!{
    #[derive(Debug)]
    pub enum BundleDbError {
        ListBundles(err: io::Error) {
            cause(err)
            description(tr!("Failed to list bundles"))
            display("{}", tr_format!("Bundle db error: failed to list bundles\n\tcaused by: {}", err))
        }
        Reader(err: BundleReaderError) {
            from()
            cause(err)
            description(tr!("Failed to read bundle"))
            display("{}", tr_format!("Bundle db error: failed to read bundle\n\tcaused by: {}", err))
        }
        Writer(err: BundleWriterError) {
            from()
            cause(err)
            description(tr!("Failed to write bundle"))
            display("{}", tr_format!("Bundle db error: failed to write bundle\n\tcaused by: {}", err))
        }
        Cache(err: BundleCacheError) {
            from()
            cause(err)
            description(tr!("Failed to read/write bundle cache"))
            display("{}", tr_format!("Bundle db error: failed to read/write bundle cache\n\tcaused by: {}", err))
        }
        UploadFailed {
            description(tr!("Uploading a bundle failed"))
        }
        Io(err: io::Error, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: io::Error) -> (err, path.to_path_buf())
            description(tr!("Io error"))
            display("{}", tr_format!("Bundle db error: io error on {:?}\n\tcaused by: {}", path, err))
        }
        NoSuchBundle(bundle: BundleId) {
            description(tr!("No such bundle"))
            display("{}", tr_format!("Bundle db error: no such bundle: {:?}", bundle))
        }
        Remove(err: io::Error, bundle: BundleId) {
            cause(err)
            description(tr!("Failed to remove bundle"))
            display("{}", tr_format!("Bundle db error: failed to remove bundle {}\n\tcaused by: {}", bundle, err))
        }
    }
}


#[allow(needless_pass_by_value)]
fn load_bundles(
    path: &Path,
    base: &Path,
    bundles: &mut HashMap<BundleId, StoredBundle>,
    crypto: Arc<Crypto>
) -> Result<(Vec<StoredBundle>, Vec<StoredBundle>), BundleDbError> {
    let mut paths = vec![path.to_path_buf()];
    let mut bundle_paths = HashSet::new();
    while let Some(path) = paths.pop() {
        for entry in try!(fs::read_dir(path).map_err(BundleDbError::ListBundles)) {
            let entry = try!(entry.map_err(BundleDbError::ListBundles));
            let path = entry.path();
            if path.is_dir() {
                paths.push(path);
            } else {
                if path.extension() != Some("bundle".as_ref()) {
                    continue;
                }
                bundle_paths.insert(path.strip_prefix(base).unwrap().to_path_buf());
            }
        }
    }
    let mut gone = HashSet::new();
    for (id, bundle) in bundles.iter() {
        if !bundle_paths.contains(&bundle.path) {
            gone.insert(id.clone());
        } else {
            bundle_paths.remove(&bundle.path);
        }
    }
    let mut new = vec![];
    for path in bundle_paths {
        let info = match BundleReader::load_info(base.join(&path), crypto.clone()) {
            Ok(info) => info,
            Err(err) => {
                warn!("Failed to read bundle {:?}\n\tcaused by: {}", path, err);
                info!("Ignoring unreadable bundle");
                continue;
            }
        };
        let bundle = StoredBundle {
            info,
            path
        };
        let id = bundle.info.id.clone();
        if !bundles.contains_key(&id) {
            new.push(bundle.clone());
        } else {
            gone.remove(&id);
        }
        bundles.insert(id, bundle);
    }
    let gone = gone.iter().map(|id| bundles.remove(id).unwrap()).collect();
    Ok((new, gone))
}



pub struct BundleDb {
    layout: Arc<ChunkRepositoryLayout>,
    uploader: Option<Arc<BundleUploader>>,
    crypto: Arc<Crypto>,
    local_bundles: HashMap<BundleId, StoredBundle>,
    remote_bundles: HashMap<BundleId, StoredBundle>,
    bundle_cache: LruCache<BundleId, (BundleReader, Vec<u8>)>
}


impl BundleDb {
    fn new(layout: Arc<ChunkRepositoryLayout>, crypto: Arc<Crypto>) -> Self {
        BundleDb {
            layout,
            crypto,
            uploader: None,
            local_bundles: HashMap::new(),
            remote_bundles: HashMap::new(),
            bundle_cache: LruCache::new(5, 10)
        }
    }

    fn load_local_bundle_list(
        &mut self,
        _lock: &ReadonlyMode
    ) -> Result<(), BundleDbError> {
        if let Ok(list) = StoredBundle::read_list_from(&self.layout.local_bundle_cache_path()) {
            for bundle in list {
                self.local_bundles.insert(bundle.id(), bundle);
            }
        } else {
            tr_warn!("Failed to read local bundle cache, rebuilding cache");
        }
        if let Ok(list) = StoredBundle::read_list_from(&self.layout.remote_bundle_cache_path()) {
            for bundle in list {
                self.remote_bundles.insert(bundle.id(), bundle);
            }
        } else {
            tr_warn!("Failed to read remote bundle cache, rebuilding cache");
        }
        let base_path = self.layout.base_path();
        let (new, gone) = try!(load_bundles(
            &self.layout.local_bundles_path(),
            base_path,
            &mut self.local_bundles,
            self.crypto.clone()
        ));
        if !new.is_empty() || !gone.is_empty() {
            let bundles: Vec<_> = self.local_bundles.values().cloned().collect();
            try!(StoredBundle::save_list_to(
                &bundles,
                &self.layout.local_bundle_cache_path()
            ));
        }
        Ok(())
    }

    fn load_remote_bundle_list(
        &mut self,
        lock: &OnlineMode
    ) -> Result<(Vec<StoredBundle>, Vec<StoredBundle>), BundleDbError> {
        try!(self.load_local_bundle_list(lock.as_readonly()));
        let base_path = self.layout.base_path();
        let (new, gone) = try!(load_bundles(
            &self.layout.remote_bundles_path(),
            base_path,
            &mut self.remote_bundles,
            self.crypto.clone()
        ));
        if !new.is_empty() || !gone.is_empty() {
            let bundles: Vec<_> = self.remote_bundles.values().cloned().collect();
            try!(StoredBundle::save_list_to(
                &bundles,
                &self.layout.remote_bundle_cache_path()
            ));
        }
        Ok((new, gone))
    }

    pub fn flush(&mut self, lock: &BackupMode) -> Result<(), BundleDbError> {
        self.finish_uploads(lock).and_then(|()| self.save_cache(lock.as_localwrite()))
    }

    fn save_cache(&self, _lock: &LocalWriteMode) -> Result<(), BundleDbError> {
        let bundles: Vec<_> = self.local_bundles.values().cloned().collect();
        try!(StoredBundle::save_list_to(
            &bundles,
            &self.layout.local_bundle_cache_path()
        ));
        let bundles: Vec<_> = self.remote_bundles.values().cloned().collect();
        try!(StoredBundle::save_list_to(
            &bundles,
            &self.layout.remote_bundle_cache_path()
        ));
        Ok(())
    }

    pub fn synchronize(&mut self, lock: &OnlineMode) -> Result<(Vec<BundleInfo>, Vec<BundleInfo>), BundleDbError> {
        let (new, gone) = try!(self.load_remote_bundle_list(lock));
        let mut meta_bundles = HashSet::new();
        for (id, bundle) in &self.remote_bundles {
            if bundle.info.mode == BundleMode::Meta {
                meta_bundles.insert(id.clone());
            }
        }
        let mut remove = vec![];
        for id in self.local_bundles.keys() {
            if !meta_bundles.contains(id) {
                remove.push(id.clone());
            }
        }
        for id in meta_bundles {
            if !self.local_bundles.contains_key(&id) {
                let bundle = self.remote_bundles[&id].clone();
                tr_debug!("Copying new meta bundle to local cache: {}", bundle.info.id);
                try!(self.copy_remote_bundle_to_cache(&bundle, lock));
            }
        }
        let base_path = self.layout.base_path();
        for id in remove {
            if let Some(bundle) = self.local_bundles.remove(&id) {
                try!(fs::remove_file(base_path.join(&bundle.path)).map_err(|e| {
                    BundleDbError::Remove(e, id)
                }))
            }
        }
        let new = new.into_iter().map(|s| s.info).collect();
        let gone = gone.into_iter().map(|s| s.info).collect();
        Ok((new, gone))
    }

    pub fn open(
        layout: Arc<ChunkRepositoryLayout>,
        crypto: Arc<Crypto>,
        lock: &ReadonlyMode
    ) -> Result<Self, BundleDbError> {
        let mut self_ = Self::new(layout, crypto);
        try!(self_.load_local_bundle_list(lock));
        Ok(self_)
    }

    pub fn create(layout: Arc<ChunkRepositoryLayout>) -> Result<(), BundleDbError> {
        try!(fs::create_dir_all(layout.remote_bundles_path()).context(
            &layout.remote_bundles_path() as
                &Path
        ));
        try!(fs::create_dir_all(layout.local_bundles_path()).context(
            &layout.local_bundles_path() as
                &Path
        ));
        try!(fs::create_dir_all(layout.temp_bundles_path()).context(
            &layout.temp_bundles_path() as
                &Path
        ));
        try!(StoredBundle::save_list_to(
            &[],
            layout.local_bundle_cache_path()
        ));
        try!(StoredBundle::save_list_to(
            &[],
            layout.remote_bundle_cache_path()
        ));
        Ok(())
    }

    #[inline]
    pub fn create_bundle(
        &self,
        mode: BundleMode,
        hash_method: HashMethod,
        compression: Option<Compression>,
        encryption: Option<Encryption>,
        lock: &BackupMode
    ) -> Result<BundleWriter, BundleDbError> {
        Ok(try!(BundleWriter::new(
            self.layout.clone(),
            mode,
            hash_method,
            compression,
            encryption,
            self.crypto.clone()
        )))
    }

    fn get_stored_bundle(&self, bundle_id: &BundleId) -> Result<&StoredBundle, BundleDbError> {
        if let Some(stored) = self.local_bundles.get(bundle_id).or_else(|| {
            self.remote_bundles.get(bundle_id)
        })
        {
            Ok(stored)
        } else {
            Err(BundleDbError::NoSuchBundle(bundle_id.clone()))
        }
    }

    #[inline]
    fn get_bundle(&self, stored: &StoredBundle, _lock: &OnlineMode) -> Result<BundleReader, BundleDbError> {
        let base_path = self.layout.base_path();
        Ok(try!(BundleReader::load(
            base_path.join(&stored.path),
            self.crypto.clone()
        )))
    }

    pub fn get_chunk(&mut self, bundle_id: &BundleId, id: usize, lock: &OnlineMode) -> Result<Vec<u8>, BundleDbError> {
        if let Some(&mut (ref mut bundle, ref data)) = self.bundle_cache.get_mut(bundle_id) {
            let (pos, len) = try!(bundle.get_chunk_position(id));
            let mut chunk = Vec::with_capacity(len);
            chunk.extend_from_slice(&data[pos..pos + len]);
            return Ok(chunk);
        }
        let mut bundle = try!(self.get_stored_bundle(bundle_id).and_then(
            |s| self.get_bundle(s, lock)
        ));
        let (pos, len) = try!(bundle.get_chunk_position(id));
        let mut chunk = Vec::with_capacity(len);
        let data = try!(bundle.load_contents());
        chunk.extend_from_slice(&data[pos..pos + len]);
        self.bundle_cache.put(bundle_id.clone(), (bundle, data));
        Ok(chunk)
    }

    fn copy_remote_bundle_to_cache(&mut self, bundle: &StoredBundle, _lock: &OnlineMode) -> Result<(), BundleDbError> {
        let id = bundle.id();
        let dst_path = self.layout.local_bundle_path(&id, self.local_bundles.len());
        {
            let folder = dst_path.parent().unwrap();
            try!(fs::create_dir_all(folder).context(folder as &Path));
        }
        let bundle = try!(bundle.copy_to(
            self.layout.base_path(),
            dst_path
        ));
        self.local_bundles.insert(id, bundle);
        Ok(())
    }

    pub fn add_bundle(&mut self, bundle: BundleWriter, lock: &BackupMode) -> Result<BundleInfo, BundleDbError> {
        let mut bundle = try!(bundle.finish());
        if bundle.info.mode == BundleMode::Meta {
            try!(self.copy_remote_bundle_to_cache(&bundle, lock.as_online()))
        }
        let dst_path = self.layout.remote_bundle_path(&bundle.id(),self.remote_bundles.len());
        let src_path = self.layout.base_path().join(bundle.path);
        bundle.path = dst_path
            .strip_prefix(self.layout.base_path())
            .unwrap()
            .to_path_buf();
        if self.uploader.is_none() {
            self.uploader = Some(BundleUploader::new(5));
        }
        try!(self.uploader.as_ref().unwrap().queue(src_path, dst_path));
        self.remote_bundles.insert(bundle.id(), bundle.clone());
        Ok(bundle.info)
    }

    fn finish_uploads(&mut self, _lock: &BackupMode) -> Result<(), BundleDbError> {
        let mut uploader = None;
        mem::swap(&mut self.uploader, &mut uploader);
        if let Some(uploader) = uploader {
            uploader.finish()
        } else {
            Ok(())
        }
    }

    pub fn get_chunk_list(&self, bundle: &BundleId, lock: &OnlineMode) -> Result<ChunkList, BundleDbError> {
        let mut bundle = try!(self.get_stored_bundle(bundle).and_then(|stored| {
            self.get_bundle(stored, lock)
        }));
        Ok(try!(bundle.get_chunk_list()).clone())
    }

    #[inline]
    pub fn get_bundle_info(&self, bundle: &BundleId) -> Option<&StoredBundle> {
        self.remote_bundles.get(bundle)
    }

    #[inline]
    pub fn list_bundles(&self) -> Vec<&BundleInfo> {
        self.remote_bundles.values().map(|b| &b.info).collect()
    }

    pub fn delete_local_bundle(&mut self, bundle: &BundleId, _lock: &LocalWriteMode) -> Result<(), BundleDbError> {
        if let Some(bundle) = self.local_bundles.remove(bundle) {
            let path = self.layout.base_path().join(&bundle.path);
            try!(fs::remove_file(path).map_err(|e| {
                BundleDbError::Remove(e, bundle.id())
            }))
        }
        Ok(())
    }

    pub fn delete_bundle(&mut self, bundle: &BundleId, lock: &VacuumMode) -> Result<(), BundleDbError> {
        try!(self.delete_local_bundle(bundle, lock.as_localwrite()));
        if let Some(bundle) = self.remote_bundles.remove(bundle) {
            let path = self.layout.base_path().join(&bundle.path);
            fs::remove_file(path).map_err(|e| BundleDbError::Remove(e, bundle.id()))
        } else {
            Err(BundleDbError::NoSuchBundle(bundle.clone()))
        }
    }

    pub fn check(&mut self, full: bool, lock: &OnlineMode) -> HashMap<BundleId, BundleDbError> {
        let mut errors = HashMap::new();
        for (id, stored) in ProgressIter::new(
            tr!("checking bundles"),
            self.remote_bundles.len(),
            self.remote_bundles.iter()
        )
        {
            let mut bundle = match self.get_bundle(stored, lock) {
                Ok(bundle) => bundle,
                Err(err) => {
                    errors.insert(id.clone(), err);
                    continue;
                }
            };
            if let Err(err) = bundle.check(full) {
                errors.insert(id.clone(), err.into());
                continue;
            }
        }
        errors
    }

    pub fn repair(&mut self, lock: &VacuumMode, bundles: &[BundleId]) -> Result<(), BundleDbError> {
        for id in ProgressIter::new(tr!("repairing bundles"), bundles.len(), bundles.iter()) {
            try!(self.repair_bundle(id, lock));
        }
        try!(self.flush(lock.as_backup()));
        Ok(())
    }

    fn evacuate_broken_bundle(&mut self, mut bundle: StoredBundle, _lock: &VacuumMode) -> Result<(), BundleDbError> {
        let src = self.layout.base_path().join(&bundle.path);
        let mut dst = src.with_extension("bundle.broken");
        let mut num = 1;
        while dst.exists() {
            dst = src.with_extension(&format!("bundle.{}.broken", num));
            num += 1;
        }
        warn!("Moving bundle to {:?}", dst);
        try!(bundle.move_to(self.layout.base_path(), dst));
        self.remote_bundles.remove(&bundle.info.id);
        Ok(())
    }

    fn repair_bundle(&mut self, id: &BundleId, lock: &VacuumMode) -> Result<(), BundleDbError> {
        let stored = self.remote_bundles[id].clone();
        let mut bundle = match self.get_bundle(&stored, lock.as_online()) {
            Ok(bundle) => bundle,
            Err(err) => {
                tr_warn!(
                    "Problem detected: failed to read bundle header: {}\n\tcaused by: {}",
                    id,
                    err
                );
                return self.evacuate_broken_bundle(stored, lock);
            }
        };
        let chunks = match bundle.get_chunk_list() {
            Ok(chunks) => chunks.clone(),
            Err(err) => {
                tr_warn!(
                    "Problem detected: failed to read bundle chunks: {}\n\tcaused by: {}",
                    id,
                    err
                );
                return self.evacuate_broken_bundle(stored, lock);
            }
        };
        let data = match bundle.load_contents() {
            Ok(data) => data,
            Err(err) => {
                tr_warn!(
                    "Problem detected: failed to read bundle data: {}\n\tcaused by: {}",
                    id,
                    err
                );
                return self.evacuate_broken_bundle(stored, lock);
            }
        };
        tr_warn!("Problem detected: bundle data was truncated: {}", id);
        tr_info!("Copying readable data into new bundle");
        let info = stored.info.clone();
        let mut new_bundle = try!(self.create_bundle(
            info.mode,
            info.hash_method,
            info.compression,
            info.encryption,
            lock.as_backup()
        ));
        let mut pos = 0;
        for (hash, mut len) in chunks.into_inner() {
            if pos >= data.len() {
                break;
            }
            len = min(len, (data.len() - pos) as u32);
            try!(new_bundle.add(&data[pos..pos + len as usize], hash));
            pos += len as usize;
        }
        let bundle = try!(self.add_bundle(new_bundle, lock.as_backup()));
        tr_info!("New bundle id is {}", bundle.id);
        self.evacuate_broken_bundle(stored, lock)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.remote_bundles.len()
    }

    pub fn statistics(&self) -> BundleStatistics {
        let bundles = self.list_bundles();
        let bundles_meta: Vec<_> = bundles.iter().filter(|b| b.mode == BundleMode::Meta).collect();
        let bundles_data: Vec<_> = bundles.iter().filter(|b| b.mode == BundleMode::Data).collect();
        let mut hash_methods = HashMap::new();
        let mut compressions = HashMap::new();
        let mut encryptions = HashMap::new();
        for bundle in &bundles {
            *hash_methods.entry(bundle.hash_method).or_insert(0) += 1;
            *compressions.entry(bundle.compression.clone()).or_insert(0) += 1;
            *encryptions.entry(bundle.encryption.clone()).or_insert(0) += 1;
        }
        BundleStatistics {
            hash_methods, compressions, encryptions,
            raw_size: ValueStats::from_iter(|| bundles.iter().map(|b| b.raw_size as f32)),
            encoded_size: ValueStats::from_iter(|| bundles.iter().map(|b| b.encoded_size as f32)),
            chunk_count: ValueStats::from_iter(|| bundles.iter().map(|b| b.chunk_count as f32)),
            raw_size_meta: ValueStats::from_iter(|| bundles_meta.iter().map(|b| b.raw_size as f32)),
            encoded_size_meta: ValueStats::from_iter(|| bundles_meta.iter().map(|b| b.encoded_size as f32)),
            chunk_count_meta: ValueStats::from_iter(|| bundles_meta.iter().map(|b| b.chunk_count as f32)),
            raw_size_data: ValueStats::from_iter(|| bundles_data.iter().map(|b| b.raw_size as f32)),
            encoded_size_data: ValueStats::from_iter(|| bundles_data.iter().map(|b| b.encoded_size as f32)),
            chunk_count_data: ValueStats::from_iter(|| bundles_data.iter().map(|b| b.chunk_count as f32))
        }
    }
}
