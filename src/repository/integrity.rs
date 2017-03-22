use ::prelude::*;

use super::metadata::FileContents;

use std::collections::VecDeque;


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
    }
}

impl Repository {
    fn check_index_chunks(&self) -> Result<(), RepositoryError> {
        let mut pos = 0;
        loop {
            pos = if let Some(pos) = self.index.next_entry(pos) {
                pos
            } else {
                break
            };
            let entry = self.index.get_entry(pos).unwrap();
            // Lookup bundle id from map
            let bundle_id = try!(self.get_bundle_id(entry.data.bundle));
            // Get bundle object from bundledb
            let bundle = if let Some(bundle) = self.bundles.get_bundle_info(&bundle_id) {
                bundle
            } else {
                return Err(RepositoryIntegrityError::MissingBundle(bundle_id.clone()).into())
            };
            // Get chunk from bundle
            if bundle.chunk_count <= entry.data.chunk as usize {
                return Err(RepositoryIntegrityError::NoSuchChunk(bundle_id.clone(), entry.data.chunk).into())
            }
            pos += 1;
        }
        Ok(())
    }

    fn check_repository(&self) -> Result<(), RepositoryError> {
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

    fn check_chunks(&self, checked: &mut Bitmap, chunks: &[Chunk]) -> Result<bool, RepositoryError> {
        let mut new = false;
        for &(hash, _len) in chunks {
            if let Some(pos) = self.index.pos(&hash) {
                new |= !checked.get(pos);
                checked.set(pos);
            } else {
                return Err(RepositoryIntegrityError::MissingChunk(hash).into())
            }
        }
        Ok(new)
    }

    fn check_inode_contents(&mut self, inode: &Inode, checked: &mut Bitmap) -> Result<(), RepositoryError> {
        match inode.contents {
            Some(FileContents::ChunkedDirect(ref chunks)) => {
                try!(self.check_chunks(checked, chunks));
            },
            Some(FileContents::ChunkedIndirect(ref chunks)) => {
                if try!(self.check_chunks(checked, chunks)) {
                    let chunk_data = try!(self.get_data(&chunks));
                    let chunks = ChunkList::read_from(&chunk_data);
                    try!(self.check_chunks(checked, &chunks));
                }
            }
            _ => ()
        }
        Ok(())
    }

    fn check_subtree(&mut self, chunks: &[Chunk], checked: &mut Bitmap) -> Result<(), RepositoryError> {
        let mut todo = VecDeque::new();
        todo.push_back(ChunkList::from(chunks.to_vec()));
        while let Some(chunks) = todo.pop_front() {
            if !try!(self.check_chunks(checked, &chunks)) {
                continue
            }
            let inode = try!(self.get_inode(&chunks));
            // Mark the content chunks as used
            try!(self.check_inode_contents(&inode, checked));
            // Put children in todo
            if let Some(children) = inode.children {
                for (_name, chunks) in children {
                    todo.push_back(chunks);
                }
            }
        }
        Ok(())
    }

    pub fn check_backup(&mut self, backup: &Backup) -> Result<(), RepositoryError> {
        let mut checked = Bitmap::new(self.index.capacity());
        self.check_subtree(&backup.root, &mut checked)
    }

    pub fn check_inode(&mut self, inode: &Inode) -> Result<(), RepositoryError> {
        let mut checked = Bitmap::new(self.index.capacity());
        try!(self.check_inode_contents(inode, &mut checked));
        if let Some(ref children) = inode.children {
            for chunks in children.values() {
                try!(self.check_subtree(chunks, &mut checked))
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
        for (_name, backup) in backup_map {
            try!(self.check_subtree(&backup.root, &mut checked));
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
