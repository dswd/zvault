use ::prelude::*;

use super::*;

use std::collections::{VecDeque, HashSet};


impl BackupRepository {
    fn mark_used(
        &self,
        bundles: &mut HashMap<u32, BundleAnalysis>,
        chunks: &[Chunk],
    ) -> Result<bool, RepositoryError> {
        let mut new = false;
        for &(hash, len) in chunks {
            if let Some(pos) = self.repo.get_chunk_location(hash) {
                let bundle = pos.bundle;
                if let Some(bundle) = bundles.get_mut(&bundle) {
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
        if self.repo.is_dirty() {
            return Err(RepositoryError::Dirty);
        }
        try!(self.repo.set_dirty());
        let mut usage = HashMap::new();
        for (id, bundle) in try!(self.repo.get_bundle_map()) {
            usage.insert(
                id,
                BundleAnalysis {
                    chunk_usage: Bitmap::new(bundle.chunk_count),
                    info: bundle.clone(),
                    used_raw_size: 0
                }
            );
        }
        let backups = try!(self.get_all_backups());
        let mut todo = VecDeque::new();
        for (_name, backup) in backups {
            todo.push_back(backup.root);
        }
        while let Some(chunks) = todo.pop_back() {
            if !try!(self.mark_used(&mut usage, &chunks)) {
                continue;
            }
            let inode = try!(self.get_inode(&chunks));
            // Mark the content chunks as used
            match inode.data {
                None |
                Some(FileData::Inline(_)) => (),
                Some(FileData::ChunkedDirect(chunks)) => {
                    try!(self.mark_used(&mut usage, &chunks));
                }
                Some(FileData::ChunkedIndirect(chunks)) => {
                    if try!(self.mark_used(&mut usage, &chunks)) {
                        let chunk_data = try!(self.get_data(&chunks));
                        let chunks = ChunkList::read_from(&chunk_data);
                        try!(self.mark_used(&mut usage, &chunks));
                    }
                }
            }
            // Put children in to do
            if let Some(children) = inode.children {
                for (_name, chunks) in children {
                    todo.push_back(chunks);
                }
            }
        }
        self.repo.set_clean();
        Ok(usage)
    }

    pub fn vacuum(
        &mut self,
        ratio: f32,
        combine: bool,
        force: bool,
    ) -> Result<(), RepositoryError> {
        try!(self.repo.flush());
        tr_info!("Locking repository");
        try!(self.repo.write_mode());
        let _lock = try!(self.repo.lock(true));
        // analyze_usage will set the dirty flag
        tr_info!("Analyzing chunk usage");
        let usage = try!(self.analyze_usage());
        let mut data_total = 0;
        let mut data_used = 0;
        for bundle in usage.values() {
            data_total += bundle.info.encoded_size;
            data_used += bundle.get_used_size();
        }
        tr_info!(
            "Usage: {} of {}, {:.1}%",
            to_file_size(data_used as u64),
            to_file_size(data_total as u64),
            data_used as f32 / data_total as f32 * 100.0
        );
        let mut rewrite_bundles = HashSet::new();
        let mut reclaim_space = 0;
        let mut rewrite_data = 0;
        for (id, bundle) in &usage {
            if bundle.get_usage_ratio() <= ratio {
                rewrite_bundles.insert(*id);
                reclaim_space += bundle.get_unused_size();
                rewrite_data += bundle.get_used_size();
            }
        }
        if combine {
            let mut small_meta = vec![];
            let mut small_data = vec![];
            for (id, bundle) in &usage {
                if bundle.info.encoded_size * 4 < self.repo.get_config().bundle_size {
                    match bundle.info.mode {
                        BundleMode::Meta => small_meta.push(*id),
                        BundleMode::Data => small_data.push(*id),
                    }
                }
            }
            if small_meta.len() >= 2 {
                for bundle in small_meta {
                    rewrite_bundles.insert(bundle);
                }
            }
            if small_data.len() >= 2 {
                for bundle in small_data {
                    rewrite_bundles.insert(bundle);
                }
            }
        }
        tr_info!(
            "Reclaiming about {} by rewriting {} bundles ({})",
            to_file_size(reclaim_space as u64),
            rewrite_bundles.len(),
            to_file_size(rewrite_data as u64)
        );
        if !force {
            self.repo.set_clean();
            return Ok(());
        }
        let rewrite_bundles: Vec<_> = rewrite_bundles.into_iter().collect();
        try!(self.repo.rewrite_bundles(&rewrite_bundles, &usage));
        self.repo.set_clean();
        Ok(())
    }
}