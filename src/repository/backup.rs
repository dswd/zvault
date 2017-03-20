use super::{Repository, RepositoryError};
use super::metadata::{FileType, Inode};

use ::util::*;

use std::fs::{self, File};
use std::path::{self, Path};
use std::collections::{HashMap, VecDeque};

use chrono::prelude::*;


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
    pub dir_count: usize
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
    dir_count: usize => 11
});


impl Repository {
    pub fn list_backups(&self) -> Result<HashMap<String, Backup>, RepositoryError> {
        let mut backups = HashMap::new();
        let mut paths = Vec::new();
        let base_path = self.path.join("backups");
        paths.push(base_path.clone());
        while let Some(path) = paths.pop() {
            for entry in try!(fs::read_dir(path)) {
                let entry = try!(entry);
                let path = entry.path();
                if path.is_dir() {
                    paths.push(path);
                } else {
                    let relpath = path.strip_prefix(&base_path).unwrap();
                    let name = relpath.to_string_lossy().to_string();
                    let backup = try!(self.get_backup(&name));
                    backups.insert(name, backup);
                }
            }
        }
        Ok(backups)
    }

    pub fn get_backup(&self, name: &str) -> Result<Backup, RepositoryError> {
        let mut file = try!(File::open(self.path.join("backups").join(name)));
        Ok(try!(msgpack::decode_from_stream(&mut file)))
    }

    pub fn save_backup(&mut self, backup: &Backup, name: &str) -> Result<(), RepositoryError> {
        let path = self.path.join("backups").join(name);
        try!(fs::create_dir_all(path.parent().unwrap()));
        let mut file = try!(File::create(path));
        Ok(try!(msgpack::encode_to_stream(backup, &mut file)))
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
        for (name, backup) in try!(self.list_backups()) {
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
    pub fn create_full_backup<P: AsRef<Path>>(&mut self, path: P) -> Result<Backup, RepositoryError> {
        let mut scan_stack = vec![path.as_ref().to_owned()];
        let mut save_stack = vec![];
        let mut directories = HashMap::new();
        let mut backup = Backup::default();
        let info_before = self.info();
        let start = Local::now();
        while let Some(path) = scan_stack.pop() {
            // Create an inode for this path containing all attributes and contents
            // (for files) but no children (for directories)
            let mut inode = try!(self.create_inode(&path));
            backup.total_data_size += inode.size;
            backup.changed_data_size += inode.size;
            if inode.file_type == FileType::Directory {
                backup.dir_count +=1;
                // For directories we need to put all children on the stack too, so there will be inodes created for them
                // Also we put directories on the save stack to save them in order
                save_stack.push(path.clone());
                inode.children = Some(HashMap::new());
                directories.insert(path.clone(), inode);
                for ch in try!(fs::read_dir(&path)) {
                    scan_stack.push(try!(ch).path());
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
        Ok(backup)
    }

    pub fn get_backup_inode<P: AsRef<Path>>(&mut self, backup: &Backup, path: P) -> Result<Inode, RepositoryError> {
        let mut inode = try!(self.get_inode(&backup.root));
        for c in path.as_ref().components() {
            if let path::Component::Normal(name) = c {
                let name = name.to_string_lossy();
                if let Some(chunks) = inode.children.as_mut().and_then(|c| c.remove(&name as &str)) {
                    inode = try!(self.get_inode(&chunks));
                } else {
                    return Err(RepositoryError::NoSuchFileInBackup(backup.clone(), path.as_ref().to_owned()));
                }
            }
        }
        Ok(inode)
    }
}
