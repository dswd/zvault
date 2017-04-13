use ::prelude::*;

use super::*;

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

    fn check_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap, repair: bool) -> Result<Option<ChunkList>, RepositoryError> {
        let mut modified = false;
        match self.check_chunks(checked, chunks) {
            Ok(false) => return Ok(None),
            Ok(true) => (),
            Err(err) => return Err(IntegrityError::BrokenInode(path, Box::new(err)).into())
        }
        let mut inode = try!(self.get_inode(chunks));
        // Mark the content chunks as used
        if let Err(err) = self.check_inode_contents(&inode, checked) {
            if repair {
                warn!("Problem detected: data of {:?} is corrupt\n\tcaused by: {}", path, err);
                info!("Removing inode data");
                inode.data = Some(FileData::Inline(vec![].into()));
                inode.size = 0;
                modified = true;
            } else {
                return Err(IntegrityError::MissingInodeData(path, Box::new(err)).into())
            }
        }
        // Put children in todo
        if let Some(ref mut children) = inode.children {
            let mut removed = vec![];
            for (name, chunks) in children.iter_mut() {
                match self.check_subtree(path.join(name), chunks, checked, repair) {
                    Ok(None) => (),
                    Ok(Some(c)) => {
                        *chunks = c;
                        modified = true;
                    },
                    Err(err) => {
                        warn!("Problem detected: inode {:?} is corrupt\n\tcaused by: {}", path.join(name), err);
                        info!("Removing broken inode from backup");
                        removed.push(name.to_string());
                        modified = true;
                    }
                }
            }
            for name in removed {
                children.remove(&name);
            }
        }
        if modified {
            Ok(Some(try!(self.put_inode(&inode))))
        } else {
            Ok(None)
        }
    }

    fn evacuate_broken_backup(&self, name: &str) -> Result<(), RepositoryError> {
        warn!("The backup {} was corrupted and needed to be modified.", name);
        let src = self.layout.backup_path(name);
        let dst = src.with_extension("backup.broken");
        if fs::rename(&src, &dst).is_err() {
            try!(fs::copy(&src, &dst));
            try!(fs::remove_file(&src));
        }
        info!("The original backup was renamed to {:?}", dst);
        Ok(())
    }

    #[inline]
    pub fn check_backup(&mut self, name: &str, backup: &mut Backup, repair: bool) -> Result<(), RepositoryError> {
        let _lock = if repair {
            try!(self.write_mode());
            Some(self.lock(false))
        } else {
            None
        };
        info!("Checking backup...");
        let mut checked = Bitmap::new(self.index.capacity());
        match self.check_subtree(Path::new("").to_path_buf(), &backup.root, &mut checked, repair) {
            Ok(None) => (),
            Ok(Some(chunks)) => {
                try!(self.flush());
                backup.root = chunks;
                backup.modified = true;
                try!(self.evacuate_broken_backup(&name));
                try!(self.save_backup(&backup, &name));
            },
            Err(err) => {
                warn!("The root of the backup {} has been corrupted\n\tcaused by: {}", name, err);
                try!(self.evacuate_broken_backup(&name));
            }
        }
        Ok(())
    }

    pub fn check_backup_inode(&mut self, name: &str, backup: &mut Backup, path: &Path, repair: bool) -> Result<(), RepositoryError> {
        let _lock = if repair {
            try!(self.write_mode());
            Some(self.lock(false))
        } else {
            None
        };
        info!("Checking inode...");
        let mut checked = Bitmap::new(self.index.capacity());
        let mut inodes = try!(self.get_backup_path(&backup, path));
        let mut inode = inodes.pop().unwrap();
        let mut modified = false;
        if let Err(err) = self.check_inode_contents(&inode, &mut checked) {
            if repair {
                warn!("Problem detected: data of {:?} is corrupt\n\tcaused by: {}", path, err);
                info!("Removing inode data");
                inode.data = Some(FileData::Inline(vec![].into()));
                inode.size = 0;
                modified = true;
            } else {
                return Err(IntegrityError::MissingInodeData(path.to_path_buf(), Box::new(err)).into())
            }
        }
        if let Some(ref mut children) = inode.children {
            let mut removed = vec![];
            for (name, chunks) in children.iter_mut() {
                match self.check_subtree(path.join(name), chunks, &mut checked, repair) {
                    Ok(None) => (),
                    Ok(Some(c)) => {
                        *chunks = c;
                        modified = true;
                    },
                    Err(err) => {
                        warn!("Problem detected: inode {:?} is corrupt\n\tcaused by: {}", path.join(name), err);
                        info!("Removing broken inode from backup");
                        removed.push(name.to_string());
                        modified = true;
                    }
                }
            }
            for name in removed {
                children.remove(&name);
            }
        }
        let mut chunks = try!(self.put_inode(&inode));
        while let Some(mut parent) = inodes.pop() {
            parent.children.as_mut().unwrap().insert(inode.name, chunks);
            inode = parent;
            chunks = try!(self.put_inode(&inode));
        }
        if modified {
            try!(self.flush());
            backup.root = chunks;
            backup.modified = true;
            try!(self.evacuate_broken_backup(&name));
            try!(self.save_backup(&backup, &name));
        }
        Ok(())
    }

    pub fn check_backups(&mut self, repair: bool) -> Result<(), RepositoryError> {
        let _lock = if repair {
            try!(self.write_mode());
            Some(self.lock(false))
        } else {
            None
        };
        info!("Checking backups...");
        let mut checked = Bitmap::new(self.index.capacity());
        let backup_map = match self.get_all_backups() {
            Ok(backup_map) => backup_map,
            Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
                warn!("Some backups could not be read, ignoring them");
                backup_map
            },
            Err(err) => return Err(err)
        };
        for (name, mut backup) in ProgressIter::new("ckecking backups", backup_map.len(), backup_map.into_iter()) {
            let path = format!("{}::", name);
            match self.check_subtree(Path::new(&path).to_path_buf(), &backup.root, &mut checked, repair) {
                Ok(None) => (),
                Ok(Some(chunks)) => {
                    try!(self.flush());
                    backup.root = chunks;
                    backup.modified = true;
                    try!(self.evacuate_broken_backup(&name));
                    try!(self.save_backup(&backup, &name));
                },
                Err(err) => {
                    warn!("The root of the backup {} has been corrupted\n\tcaused by: {}", name, err);
                    try!(self.evacuate_broken_backup(&name));
                }
            }
        }
        Ok(())
    }

    pub fn check_repository(&mut self, repair: bool) -> Result<(), RepositoryError> {
        info!("Checking repository integrity...");
        let mut rebuild = false;
        for (_id, bundle_id) in self.bundle_map.bundles() {
            if self.bundles.get_bundle_info(&bundle_id).is_none() {
                if repair {
                    warn!("Problem detected: bundle map contains unknown bundle {}", bundle_id);
                    rebuild = true;
                } else {
                    return Err(IntegrityError::MissingBundle(bundle_id).into())
                }
            }
        }
        if self.bundle_map.len() < self.bundles.len() {
            if repair {
                warn!("Problem detected: bundle map does not contain all remote bundles");
                rebuild = true;
            } else {
                return Err(IntegrityError::RemoteBundlesNotInMap.into())
            }
        }
        if self.bundle_map.len() > self.bundles.len() {
            if repair {
                warn!("Problem detected: bundle map contains bundles multiple times");
                rebuild = true;
            } else {
                return Err(IntegrityError::MapContainsDuplicates.into())
            }
        }
        if rebuild {
            try!(self.rebuild_bundle_map());
            try!(self.rebuild_index());
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
            warn!("Some bundles have been rewritten, please remove the broken bundles manually.");
            try!(self.bundles.finish_uploads());
            try!(self.rebuild_bundle_map());
            try!(self.rebuild_index());
        }
        Ok(())
    }
}
