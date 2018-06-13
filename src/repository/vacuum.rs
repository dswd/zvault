use prelude::*;

use std::collections::HashMap;


impl Repository {
    pub fn delete_bundle(&mut self, id: u32, lock: &VacuumMode) -> Result<(), RepositoryError> {
        if let Some(bundle) = self.bundle_map.remove(id, lock.as_localwrite()) {
            try!(self.bundles.delete_bundle(&bundle, lock));
            Ok(())
        } else {
            Err(IntegrityError::MissingBundleId(id).into())
        }
    }

    pub fn rewrite_bundles(&mut self, rewrite_bundles: &[u32], usage: &HashMap<u32, BundleAnalysis>, lock: &VacuumMode) -> Result<(), RepositoryError> {
        for &id in ProgressIter::new(
            tr!("rewriting bundles"),
            rewrite_bundles.len(),
            rewrite_bundles.iter()
        )
            {
                let bundle = &usage[&id];
                let bundle_id = self.bundle_map.get(id).unwrap();
                let chunks = try!(self.bundles.get_chunk_list(&bundle_id, lock.as_online()));
                let mode = usage[&id].info.mode;
                for (chunk, &(hash, _len)) in chunks.into_iter().enumerate() {
                    if !bundle.chunk_usage.get(chunk) {
                        try!(self.index.delete(&hash));
                        continue;
                    }
                    let data = try!(self.bundles.get_chunk(&bundle_id, chunk, lock.as_online()));
                    try!(self.put_chunk_override(mode, hash, &data, lock.as_backup()));
                }
            }
        try!(self.flush(lock.as_backup()));
        tr_info!("Checking index");
        for (hash, location) in self.index.iter() {
            let loc_bundle = location.bundle;
            let loc_chunk = location.chunk;
            if rewrite_bundles.contains(&loc_bundle) {
                tr_panic!(
                    "Removed bundle is still referenced in index: hash:{}, bundle:{}, chunk:{}",
                    hash,
                    loc_bundle,
                    loc_chunk
                );
            }
        }
        tr_info!("Deleting {} bundles", rewrite_bundles.len());
        for &id in rewrite_bundles {
            try!(self.delete_bundle(id, lock));
        }
        try!(self.save_bundle_map(lock.as_localwrite()));
        Ok(())
    }
}
