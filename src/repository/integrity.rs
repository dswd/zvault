use prelude::*;

use super::*;
use super::bundle_map::BundleMap;
use super::bundledb::BundleDbError;
use super::index::IndexError;

use std::time::Duration;
use pbr::ProgressBar;


quick_error!{
    #[derive(Debug)]
    pub enum IntegrityError {
        MissingChunk(hash: Hash) {
            description(tr!("Missing chunk"))
            display("{}", tr_format!("Missing chunk: {}", hash))
        }
        MissingBundleId(id: u32) {
            description(tr!("Missing bundle"))
            display("{}", tr_format!("Missing bundle: {}", id))
        }
        MissingBundle(id: BundleId) {
            description(tr!("Missing bundle"))
            display("{}", tr_format!("Missing bundle: {}", id))
        }
        NoSuchChunk(bundle: BundleId, chunk: u32) {
            description(tr!("No such chunk"))
            display("{}", tr_format!("Bundle {} does not contain the chunk {}", bundle, chunk))
        }
        RemoteBundlesNotInMap {
            description(tr!("Remote bundles missing from map"))
        }
        MapContainsDuplicates {
            description(tr!("Map contains duplicates"))
        }
        Index(err: IndexError) {
            description(tr!("Index error"))
            display("{}", tr_format!("Index error: {}", err))
        }
        BundleIntegrity(id: BundleId, err: BundleDbError) {
            description(tr!("Bundle error"))
            display("{}", tr_format!("Bundle {} has error: {}", id, err))
        }
    }
}


pub struct ModuleIntegrityReport<T> {
    pub errors_fixed: Vec<T>,
    pub errors_unfixed: Vec<T>
}

pub struct IntegrityReport {
    pub bundle_map: Option<ModuleIntegrityReport<IntegrityError>>,
    pub index: Option<ModuleIntegrityReport<IntegrityError>>,
    pub bundles: Option<ModuleIntegrityReport<IntegrityError>>
}


pub struct ChunkMarker<'a> {
    marked: Bitmap,
    repo: &'a Repository
}

impl Repository {
    pub fn get_chunk_marker(&self) -> Bitmap {
        Bitmap::new(self.index.capacity())
    }

    pub fn mark_chunks(&mut self, bitmap: &mut Bitmap, chunks: &[Chunk], set_marked: bool) -> Result<bool, RepositoryError> {
        let mut new = false;
        for &(hash, _len) in chunks {
            if let Some(pos) = self.index.pos(&hash) {
                new |= !bitmap.get(pos);
                if set_marked {
                    bitmap.set(pos);
                }
            } else {
                return Err(IntegrityError::MissingChunk(hash).into());
            }
        }
        Ok(new)
    }

    pub fn check_bundle_map(&mut self) -> ModuleIntegrityReport<IntegrityError> {
        tr_info!("Checking bundle map...");
        let mut errors = vec![];
        for (_id, bundle_id) in self.bundle_map.bundles() {
            if self.bundles.get_bundle_info(&bundle_id).is_none() {
                errors.push(IntegrityError::MissingBundle(bundle_id).into());
            }
        }
        if self.bundle_map.len() < self.bundles.len() {
            errors.push(IntegrityError::RemoteBundlesNotInMap.into());
        }
        if self.bundle_map.len() > self.bundles.len() {
            errors.push(IntegrityError::MapContainsDuplicates.into());
        }
        ModuleIntegrityReport { errors_fixed: vec![], errors_unfixed: errors }
    }

    pub fn rebuild_bundle_map(&mut self, lock: &OnlineMode) -> Result<(), RepositoryError> {
        tr_info!("Rebuilding bundle map from bundles");
        try!(self.bundles.synchronize(lock));
        self.bundle_map = BundleMap::create();
        for bundle in self.bundles.list_bundles() {
            let bundle_id = match bundle.mode {
                BundleMode::Data => self.next_data_bundle,
                BundleMode::Meta => self.next_meta_bundle,
            };
            self.bundle_map.set(bundle_id, bundle.id.clone(), lock.as_localwrite());
            if self.next_meta_bundle == bundle_id {
                self.next_meta_bundle = self.next_free_bundle_id()
            }
            if self.next_data_bundle == bundle_id {
                self.next_data_bundle = self.next_free_bundle_id()
            }
        }
        self.save_bundle_map(lock.as_localwrite())
    }

    pub fn check_and_repair_bundle_map(&mut self, lock: &OnlineMode) -> Result<ModuleIntegrityReport<IntegrityError>, RepositoryError> {
        let mut report = self.check_bundle_map();
        if !report.errors_unfixed.is_empty() {
            try!(self.rebuild_bundle_map(lock));
            mem::swap(&mut report.errors_unfixed, &mut report.errors_fixed);
        }
        Ok(report)
    }

    fn check_index_chunks(&self) -> Vec<IntegrityError> {
        let mut errors = vec![];
        let mut progress = ProgressBar::new(self.index.len() as u64);
        progress.message(tr!("checking index: "));
        progress.set_max_refresh_rate(Some(Duration::from_millis(100)));
        for (count, (_hash, location)) in self.index.iter().enumerate() {
            // Lookup bundle id from map
            let bundle_id = if let Some(bundle_id) = self.bundle_map.get(location.bundle) {
                bundle_id
            } else {
                errors.push(IntegrityError::MissingBundleId(location.bundle));
                continue
            };
            // Get bundle object from bundledb
            let bundle = if let Some(bundle) = self.bundles.get_bundle_info(&bundle_id) {
                bundle
            } else {
                errors.push(IntegrityError::MissingBundle(bundle_id.clone()));
                continue
            };
            // Get chunk from bundle
            if bundle.info.chunk_count <= location.chunk as usize {
                errors.push(IntegrityError::NoSuchChunk(bundle_id.clone(), location.chunk));
                continue
            }
            if count % 1000 == 0 {
                progress.set(count as u64);
            }
        }
        progress.finish_print(tr!("checking index: done."));
        errors
    }

    pub fn rebuild_index(&mut self, lock: &OnlineMode) -> Result<(), RepositoryError> {
        tr_info!("Rebuilding index from bundles");
        self.index.clear();
        let mut bundles = self.bundle_map.bundles();
        bundles.sort_by_key(|&(_, ref v)| v.clone());
        for (num, id) in ProgressIter::new(tr!("Rebuilding index from bundles"), bundles.len(), bundles.into_iter()) {
            let chunks = try!(self.bundles.get_chunk_list(&id, lock));
            for (i, (hash, _len)) in chunks.into_inner().into_iter().enumerate() {
                try!(self.index.set(
                    &hash,
                    &Location {
                        bundle: num as u32,
                        chunk: i as u32
                    }
                ));
            }
        }
        Ok(())
    }

    #[inline]
    pub fn check_index(&mut self, lock: &ReadonlyMode) -> ModuleIntegrityReport<IntegrityError> {
        tr_info!("Checking index integrity...");
        let mut errors: Vec<IntegrityError> = self.index.check().into_iter().map(IntegrityError::Index).collect();
        tr_info!("Checking index entries...");
        errors.extend(self.check_index_chunks());
        ModuleIntegrityReport { errors_fixed: vec![], errors_unfixed: errors }
    }

    pub fn check_and_repair_index(&mut self, lock: &OnlineMode) -> Result<ModuleIntegrityReport<IntegrityError>, RepositoryError> {
        let mut report = self.check_index(lock.as_readonly());
        if !report.errors_unfixed.is_empty() {
            try!(self.rebuild_index(lock));
            mem::swap(&mut report.errors_unfixed, &mut report.errors_fixed);
        }
        Ok(report)
    }

    #[inline]
    fn check_bundles_internal(&mut self, full: bool, lock: &OnlineMode) -> (ModuleIntegrityReport<IntegrityError>, Vec<BundleId>) {
        tr_info!("Checking bundle integrity...");
        let mut errors = vec![];
        let mut bundles = vec![];
        for (id, err) in self.bundles.check(full, lock) {
            bundles.push(id.clone());
            errors.push(IntegrityError::BundleIntegrity(id, err));
        }
        (ModuleIntegrityReport { errors_fixed: vec![], errors_unfixed: errors }, bundles)
    }

    #[inline]
    pub fn check_bundles(&mut self, full: bool, lock: &OnlineMode) -> ModuleIntegrityReport<IntegrityError> {
        self.check_bundles_internal(full, lock).0
    }

    pub fn check_and_repair_bundles(&mut self, full: bool, lock: &VacuumMode) -> Result<ModuleIntegrityReport<IntegrityError>, RepositoryError> {
        let (mut report, bundles) = self.check_bundles_internal(full, lock.as_online());
        if !report.errors_unfixed.is_empty() {
            try!(self.bundles.repair(lock, &bundles));
            mem::swap(&mut report.errors_unfixed, &mut report.errors_fixed);
            // Some bundles got repaired
            tr_warn!("Some bundles have been rewritten, please remove the broken bundles manually.");
            try!(self.rebuild_bundle_map(lock.as_online()));
            try!(self.rebuild_index(lock.as_online()));
        }
        Ok(report)
    }

    pub fn check(&mut self, index: bool, bundles: bool, bundle_data: bool, lock: &OnlineMode) -> IntegrityReport {
        let mut report = IntegrityReport {
            bundle_map: None,
            index: None,
            bundles: None
        };
        report.bundle_map = Some(self.check_bundle_map());
        if index {
            report.index = Some(self.check_index(lock.as_readonly()));
        }
        if bundles {
            report.bundles = Some(self.check_bundles(bundle_data, lock));
        }
        report
    }

    pub fn check_and_repair(&mut self, index: bool, bundles: bool, bundle_data: bool, lock: &VacuumMode) -> Result<IntegrityReport, RepositoryError> {
        let mut report = IntegrityReport {
            bundle_map: None,
            index: None,
            bundles: None
        };
        let bundle_map = try!(self.check_and_repair_bundle_map(lock.as_online()));
        if !bundle_map.errors_fixed.is_empty() {
            try!(self.rebuild_index(lock.as_online()));
        }
        report.bundle_map = Some(bundle_map);
        if index {
            report.index = Some(try!(self.check_and_repair_index(lock.as_online())));
        }
        if bundles {
            report.bundles = Some(try!(self.check_and_repair_bundles(bundle_data, lock)));
        }
        Ok(report)
    }

}
