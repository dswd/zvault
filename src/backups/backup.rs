use prelude::*;

use std::fs;
use std::path::{self, Path, PathBuf};
use std::collections::{HashMap, BTreeMap, VecDeque};
use std::os::linux::fs::MetadataExt;

use chrono::prelude::*;
use regex::RegexSet;
use users::{self, Users, Groups};


quick_error!{
    #[derive(Debug)]
    #[allow(unknown_lints,large_enum_variant)]
    pub enum BackupError {
        FailedPaths(backup: BackupFile, failed: Vec<PathBuf>) {
            description(tr!("Some paths could not be backed up"))
            display("{}", tr_format!("Backup error: some paths could not be backed up"))
        }
        RemoveRoot {
            description(tr!("The root of a backup can not be removed"))
            display("{}", tr_format!("Backup error: the root of a backup can not be removed"))
        }
    }
}


pub struct BackupOptions {
    pub same_device: bool,
    pub excludes: Option<RegexSet>
}


pub enum DiffType {
    Add,
    Mod,
    Del
}


impl BackupRepository {
    pub fn get_all_backups(&self) -> Result<HashMap<String, BackupFile>, RepositoryError> {
        Ok(try!(BackupFile::get_all_from(
            &self.crypto,
            self.layout.backups_path()
        )))
    }

    pub fn get_backups<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<HashMap<String, BackupFile>, RepositoryError> {
        Ok(try!(BackupFile::get_all_from(
            &self.crypto,
            self.layout.backups_path().join(path)
        )))
    }

    #[inline]
    pub fn has_backup(&self, name: &str) -> bool {
        self.layout.backup_path(name).exists()
    }

    pub fn get_backup(&self, name: &str) -> Result<BackupFile, RepositoryError> {
        Ok(try!(BackupFile::read_from(
            &self.crypto,
            self.layout.backup_path(name)
        )))
    }

    pub fn save_backup(&mut self, backup: &BackupFile, name: &str) -> Result<(), RepositoryError> {
        try!(self.repo.write_mode());
        let path = self.layout.backup_path(name);
        try!(fs::create_dir_all(path.parent().unwrap()));
        try!(backup.save_to(
            &self.crypto,
            self.get_config().encryption.clone(),
            path
        ));
        Ok(())
    }

    pub fn delete_backup(&mut self, name: &str) -> Result<(), RepositoryError> {
        try!(self.repo.write_mode());
        let mut path = self.layout.backup_path(name);
        try!(fs::remove_file(&path));
        loop {
            path = path.parent().unwrap().to_owned();
            if path == self.layout.backups_path() || fs::remove_dir(&path).is_err() {
                break;
            }
        }
        Ok(())
    }


    pub fn prune_backups(
        &mut self,
        prefix: &str,
        daily: usize,
        weekly: usize,
        monthly: usize,
        yearly: usize,
        force: bool,
    ) -> Result<(), RepositoryError> {
        try!(self.repo.write_mode());
        let mut backups = Vec::new();
        let backup_map = match self.get_all_backups() {
            Ok(backup_map) => backup_map,
            Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map,
                                                                                _failed))) => {
                tr_warn!("Some backups could not be read, ignoring them");
                backup_map
            }
            Err(err) => return Err(err),
        };
        for (name, backup) in backup_map {
            if name.starts_with(prefix) {
                let date = Local.timestamp(backup.timestamp, 0);
                backups.push((name, date, backup));
            }
        }
        backups.sort_by_key(|backup| -backup.2.timestamp);
        let mut keep = Bitmap::new(backups.len());

        fn mark_needed<K: Eq, F: Fn(&DateTime<Local>) -> K>(
            backups: &[(String, DateTime<Local>, BackupFile)],
            keep: &mut Bitmap,
            max: usize,
            keyfn: F,
        ) {
            let mut kept = 0;
            let mut last = None;
            for (i, backup) in backups.iter().enumerate() {
                let val = keyfn(&backup.1);
                let cur = Some(val);
                if cur != last {
                    if kept >= max {
                        break;
                    }
                    last = cur;
                    keep.set(i);
                    kept += 1;
                }
            }
        }
        if yearly > 0 {
            mark_needed(&backups, &mut keep, yearly, |d| d.year());
        }
        if monthly > 0 {
            mark_needed(&backups, &mut keep, monthly, |d| (d.year(), d.month()));
        }
        if weekly > 0 {
            mark_needed(&backups, &mut keep, weekly, |d| {
                let week = d.iso_week();
                (week.year(), week.week())
            });
        }
        if daily > 0 {
            mark_needed(
                &backups,
                &mut keep,
                daily,
                |d| (d.year(), d.month(), d.day())
            );
        }
        let mut remove = Vec::new();
        println!("Removing the following backups");
        for (i, backup) in backups.into_iter().enumerate() {
            if !keep.get(i) {
                println!("  - {}", backup.0);
                remove.push(backup.0);
            }
        }
        if force {
            for name in remove {
                try!(self.delete_backup(&name));
            }
        }
        Ok(())
    }

    pub fn restore_inode_tree<P: AsRef<Path>>(
        &mut self,
        backup: &BackupFile,
        inode: Inode,
        path: P,
    ) -> Result<(), RepositoryError> {
        let _lock = try!(self.repo.lock(false));
        let mut queue = VecDeque::new();
        queue.push_back((path.as_ref().to_owned(), inode));
        let cache = users::UsersCache::new();
        let mut is_root = true;
        while let Some((path, mut inode)) = queue.pop_front() {
            if inode.file_type != FileType::Directory || !is_root {
                if let Some(name) = backup.user_names.get(&inode.user) {
                    if let Some(user) = cache.get_user_by_name(name) {
                        inode.user = user.uid();
                    }
                }
                if let Some(name) = backup.group_names.get(&inode.group) {
                    if let Some(group) = cache.get_group_by_name(name) {
                        inode.group = group.gid();
                    }
                }
                try!(self.save_inode_at(&inode, &path));
            }
            if inode.file_type == FileType::Directory {
                let path = if is_root {
                    path.to_path_buf()
                } else {
                    path.join(inode.name)
                };
                for chunks in inode.children.unwrap().values() {
                    let inode = try!(self.get_inode(chunks));
                    queue.push_back((path.clone(), inode));
                }
            }
            is_root = false;
        }
        Ok(())
    }

    pub fn create_backup_recurse<P: AsRef<Path>>(
        &mut self,
        path: P,
        reference: Option<&Inode>,
        options: &BackupOptions,
        backup: &mut BackupFile,
        failed_paths: &mut Vec<PathBuf>,
    ) -> Result<Inode, RepositoryError> {
        let path = path.as_ref();
        let mut inode = try!(self.create_inode(path, reference));
        if !backup.user_names.contains_key(&inode.user) {
            if let Some(user) = users::get_user_by_uid(inode.user) {
                backup.user_names.insert(
                    inode.user,
                    user.name().to_string()
                );
            } else {
                tr_warn!("Failed to retrieve name of user {}", inode.user);
            }
        }
        if !backup.group_names.contains_key(&inode.group) {
            if let Some(group) = users::get_group_by_gid(inode.group) {
                backup.group_names.insert(
                    inode.group,
                    group.name().to_string()
                );
            } else {
                tr_warn!("Failed to retrieve name of group {}", inode.group);
            }
        }
        let mut meta_size = 0;
        inode.cum_size = inode.size;
        if inode.file_type == FileType::Directory {
            inode.cum_dirs = 1;
            let mut children = BTreeMap::new();
            let parent_dev = try!(path.metadata()).st_dev();
            for ch in try!(fs::read_dir(path)) {
                let child = try!(ch);
                let child_path = child.path();
                if options.same_device {
                    let child_dev = try!(child.metadata()).st_dev();
                    if child_dev != parent_dev {
                        continue;
                    }
                }
                if let Some(ref excludes) = options.excludes {
                    let child_path_str = child_path.to_string_lossy();
                    if excludes.is_match(&child_path_str) {
                        continue;
                    }
                }
                let name = child.file_name().to_string_lossy().to_string();
                let ref_child = reference
                    .as_ref()
                    .and_then(|inode| inode.children.as_ref())
                    .and_then(|map| map.get(&name))
                    .and_then(|chunks| self.get_inode(chunks).ok());
                let child_inode = match self.create_backup_recurse(
                    &child_path,
                    ref_child.as_ref(),
                    options,
                    backup,
                    failed_paths
                ) {
                    Ok(inode) => inode,
                    Err(RepositoryError::Inode(_)) |
                    Err(RepositoryError::Chunker(_)) |
                    Err(RepositoryError::Io(_)) => {
                        info!("Failed to backup {:?}", child_path);
                        failed_paths.push(child_path);
                        continue;
                    }
                    Err(err) => return Err(err),
                };
                let chunks = try!(self.put_inode(&child_inode));
                inode.cum_size += child_inode.cum_size;
                for &(_, len) in chunks.iter() {
                    meta_size += u64::from(len);
                }
                inode.cum_dirs += child_inode.cum_dirs;
                inode.cum_files += child_inode.cum_files;
                children.insert(name, chunks);
            }
            inode.children = Some(children);
        } else {
            inode.cum_files = 1;
            if let Some(FileData::ChunkedIndirect(ref chunks)) = inode.data {
                for &(_, len) in chunks.iter() {
                    meta_size += u64::from(len);
                }
            }
        }
        inode.cum_size += meta_size;
        if let Some(ref_inode) = reference {
            if !ref_inode.is_same_meta_quick(&inode) {
                backup.changed_data_size += inode.size + meta_size;
            }
        } else {
            backup.changed_data_size += inode.size + meta_size;
        }
        Ok(inode)
    }

    pub fn create_backup_recursively<P: AsRef<Path>>(
        &mut self,
        path: P,
        reference: Option<&BackupFile>,
        options: &BackupOptions,
    ) -> Result<BackupFile, RepositoryError> {
        try!(self.repo.write_mode());
        let _lock = try!(self.repo.lock(false));
        if self.repo.is_dirty() {
            return Err(RepositoryError::Dirty);
        }
        try!(self.repo.set_dirty());
        let reference_inode = reference.and_then(|b| self.get_inode(&b.root).ok());
        let mut backup = BackupFile::default();
        backup.config = self.get_config().clone();
        backup.host = get_hostname().unwrap_or_else(|_| "".to_string());
        backup.path = path.as_ref().to_string_lossy().to_string();
        let info_before = self.info();
        let start = Local::now();
        let mut failed_paths = vec![];
        let root_inode = try!(self.create_backup_recurse(
            path,
            reference_inode.as_ref(),
            options,
            &mut backup,
            &mut failed_paths
        ));
        backup.root = try!(self.put_inode(&root_inode));
        try!(self.repo.flush());
        let elapsed = Local::now().signed_duration_since(start);
        backup.timestamp = start.timestamp();
        backup.total_data_size = root_inode.cum_size;
        for &(_, len) in backup.root.iter() {
            backup.total_data_size += u64::from(len);
        }
        backup.file_count = root_inode.cum_files;
        backup.dir_count = root_inode.cum_dirs;
        backup.duration = elapsed.num_milliseconds() as f32 / 1_000.0;
        let info_after = self.info();
        backup.deduplicated_data_size = info_after.raw_data_size - info_before.raw_data_size;
        backup.encoded_data_size = info_after.encoded_data_size - info_before.encoded_data_size;
        backup.bundle_count = info_after.bundle_count - info_before.bundle_count;
        backup.chunk_count = info_after.chunk_count - info_before.chunk_count;
        backup.avg_chunk_size = backup.deduplicated_data_size as f32 / backup.chunk_count as f32;
        self.repo.set_clean();
        if failed_paths.is_empty() {
            Ok(backup)
        } else {
            Err(BackupError::FailedPaths(backup, failed_paths).into())
        }
    }

    pub fn remove_backup_path<P: AsRef<Path>>(
        &mut self,
        backup: &mut BackupFile,
        path: P,
    ) -> Result<(), RepositoryError> {
        try!(self.repo.write_mode());
        let _lock = try!(self.repo.lock(false));
        let mut inodes = try!(self.get_backup_path(backup, path));
        let to_remove = inodes.pop().unwrap();
        let mut remove_from = match inodes.pop() {
            Some(inode) => inode,
            None => return Err(BackupError::RemoveRoot.into()),
        };
        remove_from.children.as_mut().unwrap().remove(
            &to_remove.name
        );
        let mut last_inode_chunks = try!(self.put_inode(&remove_from));
        let mut last_inode_name = remove_from.name;
        while let Some(mut inode) = inodes.pop() {
            inode.children.as_mut().unwrap().insert(
                last_inode_name,
                last_inode_chunks
            );
            last_inode_chunks = try!(self.put_inode(&inode));
            last_inode_name = inode.name;
        }
        backup.root = last_inode_chunks;
        backup.modified = true;
        Ok(())
    }

    pub fn get_backup_path<P: AsRef<Path>>(
        &mut self,
        backup: &BackupFile,
        path: P,
    ) -> Result<Vec<Inode>, RepositoryError> {
        let mut inodes = vec![];
        let mut inode = try!(self.get_inode(&backup.root));
        for c in path.as_ref().components() {
            if let path::Component::Normal(name) = c {
                let name = name.to_string_lossy();
                if inodes.is_empty() && inode.file_type != FileType::Directory &&
                    inode.name == name
                {
                    return Ok(vec![inode]);
                }
                if let Some(chunks) = inode.children.as_mut().and_then(
                    |c| c.remove(&name as &str)
                )
                {
                    inodes.push(inode);
                    inode = try!(self.get_inode(&chunks));
                } else {
                    return Err(RepositoryError::NoSuchFileInBackup(
                        backup.clone(),
                        path.as_ref().to_owned()
                    ));
                }
            }
        }
        inodes.push(inode);
        Ok(inodes)
    }

    #[inline]
    pub fn get_backup_inode<P: AsRef<Path>>(
        &mut self,
        backup: &BackupFile,
        path: P,
    ) -> Result<Inode, RepositoryError> {
        self.get_backup_path(backup, path).map(|mut inodes| {
            inodes.pop().unwrap()
        })
    }

    pub fn find_versions<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<Vec<(String, Inode)>, RepositoryError> {
        let path = path.as_ref();
        let mut versions = HashMap::new();
        for (name, backup) in try!(self.get_all_backups()) {
            match self.get_backup_inode(&backup, path) {
                Ok(inode) => {
                    versions.insert(
                        (inode.file_type, inode.timestamp, inode.size),
                        (name, inode)
                    );
                }
                Err(RepositoryError::NoSuchFileInBackup(..)) => continue,
                Err(err) => return Err(err),
            }
        }
        let mut versions: Vec<_> = versions.into_iter().map(|(_, v)| v).collect();
        versions.sort_by_key(|v| v.1.timestamp);
        Ok(versions)
    }

    #[allow(needless_pass_by_value)]
    fn find_differences_recurse(
        &mut self,
        inode1: &Inode,
        inode2: &Inode,
        path: PathBuf,
        diffs: &mut Vec<(DiffType, PathBuf)>,
    ) -> Result<(), RepositoryError> {
        if !inode1.is_same_meta(inode2) || inode1.data != inode2.data {
            diffs.push((DiffType::Mod, path.clone()));
        }
        if let Some(ref children1) = inode1.children {
            if let Some(ref children2) = inode2.children {
                for name in children1.keys() {
                    if !children2.contains_key(name) {
                        diffs.push((DiffType::Del, path.join(name)));
                    }
                }
            } else {
                for name in children1.keys() {
                    diffs.push((DiffType::Del, path.join(name)));
                }
            }
        }
        if let Some(ref children2) = inode2.children {
            if let Some(ref children1) = inode1.children {
                for (name, chunks2) in children2 {
                    if let Some(chunks1) = children1.get(name) {
                        if chunks1 != chunks2 {
                            let inode1 = try!(self.get_inode(chunks1));
                            let inode2 = try!(self.get_inode(chunks2));
                            try!(self.find_differences_recurse(
                                &inode1,
                                &inode2,
                                path.join(name),
                                diffs
                            ));
                        }
                    } else {
                        diffs.push((DiffType::Add, path.join(name)));
                    }
                }
            } else {
                for name in children2.keys() {
                    diffs.push((DiffType::Add, path.join(name)));
                }
            }
        }
        Ok(())
    }

    #[inline]
    pub fn find_differences(
        &mut self,
        inode1: &Inode,
        inode2: &Inode,
    ) -> Result<Vec<(DiffType, PathBuf)>, RepositoryError> {
        let mut diffs = vec![];
        let path = PathBuf::from("/");
        try!(self.find_differences_recurse(
            inode1,
            inode2,
            path,
            &mut diffs
        ));
        Ok(diffs)
    }

    fn count_sizes_recursive(&mut self, inode: &Inode, sizes: &mut HashMap<u64, usize>, min_size: u64) -> Result<(), RepositoryError> {
        if inode.size >= min_size {
            *sizes.entry(inode.size).or_insert(0) += 1;
        }
        if let Some(ref children) = inode.children {
            for chunks in children.values() {
                let ch = try!(self.get_inode(chunks));
                try!(self.count_sizes_recursive(&ch, sizes, min_size));
            }
        }
        Ok(())
    }

    fn find_duplicates_recursive(&mut self, inode: &Inode, path: &Path, sizes: &HashMap<u64, usize>, hashes: &mut HashMap<Hash, (Vec<PathBuf>, u64)>) -> Result<(), RepositoryError> {
        let path = path.join(&inode.name);
        if sizes.get(&inode.size).cloned().unwrap_or(0) > 1 {
            if let Some(ref data) = inode.data {
                let chunk_data = try!(msgpack::encode(data).map_err(InodeError::from));
                let hash = HashMethod::Blake2.hash(&chunk_data);
                hashes.entry(hash).or_insert((Vec::new(), inode.size)).0.push(path.clone());
            }
        }
        if let Some(ref children) = inode.children {
            for chunks in children.values() {
                let ch = try!(self.get_inode(chunks));
                try!(self.find_duplicates_recursive(&ch, &path, sizes, hashes));
            }
        }
        Ok(())
    }

    pub fn find_duplicates(&mut self, inode: &Inode, min_size: u64) -> Result<Vec<(Vec<PathBuf>, u64)>, RepositoryError> {
        let mut sizes = HashMap::new();
        try!(self.count_sizes_recursive(inode, &mut sizes, min_size));
        let mut hashes = HashMap::new();
        if let Some(ref children) = inode.children {
            for chunks in children.values() {
                let ch = try!(self.get_inode(chunks));
                try!(self.find_duplicates_recursive(&ch, Path::new(""), &sizes, &mut hashes));
            }
        }
        let dups = hashes.into_iter().map(|(_,v)| v).filter(|&(ref v, _)| v.len() > 1).collect();
        Ok(dups)
    }
}
