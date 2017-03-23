use ::prelude::*;

use std::io::{self, BufReader, BufWriter, Read, Write};
use std::fs::{self, File};
use std::path::{self, Path, PathBuf};
use std::collections::{HashMap, BTreeMap, VecDeque};

use chrono::prelude::*;


static HEADER_STRING: [u8; 7] = *b"zvault\x03";
static HEADER_VERSION: u8 = 1;


quick_error!{
    #[derive(Debug)]
    pub enum BackupFileError {
        Read(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to write backup")
            display("Backup file error: failed to write backup file {:?}\n\tcaused by: {}", path, err)
        }
        Write(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to read/write backup")
            display("Backup file error: failed to read backup file {:?}\n\tcaused by: {}", path, err)
        }
        Decode(err: msgpack::DecodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::DecodeError) -> (err, path.to_path_buf())
            description("Failed to decode backup")
            display("Backup file error: failed to decode backup of {:?}\n\tcaused by: {}", path, err)
        }
        Encode(err: msgpack::EncodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::EncodeError) -> (err, path.to_path_buf())
            description("Failed to encode backup")
            display("Backup file error: failed to encode backup of {:?}\n\tcaused by: {}", path, err)
        }
        WrongHeader(path: PathBuf) {
            description("Wrong header")
            display("Backup file error: wrong header on backup {:?}", path)
        }
        UnsupportedVersion(path: PathBuf, version: u8) {
            description("Wrong version")
            display("Backup file error: unsupported version on backup {:?}: {}", path, version)
        }
        Decryption(err: EncryptionError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: EncryptionError) -> (err, path.to_path_buf())
            description("Decryption failed")
            display("Backup file error: decryption failed on backup {:?}\n\tcaused by: {}", path, err)
        }
        Encryption(err: EncryptionError) {
            from()
            cause(err)
            description("Encryption failed")
            display("Backup file error: encryption failed\n\tcaused by: {}", err)
        }
        PartialBackupsList(partial: HashMap<String, Backup>, failed: Vec<PathBuf>) {
            description("Some backups could not be loaded")
            display("Backup file error: some backups could not be loaded: {:?}", failed)
        }
    }
}

#[derive(Default, Debug, Clone)]
struct BackupHeader {
    pub encryption: Option<Encryption>
}
serde_impl!(BackupHeader(u8) {
    encryption: Option<Encryption> => 0
});


#[derive(Default, Debug, Clone)]
pub struct Backup {
    pub root: ChunkList,
    pub total_data_size: u64, // Sum of all raw sizes of all entities
    pub changed_data_size: u64, // Sum of all raw sizes of all entities actively stored
    pub deduplicated_data_size: u64, // Sum of all raw sizes of all new bundles
    pub encoded_data_size: u64, // Sum al all encoded sizes of all new bundles
    pub bundle_count: usize,
    pub chunk_count: usize,
    pub avg_chunk_size: f32,
    pub date: i64,
    pub duration: f32,
    pub file_count: usize,
    pub dir_count: usize,
    pub host: String,
    pub path: String,
    pub config: Config,
}
serde_impl!(Backup(u8) {
    root: Vec<Chunk> => 0,
    total_data_size: u64 => 1,
    changed_data_size: u64 => 2,
    deduplicated_data_size: u64 => 3,
    encoded_data_size: u64 => 4,
    bundle_count: usize => 5,
    chunk_count: usize => 6,
    avg_chunk_size: f32 => 7,
    date: i64 => 8,
    duration: f32 => 9,
    file_count: usize => 10,
    dir_count: usize => 11,
    host: String => 12,
    path: String => 13,
    config: Config => 14
});

impl Backup {
    pub fn read_from<P: AsRef<Path>>(crypto: &Crypto, path: P) -> Result<Self, BackupFileError> {
        let path = path.as_ref();
        let mut file = BufReader::new(try!(File::open(path).map_err(|err| BackupFileError::Read(err, path.to_path_buf()))));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header).map_err(|err| BackupFileError::Read(err, path.to_path_buf())));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err(BackupFileError::WrongHeader(path.to_path_buf()))
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err(BackupFileError::UnsupportedVersion(path.to_path_buf(), version))
        }
        let header: BackupHeader = try!(msgpack::decode_from_stream(&mut file).context(path));
        let mut data = Vec::new();
        try!(file.read_to_end(&mut data).map_err(|err| BackupFileError::Read(err, path.to_path_buf())));
        if let Some(ref encryption) = header.encryption {
            data = try!(crypto.decrypt(encryption, &data));
        }
        Ok(try!(msgpack::decode(&data).context(path)))
    }

    pub fn save_to<P: AsRef<Path>>(&self, crypto: &Crypto, encryption: Option<Encryption>, path: P) -> Result<(), BackupFileError> {
        let path = path.as_ref();
        let mut data = try!(msgpack::encode(self).context(path));
        if let Some(ref encryption) = encryption {
            data = try!(crypto.encrypt(encryption, &data));
        }
        let mut file = BufWriter::new(try!(File::create(path).map_err(|err| BackupFileError::Write(err, path.to_path_buf()))));
        try!(file.write_all(&HEADER_STRING).map_err(|err| BackupFileError::Write(err, path.to_path_buf())));
        try!(file.write_all(&[HEADER_VERSION]).map_err(|err| BackupFileError::Write(err, path.to_path_buf())));
        let header = BackupHeader { encryption: encryption };
        try!(msgpack::encode_to_stream(&header, &mut file).context(path));
        try!(file.write_all(&data).map_err(|err| BackupFileError::Write(err, path.to_path_buf())));
        Ok(())
    }

    pub fn get_all_from<P: AsRef<Path>>(crypto: &Crypto, path: P) -> Result<HashMap<String, Backup>, BackupFileError> {
        let mut backups = HashMap::new();
        let base_path = path.as_ref();
        let mut paths = vec![path.as_ref().to_path_buf()];
        let mut failed_paths = vec![];
        while let Some(path) = paths.pop() {
            for entry in try!(fs::read_dir(&path).map_err(|e| BackupFileError::Read(e, path.clone()))) {
                let entry = try!(entry.map_err(|e| BackupFileError::Read(e, path.clone())));
                let path = entry.path();
                if path.is_dir() {
                    paths.push(path);
                } else {
                    let relpath = path.strip_prefix(&base_path).unwrap();
                    let name = relpath.to_string_lossy().to_string();
                    if let Ok(backup) = Backup::read_from(crypto, &path) {
                        backups.insert(name, backup);
                    } else {
                        failed_paths.push(path.clone());
                    }
                }
            }
        }
        if failed_paths.is_empty() {
            Ok(backups)
        } else {
            Err(BackupFileError::PartialBackupsList(backups, failed_paths))
        }
    }
}


quick_error!{
    #[derive(Debug)]
    #[allow(unknown_lints,large_enum_variant)]
    pub enum BackupError {
        FailedPaths(backup: Backup, failed: Vec<PathBuf>) {
            description("Some paths could not be backed up")
            display("Backup error: some paths could not be backed up")
        }
        RemoveRoot {
            description("The root of a backup can not be removed")
            display("Backup error: the root of a backup can not be removed")
        }
    }
}


impl Repository {
    pub fn get_backups(&self) -> Result<HashMap<String, Backup>, RepositoryError> {
        Ok(try!(Backup::get_all_from(&self.crypto.lock().unwrap(), self.path.join("backups"))))
    }

    pub fn get_backup(&self, name: &str) -> Result<Backup, RepositoryError> {
        Ok(try!(Backup::read_from(&self.crypto.lock().unwrap(), self.path.join("backups").join(name))))
    }

    pub fn save_backup(&mut self, backup: &Backup, name: &str) -> Result<(), RepositoryError> {
        let path = self.path.join("backups").join(name);
        try!(fs::create_dir_all(path.parent().unwrap()));
        Ok(try!(backup.save_to(&self.crypto.lock().unwrap(), self.config.encryption.clone(), path)))
    }

    pub fn delete_backup(&self, name: &str) -> Result<(), RepositoryError> {
        let mut path = self.path.join("backups").join(name);
        try!(fs::remove_file(&path));
        loop {
            path = path.parent().unwrap().to_owned();
            if fs::remove_dir(&path).is_err() {
                break
            }
        }
        Ok(())
    }


    pub fn prune_backups(&self, prefix: &str, daily: Option<usize>, weekly: Option<usize>, monthly: Option<usize>, yearly: Option<usize>, force: bool) -> Result<(), RepositoryError> {
        let mut backups = Vec::new();
        let backup_map = match self.get_backups() {
            Ok(backup_map) => backup_map,
            Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
                warn!("Some backups could not be read, ignoring them");
                backup_map
            },
            Err(err) => return Err(err)
        };
        for (name, backup) in backup_map {
            if name.starts_with(prefix) {
                let date = Local.timestamp(backup.date, 0);
                backups.push((name, date, backup));
            }
        }
        backups.sort_by_key(|backup| backup.2.date);
        let mut keep = Bitmap::new(backups.len());

        fn mark_needed<K: Eq, F: Fn(&DateTime<Local>) -> K>(backups: &[(String, DateTime<Local>, Backup)], keep: &mut Bitmap, max: usize, keyfn: F) {
            let mut unique = VecDeque::with_capacity(max+1);
            let mut last = None;
            for (i, backup) in backups.iter().enumerate() {
                let val = keyfn(&backup.1);
                let cur = Some(val);
                if cur != last {
                    last = cur;
                    unique.push_back(i);
                    if unique.len() > max {
                        unique.pop_front();
                    }
                }
            }
            for i in unique {
                keep.set(i);
            }
        }
        if let Some(max) = yearly {
            mark_needed(&backups, &mut keep, max, |d| d.year());
        }
        if let Some(max) = monthly {
            mark_needed(&backups, &mut keep, max, |d| (d.year(), d.month()));
        }
        if let Some(max) = weekly {
            mark_needed(&backups, &mut keep, max, |d| (d.isoweekdate().0, d.isoweekdate().1));
        }
        if let Some(max) = daily {
            mark_needed(&backups, &mut keep, max, |d| (d.year(), d.month(), d.day()));
        }
        let mut remove = Vec::new();
        for (i, backup) in backups.into_iter().enumerate() {
            if !keep.get(i) {
                remove.push(backup.0);
            }
        }
        info!("Removing the following backups: {:?}", remove);
        if force {
            for name in remove {
                try!(self.delete_backup(&name));
            }
        }
        Ok(())
    }

    pub fn restore_inode_tree<P: AsRef<Path>>(&mut self, inode: Inode, path: P) -> Result<(), RepositoryError> {
        let mut queue = VecDeque::new();
        queue.push_back((path.as_ref().to_owned(), inode));
        while let Some((path, inode)) = queue.pop_front() {
            try!(self.save_inode_at(&inode, &path));
            if inode.file_type == FileType::Directory {
                let path = path.join(inode.name);
                for chunks in inode.children.unwrap().values() {
                    let inode = try!(self.get_inode(&chunks));
                    queue.push_back((path.clone(), inode));
                }
            }
        }
        Ok(())
    }

    #[inline]
    pub fn restore_backup<P: AsRef<Path>>(&mut self, backup: &Backup, path: P) -> Result<(), RepositoryError> {
        let inode = try!(self.get_inode(&backup.root));
        self.restore_inode_tree(inode, path)
    }

    #[allow(dead_code)]
    pub fn create_backup<P: AsRef<Path>>(&mut self, path: P, reference: Option<&Backup>) -> Result<Backup, RepositoryError> {
        let reference_inode = reference.and_then(|b| self.get_inode(&b.root).ok());
        let mut scan_stack = vec![(path.as_ref().to_owned(), reference_inode)];
        let mut save_stack = vec![];
        let mut directories = HashMap::new();
        let mut backup = Backup::default();
        backup.config = self.config.clone();
        backup.host = get_hostname().unwrap_or_else(|_| "".to_string());
        backup.path = path.as_ref().to_string_lossy().to_string();
        let info_before = self.info();
        let start = Local::now();
        let mut failed_paths = vec![];
        while let Some((path, reference_inode)) = scan_stack.pop() {
            // Create an inode for this path containing all attributes and contents
            // (for files) but no children (for directories)
            let mut inode = match self.create_inode(&path, reference_inode.as_ref()) {
                Ok(inode) => inode,
                Err(RepositoryError::Inode(err)) => {
                    warn!("Failed to backup inode {}", err);
                    failed_paths.push(path);
                    continue
                },
                Err(err) => return Err(err)
            };
            let meta_size = 1000; // add 1000 for encoded metadata
            backup.total_data_size += inode.size + meta_size;
            if let Some(ref ref_inode) = reference_inode {
                if !ref_inode.is_unchanged(&inode) {
                    backup.changed_data_size += inode.size + meta_size;
                }
            } else {
                backup.changed_data_size += inode.size + meta_size;
            }
            if inode.file_type == FileType::Directory {
                backup.dir_count +=1;
                // For directories we need to put all children on the stack too, so there will be inodes created for them
                // Also we put directories on the save stack to save them in order
                save_stack.push(path.clone());
                inode.children = Some(BTreeMap::new());
                directories.insert(path.clone(), inode);
                let dirlist = match fs::read_dir(&path) {
                    Ok(dirlist) => dirlist,
                    Err(err) => {
                        warn!("Failed to read {:?}: {}", &path, err);
                        failed_paths.push(path);
                        continue
                    }
                };
                for ch in dirlist {
                    let child = match ch {
                        Ok(child) => child,
                        Err(err) => {
                            warn!("Failed to read {:?}: {}", &path, err);
                            continue
                        }
                    };
                    let name = child.file_name().to_string_lossy().to_string();
                    let ref_child = reference_inode.as_ref()
                        .and_then(|inode| inode.children.as_ref())
                        .and_then(|map| map.get(&name))
                        .and_then(|chunks| self.get_inode(chunks).ok());
                    scan_stack.push((child.path(), ref_child));
                }
            } else {
                backup.file_count +=1;
                // Non-directories are stored directly and the chunks are put into the children map of their parents
                if let Some(parent) = path.parent() {
                    let parent = parent.to_owned();
                    if !directories.contains_key(&parent) {
                        // This is a backup of one one file, put it in the directories map so it will be saved later
                        assert!(scan_stack.is_empty() && save_stack.is_empty() && directories.is_empty());
                        save_stack.push(path.clone());
                        directories.insert(path.clone(), inode);
                    } else {
                        let mut parent = directories.get_mut(&parent).unwrap();
                        let chunks = try!(self.put_inode(&inode));
                        let children = parent.children.as_mut().unwrap();
                        children.insert(inode.name.clone(), chunks);
                    }
                }
            }
        }
        loop {
            let path = save_stack.pop().unwrap();
            // Now that all children have been saved the directories can be saved in order, adding their chunks to their parents as well
            let inode = directories.remove(&path).unwrap();
            let chunks = try!(self.put_inode(&inode));
            if let Some(parent) = path.parent() {
                let parent = parent.to_owned();
                if let Some(ref mut parent) = directories.get_mut(&parent) {
                    let children = parent.children.as_mut().unwrap();
                    children.insert(inode.name.clone(), chunks);
                } else if save_stack.is_empty() {
                    backup.root = chunks;
                    break
                }
            } else if save_stack.is_empty() {
                backup.root = chunks;
                break
            }
        }
        try!(self.flush());
        let elapsed = Local::now().signed_duration_since(start);
        backup.date = start.timestamp();
        backup.duration = elapsed.num_milliseconds() as f32 / 1_000.0;
        let info_after = self.info();
        backup.deduplicated_data_size = info_after.raw_data_size - info_before.raw_data_size;
        backup.encoded_data_size = info_after.encoded_data_size - info_before.encoded_data_size;
        backup.bundle_count = info_after.bundle_count - info_before.bundle_count;
        backup.chunk_count = info_after.chunk_count - info_before.chunk_count;
        backup.avg_chunk_size = backup.deduplicated_data_size as f32 / backup.chunk_count as f32;
        if failed_paths.is_empty() {
            Ok(backup)
        } else {
            Err(BackupError::FailedPaths(backup, failed_paths).into())
        }
    }

    pub fn remove_backup_path<P: AsRef<Path>>(&mut self, backup: &mut Backup, path: P) -> Result<(), RepositoryError> {
        let mut inodes = try!(self.get_backup_path(backup, path));
        let to_remove = inodes.pop().unwrap();
        let mut remove_from = match inodes.pop() {
            Some(inode) => inode,
            None => return Err(BackupError::RemoveRoot.into())
        };
        remove_from.children.as_mut().unwrap().remove(&to_remove.name);
        let mut last_inode_chunks = try!(self.put_inode(&remove_from));
        let mut last_inode_name = remove_from.name;
        while let Some(mut inode) = inodes.pop() {
            inode.children.as_mut().unwrap().insert(last_inode_name, last_inode_chunks);
            last_inode_chunks = try!(self.put_inode(&inode));
            last_inode_name = inode.name;
        }
        backup.root = last_inode_chunks;
        Ok(())
    }

    pub fn get_backup_path<P: AsRef<Path>>(&mut self, backup: &Backup, path: P) -> Result<Vec<Inode>, RepositoryError> {
        let mut inodes = vec![];
        let mut inode = try!(self.get_inode(&backup.root));
        for c in path.as_ref().components() {
            if let path::Component::Normal(name) = c {
                let name = name.to_string_lossy();
                if let Some(chunks) = inode.children.as_mut().and_then(|c| c.remove(&name as &str)) {
                    inodes.push(inode);
                    inode = try!(self.get_inode(&chunks));
                } else {
                    return Err(RepositoryError::NoSuchFileInBackup(backup.clone(), path.as_ref().to_owned()));
                }
            }
        }
        inodes.push(inode);
        Ok(inodes)
    }

    #[inline]
    pub fn get_backup_inode<P: AsRef<Path>>(&mut self, backup: &Backup, path: P) -> Result<Inode, RepositoryError> {
        self.get_backup_path(backup, path).map(|mut inodes| inodes.pop().unwrap())
    }
}
