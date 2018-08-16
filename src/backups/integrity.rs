use prelude::*;

use super::*;

use std::path::{Path, PathBuf};

pub use ::repository::ModuleIntegrityReport;


quick_error!{
    #[derive(Debug)]
    pub enum InodeIntegrityError {
        BackupRead(path: PathBuf, err: Box<RepositoryError>) {
            cause(err)
            description(tr!("Backup unreadable"))
            display("{}", tr_format!("Backup unreadable: {:?}\n\tcaused by: {}", path, err))
        }
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
        errors: &mut Vec<InodeIntegrityError>, lock: &OnlineMode
    );

    fn check_backup_inode(&mut self, inode: &Inode, path: &Path, lock: &OnlineMode
    ) -> ModuleIntegrityReport<InodeIntegrityError>;

    fn check_backup(&mut self, name: &str, backup: &mut BackupFile, lock: &OnlineMode
    ) -> ModuleIntegrityReport<InodeIntegrityError>;

    fn check_backups(&mut self, lock: &OnlineMode) -> ModuleIntegrityReport<InodeIntegrityError>;

    fn check_and_repair_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap,
        errors: &mut Vec<InodeIntegrityError>, lock: &BackupMode
    ) -> Result<Option<ChunkList>, RepositoryError>;

    fn evacuate_broken_backup(&self, name: &str, lock: &BackupMode) -> Result<(), RepositoryError>;

    fn check_and_repair_backup_inode(&mut self, name: &str, backup: &mut BackupFile, path: &Path,
        lock: &BackupMode,
    ) -> Result<ModuleIntegrityReport<InodeIntegrityError>, RepositoryError>;

    fn check_and_repair_backup(&mut self, name: &str, backup: &mut BackupFile, lock: &BackupMode
    ) -> Result<ModuleIntegrityReport<InodeIntegrityError>, RepositoryError>;

    fn check_and_repair_backups(&mut self, lock: &BackupMode
    ) -> Result<ModuleIntegrityReport<InodeIntegrityError>, RepositoryError>;
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
        errors: &mut Vec<InodeIntegrityError>, lock: &OnlineMode
    ) {
        let mut modified = false;
        match self.mark_chunks(checked, chunks, false) {
            Ok(false) => return,
            Ok(true) => (),
            Err(err) => {
                errors.push(InodeIntegrityError::BrokenInode(path, Box::new(err)));
                return
            },
        }
        let mut inode = match self.get_inode(chunks, lock) {
            Ok(inode) => inode,
            Err(err) => {
                errors.push(InodeIntegrityError::BrokenInode(path, Box::new(err)));
                return
            }
        };
        // Mark the content chunks as used
        if let Err(err) = self.check_inode_contents(&inode, checked, lock) {
            errors.push(InodeIntegrityError::MissingInodeData(path, Box::new(err)));
            return
        }
        if let Some(ref mut children) = inode.children {
            for (name, chunks) in children.iter_mut() {
                self.check_subtree(path.join(name), chunks, checked, errors, lock);
            }
        }
        self.mark_chunks(checked, chunks, true).unwrap();
    }

    fn check_backup_inode(&mut self, inode: &Inode, path: &Path, lock: &OnlineMode
    ) -> ModuleIntegrityReport<InodeIntegrityError> {
        tr_info!("Checking inode...");
        let mut report = ModuleIntegrityReport { errors_unfixed: vec![], errors_fixed: vec![] };
        let mut checked = self.get_chunk_marker();
        if let Err(err) = self.check_inode_contents(inode, &mut checked, lock) {
            report.errors_unfixed.push(InodeIntegrityError::MissingInodeData(path.to_path_buf(), Box::new(err)));
        }
        if let Some(ref children) = inode.children {
            for (name, chunks) in children.iter() {
                self.check_subtree(path.join(name), chunks, &mut checked, &mut report.errors_unfixed, lock);
            }
        }
        report
    }

    #[inline]
    fn check_backup(&mut self, name: &str, backup: &mut BackupFile, lock: &OnlineMode,
    ) -> ModuleIntegrityReport<InodeIntegrityError> {
        tr_info!("Checking backup...");
        let mut checked = self.get_chunk_marker();
        let mut report = ModuleIntegrityReport { errors_unfixed: vec![], errors_fixed: vec![] };
        self.check_subtree(Path::new("").to_path_buf(), &backup.root, &mut checked, &mut report.errors_unfixed, lock);
        report
    }

    fn check_backups(&mut self, lock: &OnlineMode) -> ModuleIntegrityReport<InodeIntegrityError> {
        tr_info!("Checking backups...");
        let mut checked = self.get_chunk_marker();
        let mut report = ModuleIntegrityReport { errors_unfixed: vec![], errors_fixed: vec![] };
        let backup_map = match self.get_all_backups() {
            Ok(backup_map) => backup_map,
            Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map,
                failed))) => {
                tr_warn!("Some backups could not be read, ignoring them");
                for path in &failed {
                    report.errors_unfixed.push(InodeIntegrityError::BackupRead(path.to_path_buf(),
                        Box::new(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map.clone(), failed.clone())))
                    ))
                }
                backup_map
            },
            _ => return report
        };
        for (name, mut backup) in ProgressIter::new(tr!("checking backups"), backup_map.len(), backup_map.into_iter()) {
            let path = format!("{}::", name);
            self.check_subtree(Path::new(&path).to_path_buf(), &backup.root,
                &mut checked, &mut report.errors_unfixed, lock);
        }
        report
    }


    fn check_and_repair_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap,
        errors: &mut Vec<InodeIntegrityError>, lock: &BackupMode,
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
            errors.push(InodeIntegrityError::MissingInodeData(path.clone(), Box::new(err)));
            inode.data = Some(FileData::Inline(vec![].into()));
            inode.size = 0;
            modified = true;
        }
        // Put children in to do
        if let Some(ref mut children) = inode.children {
            let mut removed = vec![];
            for (name, chunks) in children.iter_mut() {
                match self.check_and_repair_subtree(path.join(name), chunks, checked, errors, lock) {
                    Ok(None) => (),
                    Ok(Some(c)) => {
                        *chunks = c;
                        modified = true;
                    }
                    Err(err) => {
                        errors.push(InodeIntegrityError::BrokenInode(path.join(name), Box::new(err)));
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

    fn check_and_repair_backup_inode(&mut self, name: &str, backup: &mut BackupFile, path: &Path,
        lock: &BackupMode
    ) -> Result<ModuleIntegrityReport<InodeIntegrityError>, RepositoryError> {
        tr_info!("Checking inode...");
        let mut checked = self.get_chunk_marker();
        let mut inodes = try!(self.get_backup_path(backup, path, lock.as_online()));
        let mut inode = inodes.pop().unwrap();
        let mut modified = false;
        let mut errors = vec![];
        if let Err(err) = self.check_inode_contents(&inode, &mut checked, lock.as_online()) {
            errors.push(InodeIntegrityError::MissingInodeData(path.to_path_buf(), Box::new(err)));
            inode.data = Some(FileData::Inline(vec![].into()));
            inode.size = 0;
            modified = true;
        }
        if let Some(ref mut children) = inode.children {
            let mut removed = vec![];
            for (name, chunks) in children.iter_mut() {
                match self.check_and_repair_subtree(path.join(name), chunks, &mut checked, &mut errors, lock) {
                    Ok(None) => (),
                    Ok(Some(c)) => {
                        *chunks = c;
                        modified = true;
                    }
                    Err(err) => {
                        errors.push(InodeIntegrityError::BrokenInode(path.join(name), Box::new(err)));
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
        Ok(ModuleIntegrityReport{errors_unfixed: vec![], errors_fixed: errors})
    }

    #[inline]
    fn check_and_repair_backup(&mut self, name: &str, backup: &mut BackupFile, lock: &BackupMode,
    ) -> Result<ModuleIntegrityReport<InodeIntegrityError>, RepositoryError> {
        tr_info!("Checking backup...");
        let mut checked = self.get_chunk_marker();
        let mut errors = vec![];
        match self.check_and_repair_subtree(Path::new("").to_path_buf(),
            &backup.root, &mut checked, &mut errors, lock
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
                errors.push(InodeIntegrityError::BrokenInode(PathBuf::from("/"), Box::new(err)));
                try!(self.evacuate_broken_backup(name, lock));
            }
        }
        Ok(ModuleIntegrityReport{errors_unfixed: vec![], errors_fixed: errors})
    }

    fn check_and_repair_backups(&mut self, lock: &BackupMode
    ) -> Result<ModuleIntegrityReport<InodeIntegrityError>, RepositoryError> {
        tr_info!("Checking backups...");
        let mut checked = self.get_chunk_marker();
        let mut errors = vec![];
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
                &mut errors,
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
                    errors.push(InodeIntegrityError::BrokenInode(PathBuf::from(format!("{}::/", name)), Box::new(err)));
                    try!(self.evacuate_broken_backup(&name, lock));
                }
            }
        }
        Ok(ModuleIntegrityReport{errors_unfixed: vec![], errors_fixed: errors})
    }
}