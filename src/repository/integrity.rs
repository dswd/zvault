use super::Repository;

use ::util::Hash;


impl Repository {
    fn check_chunk(&self, hash: Hash) -> Result<(), &'static str> {
        // Find bundle and chunk id in index
        let found = if let Some(found) = self.index.get(&hash) {
            found
        } else {
            return Err("Chunk not in index");
        };
        // Lookup bundle id from map
        let bundle_id = if let Some(bundle_info) = self.bundle_map.get(found.bundle) {
            bundle_info.id()
        } else {
            return Err("Bundle id not found in map")
        };
        // Get bundle object from bundledb
        let bundle = if let Some(bundle) = self.bundles.get_bundle(&bundle_id) {
            bundle
        } else {
            return Err("Bundle not found in bundledb")
        };
        // Get chunk from bundle
        if bundle.info.chunk_count > found.chunk as usize {
            Ok(())
        } else {
            Err("Bundle does not contain that chunk")
        }
        //TODO: check that contents match their hash
    }

    pub fn check(&mut self, full: bool) -> Result<(), &'static str> {
        try!(self.flush());
        try!(self.bundles.check(full).map_err(|_| "Bundles inconsistent"));
        try!(self.index.check().map_err(|_| "Index inconsistent"));
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
            return Err("Next bundle ids for meta and content as the same")
        }
        if self.bundle_map.get(self.next_content_bundle).is_some() {
            return Err("Bundle map already contains next bundle bundle id")
        }
        if self.bundle_map.get(self.next_meta_bundle).is_some() {
            return Err("Bundle map already contains next meta bundle id")
        }
        Ok(())
    }
}
