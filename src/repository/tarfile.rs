use ::prelude::*;

use std::collections::{HashMap, HashSet, BTreeMap};
use std::path::{Path, PathBuf};
use std::io::{self, Read, Write, Cursor};
use std::fs::File;
use std::str;

use chrono::prelude::*;

use tar;


static PAX_XATTR_PREFIX: &'static str = "SCHILY.xattr.";

fn inode_from_entry<R: Read>(entry: &mut tar::Entry<R>) -> Result<Inode, RepositoryError> {
    let mut inode = {
        let path = try!(entry.path());
        let header = entry.header();
        let file_type = match header.entry_type() {
            tar::EntryType::Regular | tar::EntryType::Link | tar::EntryType::Continuous => FileType::File,
            tar::EntryType::Symlink => FileType::Symlink,
            tar::EntryType::Directory => FileType::Directory,
            _ => return Err(InodeError::UnsupportedFiletype(path.to_path_buf()).into())
        };
        Inode {
            file_type: file_type,
            name: path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "/".to_string()),
            symlink_target: try!(entry.link_name()).map(|s| s.to_string_lossy().to_string()),
            size: try!(header.size()),
            mode: try!(header.mode()),
            user: try!(header.uid()),
            group: try!(header.gid()),
            timestamp: try!(header.mtime()) as i64,
            ..Default::default()
        }
    };
    if let Some(exts) = try!(entry.pax_extensions()) {
        for ext in exts {
            let ext = try!(ext);
            let key = ext.key().unwrap_or("");
            if key.starts_with(PAX_XATTR_PREFIX) {
                inode.xattrs.insert(key[PAX_XATTR_PREFIX.len()..].to_string(), ext.value_bytes().to_vec().into());
            }
        }
    }
    if inode.file_type == FileType::Directory {
        inode.children = Some(BTreeMap::new());
    }
    Ok(inode)
}

impl Repository {
    fn import_tar_entry<R: Read>(&mut self, entry: &mut tar::Entry<R>) -> Result<Inode, RepositoryError> {
        let mut inode = try!(inode_from_entry(entry));
        if inode.size < 100 {
            let mut data = Vec::with_capacity(inode.size as usize);
            try!(entry.read_to_end(&mut data));
            inode.data = Some(FileData::Inline(data.into()));
        } else {
            let mut chunks = try!(self.put_stream(BundleMode::Data, entry));
            if chunks.len() < 10 {
                inode.data = Some(FileData::ChunkedDirect(chunks));
            } else {
                let mut chunk_data = Vec::with_capacity(chunks.encoded_size());
                chunks.write_to(&mut chunk_data).unwrap();
                chunks = try!(self.put_data(BundleMode::Meta, &chunk_data));
                inode.data = Some(FileData::ChunkedIndirect(chunks));
            }
        }
        Ok(inode)
    }

    fn import_tarfile_as_inode<R: Read>(&mut self, backup: &mut Backup, input: R, failed_paths: &mut Vec<PathBuf>) -> Result<(Inode, ChunkList), RepositoryError> {
        let mut tarfile = tar::Archive::new(input);
        // Step 1: create inodes for all entries
        let mut inodes = HashMap::<PathBuf, (Inode, HashSet<String>)>::new();
        for entry in try!(tarfile.entries()) {
            let mut entry = try!(entry);
            let path = try!(entry.path()).to_path_buf();
            match self.import_tar_entry(&mut entry) {
                Ok(mut inode) => {
                    inode.cum_size = inode.size;
                    if inode.file_type == FileType::Directory {
                        inode.cum_dirs = 1;
                    } else {
                        if let Some(FileData::ChunkedIndirect(ref chunks)) = inode.data {
                            for &(_, len) in chunks.iter() {
                                inode.cum_size += len as u64;
                            }
                        }
                        inode.cum_files = 1;
                    }
                    if let Some(parent_path) = path.parent() {
                        if let Some(&mut (_, ref mut children)) = inodes.get_mut(parent_path) {
                            children.insert(inode.name.clone());
                        }
                    }
                    if let Ok(Some(name)) = entry.header().username() {
                        backup.user_names.insert(inode.user, name.to_string());
                    }
                    if let Ok(Some(name)) = entry.header().groupname() {
                        backup.group_names.insert(inode.group, name.to_string());
                    }
                    inodes.insert(path, (inode, HashSet::new()));
                },
                Err(RepositoryError::Inode(_)) | Err(RepositoryError::Chunker(_)) | Err(RepositoryError::Io(_)) => {
                    info!("Failed to backup {:?}", path);
                    failed_paths.push(path);
                    continue
                },
                Err(err) => {
                    return Err(err);
                }
            }
        }
        // Step 2: save all inodes
        let mut roots = vec![];
        while !inodes.is_empty() {
            let mut childless = vec![];
            for (path, &(_, ref children)) in &inodes {
                if children.is_empty() {
                    childless.push(path.clone());
                }
            }
            for path in childless {
                let (inode, _) = inodes.remove(&path).unwrap();
                let chunks = try!(self.put_inode(&inode));
                if let Some(parent_path) = path.parent() {
                    if let Some(&mut (ref mut parent_inode, ref mut children)) = inodes.get_mut(parent_path) {
                        children.remove(&inode.name);
                        parent_inode.cum_size += inode.cum_size;
                        for &(_, len) in chunks.iter() {
                            parent_inode.cum_size += len as u64;
                        }
                        parent_inode.cum_files += inode.cum_files;
                        parent_inode.cum_dirs += inode.cum_dirs;
                        parent_inode.children.as_mut().unwrap().insert(inode.name.clone(), chunks);
                        continue
                    }
                }
                roots.push((inode, chunks));
            }
        }
        if roots.len() == 1 {
            Ok(roots.pop().unwrap())
        } else {
            warn!("Tar file contains multiple roots, adding dummy folder");
            let mut root_inode = Inode {
                file_type: FileType::Directory,
                mode: 0o755,
                name: "archive".to_string(),
                cum_size: 0,
                cum_files: 0,
                cum_dirs: 1,
                ..Default::default()
            };
            let mut children = BTreeMap::new();
            for (inode, chunks) in roots {
                root_inode.cum_size += inode.cum_size;
                for &(_, len) in chunks.iter() {
                    root_inode.cum_size += len as u64;
                }
                root_inode.cum_files += inode.cum_files;
                root_inode.cum_dirs += inode.cum_dirs;
                children.insert(inode.name, chunks);
            }
            root_inode.children = Some(children);
            let chunks = try!(self.put_inode(&root_inode));
            Ok((root_inode, chunks))
        }
    }

    pub fn import_tarfile<P: AsRef<Path>>(&mut self, tarfile: P) -> Result<Backup, RepositoryError> {
        try!(self.write_mode());
        let _lock = try!(self.lock(false));
        if self.dirty {
            return Err(RepositoryError::Dirty)
        }
        try!(self.set_dirty());
        let mut backup = Backup::default();
        backup.config = self.config.clone();
        backup.host = get_hostname().unwrap_or_else(|_| "".to_string());
        backup.path = tarfile.as_ref().to_string_lossy().to_string();
        let info_before = self.info();
        let start = Local::now();
        let mut failed_paths = vec![];
        let tarfile = tarfile.as_ref();
        let (root_inode, chunks) = if tarfile == Path::new("-") {
            try!(self.import_tarfile_as_inode(&mut backup, try!(File::open(tarfile)), &mut failed_paths))
        } else {
            try!(self.import_tarfile_as_inode(&mut backup, io::stdin(), &mut failed_paths))
        };
        backup.root = chunks;
        try!(self.flush());
        let elapsed = Local::now().signed_duration_since(start);
        backup.timestamp = start.timestamp();
        backup.total_data_size = root_inode.cum_size;
        backup.file_count = root_inode.cum_files;
        backup.dir_count = root_inode.cum_dirs;
        backup.duration = elapsed.num_milliseconds() as f32 / 1_000.0;
        let info_after = self.info();
        backup.deduplicated_data_size = info_after.raw_data_size - info_before.raw_data_size;
        backup.encoded_data_size = info_after.encoded_data_size - info_before.encoded_data_size;
        backup.bundle_count = info_after.bundle_count - info_before.bundle_count;
        backup.chunk_count = info_after.chunk_count - info_before.chunk_count;
        backup.avg_chunk_size = backup.deduplicated_data_size as f32 / backup.chunk_count as f32;
        self.dirty = false;
        if failed_paths.is_empty() {
            Ok(backup)
        } else {
            Err(BackupError::FailedPaths(backup, failed_paths).into())
        }
    }

    fn export_xattrs<W: Write>(&mut self, inode: &Inode, tarfile: &mut tar::Builder<W>) -> Result<(), RepositoryError> {
        let mut data = Vec::new();
        for (key, value) in &inode.xattrs {
            let mut len_len = 1;
            let mut max_len = 10;
            let rest_len = 3 + key.len() + value.len() + PAX_XATTR_PREFIX.len();
            while rest_len + len_len >= max_len {
                len_len += 1;
                max_len *= 10;
            }
            let len = rest_len + len_len;
            let value = str::from_utf8(value).unwrap();
            let line = format!("{} {}{}={}\n", len, PAX_XATTR_PREFIX, key, value);
            assert_eq!(line.len(), len);
            data.extend_from_slice(line.as_bytes());
        }
        let mut header = tar::Header::new_ustar();
        header.set_size(data.len() as u64);
        header.set_entry_type(tar::EntryType::XHeader);
        header.set_cksum();
        Ok(try!(tarfile.append(&header, Cursor::new(&data))))
    }

    fn export_tarfile_recurse<W: Write>(&mut self, backup: &Backup, path: &Path, inode: Inode, tarfile: &mut tar::Builder<W>) -> Result<(), RepositoryError> {
        if !inode.xattrs.is_empty() {
            try!(self.export_xattrs(&inode, tarfile));
        }
        let mut header = tar::Header::new_gnu();
        header.set_size(inode.size);
        let path = path.join(inode.name);
        try!(header.set_path(&path));
        if let Some(target) = inode.symlink_target {
            try!(header.set_link_name(target));
        }
        header.set_mode(inode.mode);
        header.set_uid(inode.user);
        if let Some(name) = backup.user_names.get(&inode.user) {
            header.set_username(name).ok();
        }
        header.set_gid(inode.group);
        if let Some(name) = backup.group_names.get(&inode.group) {
            header.set_groupname(name).ok();
        }
        header.set_mtime(inode.timestamp as u64);
        header.set_entry_type(match inode.file_type {
            FileType::File => tar::EntryType::Regular,
            FileType::Symlink => tar::EntryType::Symlink,
            FileType::Directory => tar::EntryType::Directory
        });
        header.set_cksum();
        match inode.data {
            None => try!(tarfile.append(&header, Cursor::new(&[]))),
            Some(FileData::Inline(data)) => try!(tarfile.append(&header, Cursor::new(data))),
            Some(FileData::ChunkedDirect(chunks)) => try!(tarfile.append(&header, self.get_reader(chunks))),
            Some(FileData::ChunkedIndirect(chunks)) => {
                let chunks = ChunkList::read_from(&try!(self.get_data(&chunks)));
                try!(tarfile.append(&header, self.get_reader(chunks)))
            }
        }
        if let Some(children) = inode.children {
            for chunks in children.values() {
                let inode = try!(self.get_inode(chunks));
                try!(self.export_tarfile_recurse(backup, &path, inode, tarfile));
            }
        }
        Ok(())
    }

    pub fn export_tarfile<P: AsRef<Path>>(&mut self, backup: &Backup, inode: Inode, tarfile: P) -> Result<(), RepositoryError> {
        let tarfile = tarfile.as_ref();
        if tarfile == Path::new("-") {
            let mut tarfile = tar::Builder::new(io::stdout());
            try!(self.export_tarfile_recurse(backup, Path::new(""), inode, &mut tarfile));
            try!(tarfile.finish());
        } else {
            let mut tarfile = tar::Builder::new(try!(File::create(tarfile)));
            try!(self.export_tarfile_recurse(backup, Path::new(""), inode, &mut tarfile));
            try!(tarfile.finish());
        }
        Ok(())
    }

}
