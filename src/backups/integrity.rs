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


pub trait RepositoryIntegrityIO {
    fn check_inode_contents(&mut self, inode: &Inode, checked: &mut Bitmap, lock: &OnlineMode
    ) -> Result<(), RepositoryError>;

    fn check_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap,
        lock: &OnlineMode
    ) -> Result<(), RepositoryError>;

    fn check_and_repair_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap,
        lock: &BackupMode
    ) -> Result<Option<ChunkList>, RepositoryError>;

    fn evacuate_broken_backup(&self, name: &str, lock: &BackupMode) -> Result<(), RepositoryError>;

    fn check_backup_inode(&mut self, name: &str, backup: &mut BackupFile, path: &Path,
        lock: &OnlineMode
    ) -> Result<(), RepositoryError>;

    fn check_and_repair_backup_inode(&mut self, name: &str, backup: &mut BackupFile, path: &Path,
        lock: &BackupMode,
    ) -> Result<(), RepositoryError>;

    fn check_backup(&mut self, name: &str, backup: &mut BackupFile, lock: &OnlineMode
    ) -> Result<(), RepositoryError>;

    fn check_and_repair_backup(&mut self, name: &str, backup: &mut BackupFile, lock: &BackupMode
    ) -> Result<(), RepositoryError>;

    fn check_backups(&mut self, lock: &OnlineMode) -> Result<(), RepositoryError>;

    fn check_and_repair_backups(&mut self, lock: &BackupMode) -> Result<(), RepositoryError>;
}


impl RepositoryIntegrityIO for Repository {
    fn check_inode_contents(&mut self, inode: &Inode, checked: &mut Bitmap, lock: &OnlineMode
    ) -> Result<(), RepositoryError> {
        match inode.data {
            None |
            Some(FileData::Inline(_)) => (),
            Some(FileData::ChunkedDirect(ref chunks)) => {
                try!(self.mark_chunks(checked, chunks, true));
            }
            Some(FileData::ChunkedIndirect(ref chunks)) => {
                if try!(self.mark_chunks(checked, chunks, false)) {
                    let chunk_data = try!(self.get_data(chunks, lock));
                    let chunks2 = ChunkList::read_from(&chunk_data);
                    try!(self.mark_chunks(checked, &chunks2, true));
                    try!(self.mark_chunks(checked, chunks, true));
                }
            }
        }
        Ok(())
    }

    fn check_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap,
        lock: &OnlineMode,
    ) -> Result<(), RepositoryError> {
        let mut modified = false;
        match self.mark_chunks(checked, chunks, false) {
            Ok(false) => return Ok(()),
            Ok(true) => (),
            Err(err) => return Err(InodeIntegrityError::BrokenInode(path, Box::new(err)).into()),
        }
        let mut inode = try!(self.get_inode(chunks, lock));
        // Mark the content chunks as used
        if let Err(err) = self.check_inode_contents(&inode, checked, lock) {
            return Err(InodeIntegrityError::MissingInodeData(path, Box::new(err)).into());
        }
        // Put children in to do
        if let Some(ref mut children) = inode.children {
            for (name, chunks) in children.iter_mut() {
                try!(self.check_subtree(path.join(name), chunks, checked, lock));
            }
        }
        try!(self.mark_chunks(checked, chunks, true));
        Ok(())
    }

    fn check_and_repair_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap,
        lock: &BackupMode,
    ) -> Result<Option<ChunkList>, RepositoryError> {
        let mut modified = false;
        match self.mark_chunks(checked, chunks, false) {
            Ok(false) => return Ok(None),
            Ok(true) => (),
            Err(err) => return Err(InodeIntegrityError::BrokenInode(path, Box::new(err)).into()),
        }
        let mut inode = try!(self.get_inode(chunks, lock.as_online()));
        // Mark the content chunks as used
        if let Err(err) = self.check_inode_contents(&inode, checked, lock.as_online()) {
            tr_warn!(
                "Problem detected: data of {:?} is corrupt\n\tcaused by: {}",
                path,
                err
            );
            tr_info!("Removing inode data");
            inode.data = Some(FileData::Inline(vec![].into()));
            inode.size = 0;
            modified = true;
        }
        // Put children in to do
        if let Some(ref mut children) = inode.children {
            let mut removed = vec![];
            for (name, chunks) in children.iter_mut() {
                match self.check_and_repair_subtree(path.join(name), chunks, checked, lock) {
                    Ok(None) => (),
                    Ok(Some(c)) => {
                        *chunks = c;
                        modified = true;
                    }
                    Err(err) => {
                        tr_warn!(
                            "Problem detected: inode {:?} is corrupt\n\tcaused by: {}",
                            path.join(name),
                            err
                        );
                        tr_info!("Removing broken inode from backup");
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
            Ok(Some(try!(self.put_inode(&inode, lock))))
        } else {
            try!(self.mark_chunks(checked, chunks, true));
            Ok(None)
        }
    }


    fn evacuate_broken_backup(&self, name: &str, lock: &BackupMode) -> Result<(), RepositoryError> {
        tr_warn!(
            "The backup {} was corrupted and needed to be modified.",
            name
        );
        let src = self.get_layout().backup_path(name);
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

    fn check_backup_inode(&mut self, name: &str, backup: &mut BackupFile, path: &Path,
        lock: &OnlineMode
    ) -> Result<(), RepositoryError> {
        tr_info!("Checking inode...");
        let mut checked = self.get_chunk_marker();
        let mut inodes = try!(self.get_backup_path(backup, path, lock));
        let mut inode = inodes.pop().unwrap();
        let mut modified = false;
        if let Err(err) = self.check_inode_contents(&inode, &mut checked, lock) {
            return Err(
                InodeIntegrityError::MissingInodeData(path.to_path_buf(), Box::new(err)).into()
            );
        }
        if let Some(ref mut children) = inode.children {
            for (name, chunks) in children.iter_mut() {
                try!(self.check_subtree(path.join(name), chunks, &mut checked, lock));
            }
        }
        Ok(())
    }

    fn check_and_repair_backup_inode(&mut self, name: &str, backup: &mut BackupFile, path: &Path,
        lock: &BackupMode
    ) -> Result<(), RepositoryError> {
        tr_info!("Checking inode...");
        let mut checked = self.get_chunk_marker();
        let mut inodes = try!(self.get_backup_path(backup, path, lock.as_online()));
        let mut inode = inodes.pop().unwrap();
        let mut modified = false;
        if let Err(err) = self.check_inode_contents(&inode, &mut checked, lock.as_online()) {
            tr_warn!(
                "Problem detected: data of {:?} is corrupt\n\tcaused by: {}",
                path,
                err
            );
            tr_info!("Removing inode data");
            inode.data = Some(FileData::Inline(vec![].into()));
            inode.size = 0;
            modified = true;
        }
        if let Some(ref mut children) = inode.children {
            let mut removed = vec![];
            for (name, chunks) in children.iter_mut() {
                match self.check_and_repair_subtree(path.join(name), chunks, &mut checked, lock) {
                    Ok(None) => (),
                    Ok(Some(c)) => {
                        *chunks = c;
                        modified = true;
                    }
                    Err(err) => {
                        tr_warn!(
                            "Problem detected: inode {:?} is corrupt\n\tcaused by: {}",
                            path.join(name),
                            err
                        );
                        tr_info!("Removing broken inode from backup");
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
            let mut chunks = try!(self.put_inode(&inode, lock));
            while let Some(mut parent) = inodes.pop() {
                parent.children.as_mut().unwrap().insert(inode.name, chunks);
                inode = parent;
                chunks = try!(self.put_inode(&inode, lock));
            }
            try!(self.flush(lock));
            backup.root = chunks;
            backup.modified = true;
            try!(self.evacuate_broken_backup(name, lock));
            try!(self.save_backup(backup, name, lock));
        }
        Ok(())
    }

    #[inline]
    fn check_backup(&mut self, name: &str, backup: &mut BackupFile, lock: &OnlineMode,
    ) -> Result<(), RepositoryError> {
        tr_info!("Checking backup...");
        let mut checked = self.get_chunk_marker();
        try!(self.check_subtree(Path::new("").to_path_buf(), &backup.root, &mut checked, lock));
        Ok(())
    }

    #[inline]
    fn check_and_repair_backup(&mut self, name: &str, backup: &mut BackupFile, lock: &BackupMode,
    ) -> Result<(), RepositoryError> {
        tr_info!("Checking backup...");
        let mut checked = self.get_chunk_marker();
        match self.check_and_repair_subtree(Path::new("").to_path_buf(),
            &backup.root, &mut checked, lock
        ) {
            Ok(None) => (),
            Ok(Some(chunks)) => {
                try!(self.flush(lock));
                backup.root = chunks;
                backup.modified = true;
                try!(self.evacuate_broken_backup(name, lock));
                try!(self.save_backup(backup, name, lock));
            }
            Err(err) => {
                tr_warn!(
                    "The root of the backup {} has been corrupted\n\tcaused by: {}",
                    name,
                    err
                );
                try!(self.evacuate_broken_backup(name, lock));
            }
        }
        Ok(())
    }

    fn check_backups(&mut self, lock: &OnlineMode) -> Result<(), RepositoryError> {
        tr_info!("Checking backups...");
        let mut checked = self.get_chunk_marker();
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
            try!(self.check_subtree(Path::new(&path).to_path_buf(), &backup.root,
                &mut checked, lock));
        }
        Ok(())
    }


    fn check_and_repair_backups(&mut self, lock: &BackupMode) -> Result<(), RepositoryError> {
        tr_info!("Checking backups...");
        let mut checked = self.get_chunk_marker();
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
            match self.check_and_repair_subtree(
                Path::new(&path).to_path_buf(),
                &backup.root,
                &mut checked,
                lock
            ) {
                Ok(None) => (),
                Ok(Some(chunks)) => {
                    try!(self.flush(lock));
                    backup.root = chunks;
                    backup.modified = true;
                    try!(self.evacuate_broken_backup(&name, lock));
                    try!(self.save_backup(&backup, &name, lock));
                }
                Err(err) => {
                    tr_warn!(
                        "The root of the backup {} has been corrupted\n\tcaused by: {}",
                        name,
                        err
                    );
                    try!(self.evacuate_broken_backup(&name, lock));
                }
            }
        }
        Ok(())
    }
}