use prelude::*;

use super::*;
use super::super::bundle_map::BundleMap;

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
    }
}


impl RepositoryInner {
    pub fn get_chunk_marker(&self) -> Bitmap {
        Bitmap::new(self.index.capacity())
    }

    pub fn check_chunks(
        &self,
        checked: &mut Bitmap,
        chunks: &[Chunk],
        mark: bool,
    ) -> Result<bool, RepositoryError> {
        let mut new = false;
        for &(hash, _len) in chunks {
            if let Some(pos) = self.index.pos(&hash) {
                new |= !checked.get(pos);
                if mark {
                    checked.set(pos);
                }
            } else {
                return Err(IntegrityError::MissingChunk(hash).into());
            }
        }
        Ok(new)
    }

    fn check_index_chunks(&self) -> Result<(), RepositoryError> {
        let mut progress = ProgressBar::new(self.index.len() as u64);
        progress.message(tr!("checking index: "));
        progress.set_max_refresh_rate(Some(Duration::from_millis(100)));
        for (count, (_hash, location)) in self.index.iter().enumerate() {
            // Lookup bundle id from map
            let bundle_id = try!(self.get_bundle_id(location.bundle));
            // Get bundle object from bundledb
            let bundle = if let Some(bundle) = self.bundles.get_bundle_info(&bundle_id) {
                bundle
            } else {
                progress.finish_print(tr!("checking index: done."));
                return Err(IntegrityError::MissingBundle(bundle_id.clone()).into());
            };
            // Get chunk from bundle
            if bundle.info.chunk_count <= location.chunk as usize {
                progress.finish_print(tr!("checking index: done."));
                return Err(
                    IntegrityError::NoSuchChunk(bundle_id.clone(), location.chunk).into()
                );
            }
            if count % 1000 == 0 {
                progress.set(count as u64);
            }
        }
        progress.finish_print(tr!("checking index: done."));
        Ok(())
    }

    pub fn rebuild_bundle_map(&mut self) -> Result<(), RepositoryError> {
        tr_info!("Rebuilding bundle map from bundles");
        self.bundle_map = BundleMap::create();
        for bundle in self.bundles.list_bundles() {
            let bundle_id = match bundle.mode {
                BundleMode::Data => self.next_data_bundle,
                BundleMode::Meta => self.next_meta_bundle,
            };
            self.bundle_map.set(bundle_id, bundle.id.clone());
            if self.next_meta_bundle == bundle_id {
                self.next_meta_bundle = self.next_free_bundle_id()
            }
            if self.next_data_bundle == bundle_id {
                self.next_data_bundle = self.next_free_bundle_id()
            }
        }
        self.save_bundle_map()
    }

    pub fn rebuild_index(&mut self) -> Result<(), RepositoryError> {
        tr_info!("Rebuilding index from bundles");
        self.index.clear();
        let mut bundles = self.bundle_map.bundles();
        bundles.sort_by_key(|&(_, ref v)| v.clone());
        for (num, id) in ProgressIter::new(tr!("Rebuilding index from bundles"), bundles.len(), bundles.into_iter()) {
            let chunks = try!(self.bundles.get_chunk_list(&id));
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
    pub fn check_index(&mut self, repair: bool) -> Result<(), RepositoryError> {
        if repair {
            try!(self.write_mode());
        }
        tr_info!("Checking index integrity...");
        if let Err(err) = self.index.check() {
            if repair {
                tr_warn!(
                    "Problem detected: index was corrupted\n\tcaused by: {}",
                    err
                );
                return self.rebuild_index();
            } else {
                return Err(err.into());
            }
        }
        tr_info!("Checking index entries...");
        if let Err(err) = self.check_index_chunks() {
            if repair {
                tr_warn!(
                    "Problem detected: index entries were inconsistent\n\tcaused by: {}",
                    err
                );
                return self.rebuild_index();
            } else {
                return Err(err);
            }
        }
        Ok(())
    }

    #[inline]
    pub fn check_bundles(&mut self, full: bool, repair: bool) -> Result<(), RepositoryError> {
        if repair {
            try!(self.write_mode());
        }
        tr_info!("Checking bundle integrity...");
        if try!(self.bundles.check(full, repair)) {
            // Some bundles got repaired
            tr_warn!("Some bundles have been rewritten, please remove the broken bundles manually.");
            try!(self.rebuild_bundle_map());
            try!(self.rebuild_index());
        }
        Ok(())
    }

    pub fn check_repository(&mut self, repair: bool) -> Result<(), RepositoryError> {
        tr_info!("Checking repository integrity...");
        let mut rebuild = false;
        for (_id, bundle_id) in self.bundle_map.bundles() {
            if self.bundles.get_bundle_info(&bundle_id).is_none() {
                if repair {
                    tr_warn!(
                        "Problem detected: bundle map contains unknown bundle {}",
                        bundle_id
                    );
                    rebuild = true;
                } else {
                    return Err(IntegrityError::MissingBundle(bundle_id).into());
                }
            }
        }
        if self.bundle_map.len() < self.bundles.len() {
            if repair {
                tr_warn!("Problem detected: bundle map does not contain all remote bundles");
                rebuild = true;
            } else {
                return Err(IntegrityError::RemoteBundlesNotInMap.into());
            }
        }
        if self.bundle_map.len() > self.bundles.len() {
            if repair {
                tr_warn!("Problem detected: bundle map contains bundles multiple times");
                rebuild = true;
            } else {
                return Err(IntegrityError::MapContainsDuplicates.into());
            }
        }
        if rebuild {
            try!(self.rebuild_bundle_map());
            try!(self.rebuild_index());
        }
        Ok(())
    }

}
