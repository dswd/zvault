use ::prelude::*;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};


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
        InvalidNextBundleId {
            description("Invalid next bundle id")
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
        self.index.walk(|_hash, location| {
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
            Ok(())
        })
    }

    fn check_repository(&self) -> Result<(), RepositoryError> {
        if self.next_data_bundle == self.next_meta_bundle {
            return Err(IntegrityError::InvalidNextBundleId.into())
        }
        if self.bundle_map.get(self.next_data_bundle).is_some() {
            return Err(IntegrityError::InvalidNextBundleId.into())
        }
        if self.bundle_map.get(self.next_meta_bundle).is_some() {
            return Err(IntegrityError::InvalidNextBundleId.into())
        }
        Ok(())
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

    pub fn check_backup(&mut self, backup: &Backup) -> Result<(), RepositoryError> {
        let mut checked = Bitmap::new(self.index.capacity());
        self.check_subtree(Path::new("").to_path_buf(), &backup.root, &mut checked)
    }

    pub fn check_inode(&mut self, inode: &Inode, path: &Path) -> Result<(), RepositoryError> {
        let mut checked = Bitmap::new(self.index.capacity());
        try!(self.check_inode_contents(inode, &mut checked));
        if let Some(ref children) = inode.children {
            for chunks in children.values() {
                try!(self.check_subtree(path.to_path_buf(), chunks, &mut checked))
            }
        }
        Ok(())
    }

    fn check_backups(&mut self) -> Result<(), RepositoryError> {
        let mut checked = Bitmap::new(self.index.capacity());
        let backup_map = match self.get_backups() {
            Ok(backup_map) => backup_map,
            Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
                warn!("Some backups could not be read, ignoring them");
                backup_map
            },
            Err(err) => return Err(err)
        };
        for (name, backup) in backup_map {
            let path = name+"::";
            try!(self.check_subtree(Path::new(&path).to_path_buf(), &backup.root, &mut checked));
        }
        Ok(())
    }

    pub fn check(&mut self, full: bool) -> Result<(), RepositoryError> {
        try!(self.flush());
        info!("Checking bundle integrity...");
        try!(self.bundles.check(full));
        info!("Checking index integrity...");
        try!(self.index.check());
        try!(self.check_index_chunks());
        info!("Checking backup integrity...");
        try!(self.check_backups());
        info!("Checking repository integrity...");
        try!(self.check_repository());
        Ok(())
    }
}
