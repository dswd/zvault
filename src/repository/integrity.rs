use ::prelude::*;

use super::*;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use pbr::ProgressBar;


quick_error!{
    #[derive(Debug)]
    pub enum IntegrityError {
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
            display("Bundle {} does not contain the chunk {}", bundle, chunk)
        }
        RemoteBundlesNotInMap {
            description("Remote bundles missing from map")
        }
        MapContainsDuplicates {
            description("Map contains duplicates")
        }
        BrokenInode(path: PathBuf, err: Box<RepositoryError>) {
            cause(err)
            description("Broken inode")
            display("Broken inode: {:?}\n\tcaused by: {}", path, err)
        }
        MissingInodeData(path: PathBuf, err: Box<RepositoryError>) {
            cause(err)
            description("Missing inode data")
            display("Missing inode data in: {:?}\n\tcaused by: {}", path, err)
        }
    }
}

impl Repository {
    fn check_index_chunks(&self) -> Result<(), RepositoryError> {
        let mut count = 0;
        let mut progress = ProgressBar::new(self.index.len() as u64);
        progress.message("checking index: ");
        progress.set_max_refresh_rate(Some(Duration::from_millis(100)));
        let res = self.index.walk(|_hash, location| {
            // Lookup bundle id from map
            let bundle_id = try!(self.get_bundle_id(location.bundle));
            // Get bundle object from bundledb
            let bundle = if let Some(bundle) = self.bundles.get_bundle_info(&bundle_id) {
                bundle
            } else {
                return Err(IntegrityError::MissingBundle(bundle_id.clone()).into())
            };
            // Get chunk from bundle
            if bundle.info.chunk_count <= location.chunk as usize {
                return Err(IntegrityError::NoSuchChunk(bundle_id.clone(), location.chunk).into())
            }
            count += 1;
            if count % 1000 == 0 {
                progress.set(count);
            }
            Ok(())
        });
        progress.finish_print("checking index: done.");
        res
    }

    fn check_chunks(&self, checked: &mut Bitmap, chunks: &[Chunk]) -> Result<bool, RepositoryError> {
        let mut new = false;
        for &(hash, _len) in chunks {
            if let Some(pos) = self.index.pos(&hash) {
                new |= !checked.get(pos);
                checked.set(pos);
            } else {
                return Err(IntegrityError::MissingChunk(hash).into())
            }
        }
        Ok(new)
    }

    fn check_inode_contents(&mut self, inode: &Inode, checked: &mut Bitmap) -> Result<(), RepositoryError> {
        match inode.data {
            None | Some(FileData::Inline(_)) => (),
            Some(FileData::ChunkedDirect(ref chunks)) => {
                try!(self.check_chunks(checked, chunks));
            },
            Some(FileData::ChunkedIndirect(ref chunks)) => {
                if try!(self.check_chunks(checked, chunks)) {
                    let chunk_data = try!(self.get_data(&chunks));
                    let chunks = ChunkList::read_from(&chunk_data);
                    try!(self.check_chunks(checked, &chunks));
                }
            }
        }
        Ok(())
    }

    fn check_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap) -> Result<(), RepositoryError> {
        let mut todo = VecDeque::new();
        todo.push_back((path, ChunkList::from(chunks.to_vec())));
        while let Some((path, chunks)) = todo.pop_front() {
            match self.check_chunks(checked, &chunks) {
                Ok(false) => continue, // checked this chunk list before
                Ok(true) => (),
                Err(err) => return Err(IntegrityError::BrokenInode(path, Box::new(err)).into())
            }
            let inode = try!(self.get_inode(&chunks));
            // Mark the content chunks as used
            if let Err(err) = self.check_inode_contents(&inode, checked) {
                return Err(IntegrityError::MissingInodeData(path, Box::new(err)).into())
            }
            // Put children in todo
            if let Some(children) = inode.children {
                for (name, chunks) in children {
                    todo.push_back((path.join(name), chunks));
                }
            }
        }
        Ok(())
    }

    #[inline]
    pub fn check_backup(&mut self, backup: &Backup) -> Result<(), RepositoryError> {
        info!("Checking backup...");
        let mut checked = Bitmap::new(self.index.capacity());
        self.check_subtree(Path::new("").to_path_buf(), &backup.root, &mut checked)
    }

    pub fn check_inode(&mut self, inode: &Inode, path: &Path) -> Result<(), RepositoryError> {
        info!("Checking inode...");
        let mut checked = Bitmap::new(self.index.capacity());
        try!(self.check_inode_contents(inode, &mut checked));
        if let Some(ref children) = inode.children {
            for chunks in children.values() {
                try!(self.check_subtree(path.to_path_buf(), chunks, &mut checked))
            }
        }
        Ok(())
    }

    pub fn check_backups(&mut self) -> Result<(), RepositoryError> {
        info!("Checking backups...");
        let mut checked = Bitmap::new(self.index.capacity());
        let backup_map = match self.get_backups() {
            Ok(backup_map) => backup_map,
            Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
                warn!("Some backups could not be read, ignoring them");
                backup_map
            },
            Err(err) => return Err(err)
        };
        for (name, backup) in ProgressIter::new("ckecking backups", backup_map.len(), backup_map.into_iter()) {
            let path = name+"::";
            try!(self.check_subtree(Path::new(&path).to_path_buf(), &backup.root, &mut checked));
        }
        Ok(())
    }

    pub fn check_repository(&mut self) -> Result<(), RepositoryError> {
        info!("Checking repository integrity...");
        for (_id, bundle_id) in self.bundle_map.bundles() {
            if self.bundles.get_bundle_info(&bundle_id).is_none() {
                return Err(IntegrityError::MissingBundle(bundle_id).into())
            }
        }
        if self.bundle_map.len() < self.bundles.len() {
            return Err(IntegrityError::RemoteBundlesNotInMap.into())
        }
        if self.bundle_map.len() > self.bundles.len() {
            return Err(IntegrityError::MapContainsDuplicates.into())
        }
        Ok(())
    }

    pub fn rebuild_bundle_map(&mut self) -> Result<(), RepositoryError> {
        info!("Rebuilding bundle map from bundles");
        self.bundle_map = BundleMap::create();
        for bundle in self.bundles.list_bundles() {
            let bundle_id = match bundle.mode {
                BundleMode::Data => self.next_data_bundle,
                BundleMode::Meta => self.next_meta_bundle
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
        info!("Rebuilding index from bundles");
        self.index.clear();
        for (num, id) in self.bundle_map.bundles() {
            let chunks = try!(self.bundles.get_chunk_list(&id));
            for (i, (hash, _len)) in chunks.into_inner().into_iter().enumerate() {
                try!(self.index.set(&hash, &Location{bundle: num as u32, chunk: i as u32}));
            }
        }
        Ok(())
    }

    #[inline]
    pub fn check_index(&mut self, repair: bool) -> Result<(), RepositoryError> {
        if repair {
            try!(self.write_mode());
        }
        info!("Checking index integrity...");
        if let Err(err) = self.index.check() {
            if repair {
                warn!("Problem detected: index was corrupted\n\tcaused by: {}", err);
                return self.rebuild_index();
            } else {
                return Err(err.into())
            }
        }
        info!("Checking index entries...");
        if let Err(err) = self.check_index_chunks() {
            if repair {
                warn!("Problem detected: index entries were inconsistent\n\tcaused by: {}", err);
                return self.rebuild_index();
            } else {
                return Err(err.into())
            }
        }
        Ok(())
    }

    #[inline]
    pub fn check_bundles(&mut self, full: bool, repair: bool) -> Result<(), RepositoryError> {
        if repair {
            try!(self.write_mode());
        }
        info!("Checking bundle integrity...");
        if try!(self.bundles.check(full, repair)) {
            // Some bundles got repaired
            try!(self.bundles.finish_uploads());
            try!(self.rebuild_bundle_map());
            try!(self.rebuild_index());
        }
        Ok(())
    }
}
