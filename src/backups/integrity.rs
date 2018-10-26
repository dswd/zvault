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


pub struct CheckOptions {
    all_backups: bool,
    single_backup: Option<(String, BackupFile)>,
    subpath: Option<(PathBuf, Inode)>,
    index: bool,
    bundles: bool,
    bundle_data: bool,
    repair: bool
}

impl CheckOptions {
    pub fn new() -> CheckOptions {
        CheckOptions {
            all_backups: false,
            single_backup: None,
            subpath: None,
            index: false,
            bundles: false,
            bundle_data: false,
            repair: false
        }
    }

    pub fn all_backups(&mut self) -> &mut Self {
        self.all_backups = true;
        self.single_backup = None;
        self.subpath = None;
        self
    }

    pub fn single_backup(&mut self, name: &str, backup: BackupFile) -> &mut Self {
        self.all_backups = false;
        self.single_backup = Some((name.to_string(), backup));
        self
    }

    pub fn subpath(&mut self, subpath: &Path, inode: Inode) -> &mut Self {
        self.subpath = Some((subpath.to_path_buf(), inode));
        self
    }

    pub fn index(&mut self, index: bool) -> &mut Self {
        self.index = index;
        self
    }

    pub fn bundles(&mut self, bundles: bool) -> &mut Self {
        self.bundles = bundles;
        self.bundle_data &= bundles;
        self
    }

    pub fn bundle_data(&mut self, bundle_data: bool) -> &mut Self {
        self.bundle_data = bundle_data;
        self.bundles |= bundle_data;
        self
    }

    pub fn repair(&mut self, repair: bool) -> &mut Self {
        self.repair = repair;
        self
    }

    pub fn get_repair(&self) -> bool {
        self.repair
    }
}


pub struct IntegrityReport {
    pub bundle_map: Option<ModuleIntegrityReport<IntegrityError>>,
    pub index: Option<ModuleIntegrityReport<IntegrityError>>,
    pub bundles: Option<ModuleIntegrityReport<IntegrityError>>,
    pub backups: Option<ModuleIntegrityReport<InodeIntegrityError>>
}


pub trait RepositoryIntegrityIO {
    fn check_inode_contents(&mut self, inode: &Inode, checked: &mut Bitmap, lock: &OnlineMode
    ) -> Result<(), RepositoryError>;

    fn check_subtree(&mut self, path: PathBuf, chunks: &[Chunk], checked: &mut Bitmap,
        errors: &mut Vec<InodeIntegrityError>, lock: &OnlineMode
    );

    fn check_backup_inode(&mut self, inode: &Inode, path: &Path, lock: &OnlineMode
    ) -> ModuleIntegrityReport<InodeIntegrityError>;

    fn check_backup(&mut self, name: &str, backup: &BackupFile, lock: &OnlineMode
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

    fn check(&mut self, options: CheckOptions, lock: &OnlineMode) -> IntegrityReport;

    fn check_and_repair(&mut self, options: CheckOptions, lock: &VacuumMode
    ) -> Result<IntegrityReport, RepositoryError>;
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
    fn check_backup(&mut self, _name: &str, backup: &BackupFile, lock: &OnlineMode,
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


    fn evacuate_broken_backup(&self, name: &str, _lock: &BackupMode) -> Result<(), RepositoryError> {
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

    fn check(&mut self, options: CheckOptions, lock: &OnlineMode) -> IntegrityReport {
        let mut report = IntegrityReport {
            bundle_map: None,
            index: None,
            bundles: None,
            backups: None
        };
        report.bundle_map = Some(self.check_bundle_map());
        if options.index {
            report.index = Some(self.check_index(lock.as_readonly()));
        }
        if options.bundles {
            report.bundles = Some(self.check_bundles(options.bundle_data, lock));
        }
        if let Some((name, backup)) = options.single_backup {
            if let Some((subpath, inode)) = options.subpath {
                report.backups = Some(self.check_backup_inode(&inode, &subpath, lock))
            } else {
                report.backups = Some(self.check_backup(&name, &backup, lock));
            }
        }
        if options.all_backups {
            report.backups = Some(self.check_backups(lock));
        }
        report
    }

    fn check_and_repair(&mut self, options: CheckOptions, lock: &VacuumMode) -> Result<IntegrityReport, RepositoryError> {
        let mut report = IntegrityReport {
            bundle_map: None,
            index: None,
            bundles: None,
            backups: None
        };
        let bundle_map = try!(self.check_and_repair_bundle_map(lock.as_online()));
        if !bundle_map.errors_fixed.is_empty() {
            try!(self.rebuild_index(lock.as_online()));
        }
        report.bundle_map = Some(bundle_map);
        if options.index {
            report.index = Some(try!(self.check_and_repair_index(lock.as_online())));
        }
        if options.bundles {
            report.bundles = Some(try!(self.check_and_repair_bundles(options.bundle_data, lock)));
        }
        if let Some((name, mut backup)) = options.single_backup {
            if let Some((subpath, _inode)) = options.subpath {
                report.backups = Some(try!(self.check_and_repair_backup_inode(&name, &mut backup, &subpath, lock.as_backup())));
            } else {
                report.backups = Some(try!(self.check_and_repair_backup(&name, &mut backup, lock.as_backup())));
            }
        }
        if options.all_backups {
            report.backups = Some(try!(self.check_and_repair_backups(lock.as_backup())));
        }
        Ok(report)
    }


}