use ::prelude::*;

use std::collections::{HashMap, VecDeque};


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


impl Repository {
    fn mark_used(&self, bundles: &mut HashMap<u32, BundleAnalysis>, chunks: &[Chunk]) -> Result<bool, RepositoryError> {
        let mut new = false;
        for &(hash, len) in chunks {
            if let Some(pos) = self.index.get(&hash) {
                if let Some(bundle) = bundles.get_mut(&pos.bundle) {
                    if !bundle.chunk_usage.get(pos.chunk as usize) {
                        new = true;
                        bundle.chunk_usage.set(pos.chunk as usize);
                        bundle.used_raw_size += len as usize;
                    }
                } else {
                    return Err(IntegrityError::MissingBundleId(pos.bundle).into());
                }
            } else {
                return Err(IntegrityError::MissingChunk(hash).into());
            }
        }
        Ok(new)
    }

    pub fn analyze_usage(&mut self) -> Result<HashMap<u32, BundleAnalysis>, RepositoryError> {
        if self.dirty {
            return Err(RepositoryError::Dirty)
        }
        self.dirty = true;
        let mut usage = HashMap::new();
        for (id, bundle) in self.bundle_map.bundles() {
            let bundle = try!(self.bundles.get_bundle_info(&bundle).ok_or_else(|| IntegrityError::MissingBundle(bundle)));
            usage.insert(id, BundleAnalysis {
                chunk_usage: Bitmap::new(bundle.info.chunk_count),
                info: bundle.info.clone(),
                used_raw_size: 0
            });
        }
        let backups = try!(self.get_backups());
        let mut todo = VecDeque::new();
        for (_name, backup) in backups {
            todo.push_back(backup.root);
        }
        while let Some(chunks) = todo.pop_back() {
            if !try!(self.mark_used(&mut usage, &chunks)) {
                continue
            }
            let inode = try!(self.get_inode(&chunks));
            // Mark the content chunks as used
            match inode.data {
                None | Some(FileData::Inline(_)) => (),
                Some(FileData::ChunkedDirect(chunks)) => {
                    try!(self.mark_used(&mut usage, &chunks));
                },
                Some(FileData::ChunkedIndirect(chunks)) => {
                    if try!(self.mark_used(&mut usage, &chunks)) {
                        let chunk_data = try!(self.get_data(&chunks));
                        let chunks = ChunkList::read_from(&chunk_data);
                        try!(self.mark_used(&mut usage, &chunks));
                    }
                }
            }
            // Put children in todo
            if let Some(children) = inode.children {
                for (_name, chunks) in children {
                    todo.push_back(chunks);
                }
            }
        }
        self.dirty = false;
        Ok(usage)
    }

    #[inline]
    pub fn list_bundles(&self) -> Vec<&BundleInfo> {
        self.bundles.list_bundles()
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
