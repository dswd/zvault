use prelude::*;

use super::*;

use std::path::{Path, PathBuf};


quick_error!{
    #[derive(Debug)]
    pub enum InodeIntegrityError {
        BrokenInode(path: PathBuf, err: Box<RepositoryError>) {
            cause(err)
            description(tr!("Broken inode"))
            display("{}", tr_format!("Broken inode: {:?}\n\tcaused by: {}", path, err))
        }
        MissingInodeData(path: PathBuf, err: Box<RepositoryError>) {
            cause(err)
            description(tr!("Missing inode data"))
            display("{}", tr_format!("Missing inode data in: {:?}\n\tcaused by: {}", path, err))
        }
    }
}

impl BackupRepository {
    fn check_inode_contents(
        &mut self,
        inode: &Inode,
        checked: &mut Bitmap,
    ) -> Result<(), RepositoryError> {
        match inode.data {
            None |
            Some(FileData::Inline(_)) => (),
            Some(FileData::ChunkedDirect(ref chunks)) => {
                try!(self.repo.check_chunks(checked, chunks, true));
            }
            Some(FileData::ChunkedIndirect(ref chunks)) => {
                if try!(self.repo.check_chunks(checked, chunks, false)) {
                    let chunk_data = try!(self.get_data(chunks));
                    let chunks2 = ChunkList::read_from(&chunk_data);
                    try!(self.repo.check_chunks(checked, &chunks2, true));
                    try!(self.repo.check_chunks(checked, chunks, true));
                }
            }
        }
        Ok(())
    }

    fn check_subtree(
        &mut self,
        path: PathBuf,
        chunks: &[Chunk],
        checked: &mut Bitmap,
        repair: bool,
    ) -> Result<Option<ChunkList>, RepositoryError> {
        let mut modified = false;
        match self.repo.check_chunks(checked, chunks, false) {
            Ok(false) => return Ok(None),
            Ok(true) => (),
            Err(err) => return Err(InodeIntegrityError::BrokenInode(path, Box::new(err)).into()),
        }
        let mut inode = try!(self.get_inode(chunks));
        // Mark the content chunks as used
        if let Err(err) = self.check_inode_contents(&inode, checked) {
            if repair {
                tr_warn!(
                    "Problem detected: data of {:?} is corrupt\n\tcaused by: {}",
                    path,
                    err
                );
                tr_info!("Removing inode data");
                inode.data = Some(FileData::Inline(vec![].into()));
                inode.size = 0;
                modified = true;
            } else {
                return Err(InodeIntegrityError::MissingInodeData(path, Box::new(err)).into());
            }
        }
        // Put children in to do
        if let Some(ref mut children) = inode.children {
            let mut removed = vec![];
            for (name, chunks) in children.iter_mut() {
                match self.check_subtree(path.join(name), chunks, checked, repair) {
                    Ok(None) => (),
                    Ok(Some(c)) => {
                        *chunks = c;
                        modified = true;
                    }
                    Err(err) => {
                        if repair {
                            tr_warn!(
                                "Problem detected: inode {:?} is corrupt\n\tcaused by: {}",
                                path.join(name),
                                err
                            );
                            tr_info!("Removing broken inode from backup");
                            removed.push(name.to_string());
                            modified = true;
                        } else {
                            return Err(err);
                        }
                    }
                }
            }
            for name in removed {
                children.remove(&name);
            }
        }
        if modified {
            Ok(Some(try!(self.repo.put_inode(&inode))))
        } else {
            try!(self.repo.check_chunks(checked, chunks, true));
            Ok(None)
        }
    }

    fn evacuate_broken_backup(&self, name: &str) -> Result<(), RepositoryError> {
        tr_warn!(
            "The backup {} was corrupted and needed to be modified.",
            name
        );
        let src = self.layout.backup_path(name);
        let mut dst = src.with_extension("backup.broken");
        let mut num = 1;
        while dst.exists() {
            dst = src.with_extension(&format!("backup.{}.broken", num));
            num += 1;
        }
        if fs::rename(&src, &dst).is_err() {
            try!(fs::copy(&src, &dst));
            try!(fs::remove_file(&src));
        }
        tr_info!("The original backup was renamed to {:?}", dst);
        Ok(())
    }

    #[inline]
    pub fn check_backup(
        &mut self,
        name: &str,
        backup: &mut BackupFile,
        repair: bool,
    ) -> Result<(), RepositoryError> {
        let _lock = if repair {
            try!(self.repo.write_mode());
            Some(self.repo.lock(false))
        } else {
            None
        };
        tr_info!("Checking backup...");
        let mut checked = self.repo.get_chunk_marker();
        match self.check_subtree(
            Path::new("").to_path_buf(),
            &backup.root,
            &mut checked,
            repair
        ) {
            Ok(None) => (),
            Ok(Some(chunks)) => {
                try!(self.repo.flush());
                backup.root = chunks;
                backup.modified = true;
                try!(self.evacuate_broken_backup(name));
                try!(self.save_backup(backup, name));
            }
            Err(err) => {
                if repair {
                    tr_warn!(
                        "The root of the backup {} has been corrupted\n\tcaused by: {}",
                        name,
                        err
                    );
                    try!(self.evacuate_broken_backup(name));
                } else {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub fn check_backup_inode(
        &mut self,
        name: &str,
        backup: &mut BackupFile,
        path: &Path,
        repair: bool,
    ) -> Result<(), RepositoryError> {
        let _lock = if repair {
            try!(self.repo.write_mode());
            Some(self.repo.lock(false))
        } else {
            None
        };
        tr_info!("Checking inode...");
        let mut checked = self.repo.get_chunk_marker();
        let mut inodes = try!(self.get_backup_path(backup, path));
        let mut inode = inodes.pop().unwrap();
        let mut modified = false;
        if let Err(err) = self.check_inode_contents(&inode, &mut checked) {
            if repair {
                tr_warn!(
                    "Problem detected: data of {:?} is corrupt\n\tcaused by: {}",
                    path,
                    err
                );
                tr_info!("Removing inode data");
                inode.data = Some(FileData::Inline(vec![].into()));
                inode.size = 0;
                modified = true;
            } else {
                return Err(
                    InodeIntegrityError::MissingInodeData(path.to_path_buf(), Box::new(err)).into()
                );
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
                    }
                    Err(err) => {
                        if repair {
                            tr_warn!(
                                "Problem detected: inode {:?} is corrupt\n\tcaused by: {}",
                                path.join(name),
                                err
                            );
                            tr_info!("Removing broken inode from backup");
                            removed.push(name.to_string());
                            modified = true;
                        } else {
                            return Err(err);
                        }
                    }
                }
            }
            for name in removed {
                children.remove(&name);
            }
        }
        let mut chunks = try!(self.repo.put_inode(&inode));
        while let Some(mut parent) = inodes.pop() {
            parent.children.as_mut().unwrap().insert(inode.name, chunks);
            inode = parent;
            chunks = try!(self.repo.put_inode(&inode));
        }
        if modified {
            try!(self.repo.flush());
            backup.root = chunks;
            backup.modified = true;
            try!(self.evacuate_broken_backup(name));
            try!(self.save_backup(backup, name));
        }
        Ok(())
    }

    pub fn check_backups(&mut self, repair: bool) -> Result<(), RepositoryError> {
        let _lock = if repair {
            try!(self.repo.write_mode());
            Some(self.repo.lock(false))
        } else {
            None
        };
        tr_info!("Checking backups...");
        let mut checked = self.repo.get_chunk_marker();
        let backup_map = match self.get_all_backups() {
            Ok(backup_map) => backup_map,
            Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map,
                                                                                _failed))) => {
                tr_warn!("Some backups could not be read, ignoring them");
                backup_map
            }
            Err(err) => return Err(err),
        };
        for (name, mut backup) in
            ProgressIter::new(tr!("checking backups"), backup_map.len(), backup_map.into_iter())
            {
                let path = format!("{}::", name);
                match self.check_subtree(
                    Path::new(&path).to_path_buf(),
                    &backup.root,
                    &mut checked,
                    repair
                ) {
                    Ok(None) => (),
                    Ok(Some(chunks)) => {
                        try!(self.repo.flush());
                        backup.root = chunks;
                        backup.modified = true;
                        try!(self.evacuate_broken_backup(&name));
                        try!(self.save_backup(&backup, &name));
                    }
                    Err(err) => {
                        if repair {
                            tr_warn!(
                            "The root of the backup {} has been corrupted\n\tcaused by: {}",
                            name,
                            err
                        );
                            try!(self.evacuate_broken_backup(&name));
                        } else {
                            return Err(err);
                        }
                    }
                }
            }
        Ok(())
    }


    #[inline]
    pub fn check_bundles(&mut self, full: bool, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_bundles(full, repair)
    }

    pub fn check_repository(&mut self, repair: bool) -> Result<(), RepositoryError> {
        self.repo.check_repository(repair)
    }
}