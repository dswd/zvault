use super::{Repository, RepositoryError, RepositoryIntegrityError};
use super::metadata::FileContents;

use std::collections::{HashMap, HashSet, VecDeque};

use ::bundle::BundleMode;
use ::util::*;


pub struct BundleUsage {
    pub used: Bitmap,
    pub mode: Bitmap,
    pub chunk_count: usize,
    pub total_size: usize,
    pub used_size: usize
}

impl Repository {
    fn mark_used(&self, bundles: &mut HashMap<u32, BundleUsage>, chunks: &[Chunk], mode: BundleMode) -> Result<bool, RepositoryError> {
        let mut new = false;
        for chunk in chunks {
            if let Some(pos) = self.index.get(&chunk.0) {
                if let Some(bundle) = bundles.get_mut(&pos.bundle) {
                    if !bundle.used.get(pos.chunk as usize) {
                        new = true;
                        bundle.used.set(pos.chunk as usize);
                        bundle.used_size += chunk.1 as usize;
                        if mode == BundleMode::Meta {
                            bundle.mode.set(pos.chunk as usize);
                        }
                    }
                }
            } else {
                return Err(RepositoryIntegrityError::MissingChunk(chunk.0).into());
            }
        }
        Ok(new)
    }

    pub fn analyze_usage(&mut self) -> Result<(HashMap<u32, BundleUsage>, bool), RepositoryError> {
        let mut usage = HashMap::new();
        for (id, bundle) in self.bundle_map.bundles() {
            usage.insert(id, BundleUsage {
                used: Bitmap::new(bundle.info.chunk_count),
                mode: Bitmap::new(bundle.info.chunk_count),
                chunk_count: bundle.info.chunk_count,
                total_size: bundle.info.raw_size,
                used_size: 0
            });
        }
        let (backups, some_failed) = try!(self.get_backups());
        for (_name, backup) in backups {
            let mut todo = VecDeque::new();
            todo.push_back(backup.root);
            while let Some(chunks) = todo.pop_front() {
                if !try!(self.mark_used(&mut usage, &chunks, BundleMode::Meta)) {
                    continue
                }
                let inode = try!(self.get_inode(&chunks));
                // Mark the content chunks as used
                match inode.contents {
                    Some(FileContents::ChunkedDirect(chunks)) => {
                        try!(self.mark_used(&mut usage, &chunks, BundleMode::Content));
                    },
                    Some(FileContents::ChunkedIndirect(chunks)) => {
                        if try!(self.mark_used(&mut usage, &chunks, BundleMode::Meta)) {
                            let chunk_data = try!(self.get_data(&chunks));
                            let chunks = ChunkList::read_from(&chunk_data);
                            try!(self.mark_used(&mut usage, &chunks, BundleMode::Content));
                        }
                    }
                    _ => ()
                }
                // Put children in todo
                if let Some(children) = inode.children {
                    for (_name, chunks) in children {
                        todo.push_back(chunks);
                    }
                }
            }
        }
        Ok((usage, some_failed))
    }

    fn delete_bundle(&mut self, id: u32) -> Result<(), RepositoryError> {
        if let Some(bundle) = self.bundle_map.remove(id) {
            try!(self.bundles.delete_bundle(&bundle.id()));
            Ok(())
        } else {
            Err(RepositoryIntegrityError::MissingBundleId(id).into())
        }
    }

    pub fn vacuum(&mut self, ratio: f32, force: bool) -> Result<(), RepositoryError> {
        try!(self.flush());
        info!("Analyzing chunk usage");
        let (usage, some_failed) = try!(self.analyze_usage());
        if some_failed {
            return Err(RepositoryError::UnsafeVacuum);
        }
        let total = usage.values().map(|b| b.total_size).sum::<usize>();
        let used = usage.values().map(|b| b.used_size).sum::<usize>();
        info!("Usage: {} of {}, {:.1}%", to_file_size(used as u64), to_file_size(total as u64), used as f32/total as f32*100.0);
        let mut rewrite_bundles = HashSet::new();
        let mut reclaim_space = 0;
        for (id, bundle) in &usage {
            if bundle.used_size as f32 / bundle.total_size as f32 <= ratio {
                rewrite_bundles.insert(*id);
                reclaim_space += bundle.total_size - bundle.used_size;
            }
        }
        info!("Reclaiming {} by rewriting {} bundles", to_file_size(reclaim_space as u64), rewrite_bundles.len());
        if !force {
            return Ok(())
        }
        for id in &rewrite_bundles {
            let bundle = &usage[id];
            let bundle_id = self.bundle_map.get(*id).unwrap().id();
            for chunk in 0..bundle.chunk_count {
                let data = try!(self.bundles.get_chunk(&bundle_id, chunk));
                let hash = self.config.hash.hash(&data);
                if !bundle.used.get(chunk) {
                    try!(self.index.delete(&hash));
                    continue
                }
                let mode = if bundle.mode.get(chunk) {
                    BundleMode::Meta
                } else {
                    BundleMode::Content
                };
                try!(self.put_chunk_override(mode, hash, &data));
            }
        }
        try!(self.flush());
        info!("Checking index");
        let mut pos = 0;
        loop {
            pos = if let Some(pos) = self.index.next_entry(pos) {
                pos
            } else {
                break
            };
            let entry = self.index.get_entry(pos).unwrap();
            if rewrite_bundles.contains(&entry.data.bundle) {
                panic!("Removed bundle is still referenced from index");
            }
            pos += 1;
        }
        info!("Deleting {} bundles", rewrite_bundles.len());
        for id in rewrite_bundles {
            try!(self.delete_bundle(id));
        }
        try!(self.bundle_map.save(self.path.join("bundles.map")));
        Ok(())
    }
}
