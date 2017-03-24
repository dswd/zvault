use ::prelude::*;


pub struct RepositoryInfo {
    pub bundle_count: usize,
    pub encoded_data_size: u64,
    pub raw_data_size: u64,
    pub compression_ratio: f32,
    pub chunk_count: usize,
    pub avg_chunk_size: f32,
    pub index_size: usize,
    pub index_capacity: usize,
    pub index_entries: usize
}


impl Repository {
    #[inline]
    pub fn list_bundles(&self) -> Vec<&BundleInfo> {
        self.bundles.list_bundles()
    }

    #[inline]
    pub fn get_bundle(&self, bundle: &BundleId) -> Option<&BundleInfo> {
        self.bundles.get_bundle_info(bundle)
    }

    pub fn info(&self) -> RepositoryInfo {
        let bundles = self.list_bundles();
        let encoded_data_size = bundles.iter().map(|b| b.encoded_size as u64).sum();
        let raw_data_size = bundles.iter().map(|b| b.raw_size as u64).sum();
        let chunk_count = bundles.iter().map(|b| b.chunk_count).sum();
        RepositoryInfo {
            bundle_count: bundles.len(),
            chunk_count: chunk_count,
            encoded_data_size: encoded_data_size,
            raw_data_size: raw_data_size,
            compression_ratio: encoded_data_size as f32 / raw_data_size as f32,
            avg_chunk_size: raw_data_size as f32 / chunk_count as f32,
            index_size: self.index.size(),
            index_capacity: self.index.capacity(),
            index_entries: self.index.len()
        }
    }
}
