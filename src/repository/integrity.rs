use super::{Repository, RepositoryError};

use ::bundle::BundleId;
use ::util::Hash;


quick_error!{
    #[derive(Debug)]
    pub enum RepositoryIntegrityError {
        MissingChunk(hash: Hash) {
            description("Missing chunk")
            display("Missing chunk: {}", hash)
        }
        MissingBundleId(id: u32) {
            description("Missing bundle")
            display("Missing bundle: {}", id)
        }
        MissingBundle(id: BundleId) {
            description("Missing bundle")
            display("Missing bundle: {}", id)
        }
        NoSuchChunk(bundle: BundleId, chunk: u32) {
            description("No such chunk")
            display("Bundle {} does not conain the chunk {}", bundle, chunk)
        }
        InvalidNextBundleId {
            description("Invalid next bundle id")
        }
        SymlinkWithoutTarget {
            description("Symlink without target")
        }
    }
}

impl Repository {
    fn check_chunk(&self, hash: Hash) -> Result<(), RepositoryError> {
        // Find bundle and chunk id in index
        let found = if let Some(found) = self.index.get(&hash) {
            found
        } else {
            return Err(RepositoryIntegrityError::MissingChunk(hash).into());
        };
        // Lookup bundle id from map
        let bundle_id = try!(self.get_bundle_id(found.bundle));
        // Get bundle object from bundledb
        let bundle = if let Some(bundle) = self.bundles.get_bundle(&bundle_id) {
            bundle
        } else {
            return Err(RepositoryIntegrityError::MissingBundle(bundle_id.clone()).into())
        };
        // Get chunk from bundle
        if bundle.info.chunk_count > found.chunk as usize {
            Ok(())
        } else {
            Err(RepositoryIntegrityError::NoSuchChunk(bundle_id.clone(), found.chunk).into())
        }
        //TODO: check that contents match their hash
    }

    pub fn check(&mut self, full: bool) -> Result<(), RepositoryError> {
        try!(self.flush());
        try!(self.bundles.check(full));
        try!(self.index.check());
        let mut pos = 0;
        loop {
            pos = if let Some(pos) = self.index.next_entry(pos) {
                pos
            } else {
                break
            };
            let entry = self.index.get_entry(pos).unwrap();
            try!(self.check_chunk(entry.key));
            pos += 1;
        }
        if self.next_content_bundle == self.next_meta_bundle {
            return Err(RepositoryIntegrityError::InvalidNextBundleId.into())
        }
        if self.bundle_map.get(self.next_content_bundle).is_some() {
            return Err(RepositoryIntegrityError::InvalidNextBundleId.into())
        }
        if self.bundle_map.get(self.next_meta_bundle).is_some() {
            return Err(RepositoryIntegrityError::InvalidNextBundleId.into())
        }
        Ok(())
    }
}
