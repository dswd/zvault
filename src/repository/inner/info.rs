use prelude::*;

use std::collections::HashMap;


pub struct BundleAnalysis {
    pub info: BundleInfo,
    pub chunk_usage: Bitmap,
    pub used_raw_size: usize
}

impl BundleAnalysis {
    #[inline]
    pub fn get_usage_ratio(&self) -> f32 {
        self.used_raw_size as f32 / self.info.raw_size as f32
    }

    #[inline]
    pub fn get_used_size(&self) -> usize {
        (self.get_usage_ratio() * self.info.encoded_size as f32) as usize
    }

    #[inline]
    pub fn get_unused_size(&self) -> usize {
        ((1.0 - self.get_usage_ratio()) * self.info.encoded_size as f32) as usize
    }
}

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


#[derive(Debug)]
pub struct RepositoryStatistics {
    pub index: IndexStatistics,
    pub bundles: BundleStatistics
}


impl RepositoryInner {
    #[inline]
    pub fn list_bundles(&self) -> Vec<&BundleInfo> {
        self.bundles.list_bundles()
    }

    pub fn get_bundle_map(&self) -> Result<HashMap<u32, &BundleInfo>, RepositoryError> {
        let mut map = HashMap::with_capacity(self.bundle_map.len());
        for (id, bundle_id) in self.bundle_map.bundles() {
            let info = try!(self.bundles.get_bundle_info(&bundle_id).ok_or_else(|| {
                IntegrityError::MissingBundle(bundle_id)
            }));
            map.insert(id, &info.info);
        }
        Ok(map)
    }

    #[inline]
    pub fn get_bundle(&self, bundle: &BundleId) -> Option<&StoredBundle> {
        self.bundles.get_bundle_info(bundle)
    }

    pub fn info(&self) -> RepositoryInfo {
        let bundles = self.list_bundles();
        let encoded_data_size = bundles.iter().map(|b| b.encoded_size as u64).sum();
        let raw_data_size = bundles.iter().map(|b| b.raw_size as u64).sum();
        let chunk_count = bundles.iter().map(|b| b.chunk_count).sum();
        RepositoryInfo {
            bundle_count: bundles.len(),
            chunk_count,
            encoded_data_size,
            raw_data_size,
            compression_ratio: encoded_data_size as f32 / raw_data_size as f32,
            avg_chunk_size: raw_data_size as f32 / chunk_count as f32,
            index_size: self.index.size(),
            index_capacity: self.index.capacity(),
            index_entries: self.index.len()
        }
    }

    #[allow(dead_code)]
    pub fn statistics(&self) -> RepositoryStatistics {
        RepositoryStatistics {
            index: self.index.statistics(),
            bundles: self.bundles.statistics()
        }
    }
}
