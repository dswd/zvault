use ::prelude::*;

use filetime::{self, FileTime};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs::{self, File, Permissions};
use std::os::linux::fs::MetadataExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::io::{self, Read, Write};
use std::fmt;


quick_error!{
    #[derive(Debug)]
    pub enum InodeError {
        UnsupportedFiletype(path: PathBuf) {
            description("Unsupported file type")
            display("Inode error: file {:?} has an unsupported type", path)
        }
        ReadMetadata(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to obtain metadata for file")
            display("Inode error: failed to obtain metadata for file {:?}\n\tcaused by: {}", path, err)
        }
        ReadLinkTarget(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to obtain link target for file")
            display("Inode error: failed to obtain link target for file {:?}\n\tcaused by: {}", path, err)
        }
        Create(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to create entity")
            display("Inode error: failed to create entity {:?}\n\tcaused by: {}", path, err)
        }
        SetPermissions(err: io::Error, path: PathBuf, mode: u32) {
            cause(err)
            description("Failed to set permissions")
            display("Inode error: failed to set permissions to {:3o} on {:?}\n\tcaused by: {}", mode, path, err)
        }
        SetTimes(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to set file times")
            display("Inode error: failed to set file times on {:?}\n\tcaused by: {}", path, err)
        }
        SetOwnership(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to set file ownership")
            display("Inode error: failed to set file ownership on {:?}\n\tcaused by: {}", path, err)
        }
        Integrity(reason: &'static str) {
            description("Integrity error")
            display("Inode error: inode integrity error: {}", reason)
        }
        Decode(err: msgpack::DecodeError) {
            from()
            cause(err)
            description("Failed to decode metadata")
            display("Inode error: failed to decode metadata\n\tcaused by: {}", err)
        }
        Encode(err: msgpack::EncodeError) {
            from()
            cause(err)
            description("Failed to encode metadata")
            display("Inode error: failed to encode metadata\n\tcaused by: {}", err)
        }
    }
}


#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum FileType {
    File,
    Directory,
    Symlink
}
serde_impl!(FileType(u8) {
    File => 0,
    Directory => 1,
    Symlink => 2
});
impl fmt::Display for FileType {
    fn fmt(&self, format: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            FileType::File => write!(format, "file"),
            FileType::Directory => write!(format, "directory"),
            FileType::Symlink => write!(format, "symlink")
        }
    }
}


#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum FileContents {
    Inline(msgpack::Bytes),
    ChunkedDirect(ChunkList),
    ChunkedIndirect(ChunkList)
}
serde_impl!(FileContents(u8) {
    Inline(ByteBuf) => 0,
    ChunkedDirect(ChunkList) => 1,
    ChunkedIndirect(ChunkList) => 2
});


#[derive(Debug, Hash, Eq, PartialEq)]
pub struct Inode {
    pub name: String,
    pub size: u64,
    pub file_type: FileType,
    pub mode: u32,
    pub user: u32,
    pub group: u32,
    pub __old_access_time: i64,
    pub timestamp: i64,
    pub __old_create_time: i64,
    pub symlink_target: Option<String>,
    pub contents: Option<FileContents>,
    pub children: Option<BTreeMap<String, ChunkList>>
}
impl Default for Inode {
    fn default() -> Self {
        Inode {
            name: "".to_string(),
            size: 0,
            file_type: FileType::File,
            mode: 0o644,
            user: 1000,
            group: 1000,
            __old_access_time: 0,
            timestamp: 0,
            __old_create_time: 0,
            symlink_target: None,
            contents: None,
            children: None
        }
    }
}
serde_impl!(Inode(u8?) {
    name: String => 0,
    size: u64 => 1,
    file_type: FileType => 2,
    mode: u32 => 3,
    user: u32 => 4,
    group: u32 => 5,
    __old_access_time: i64 => 6,
    timestamp: i64 => 7,
    __old_create_time: i64 => 8,
    symlink_target: Option<String> => 9,
    contents: Option<FileContents> => 10,
    children: BTreeMap<String, ChunkList> => 11
});


impl Inode {
    pub fn get_from<P: AsRef<Path>>(path: P) -> Result<Self, InodeError> {
        let path = path.as_ref();
        let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "_".to_string());
        let meta = try!(fs::symlink_metadata(path).map_err(|e| InodeError::ReadMetadata(e, path.to_owned())));
        let mut inode = Inode::default();
        inode.name = name;
        if meta.is_file() {
            inode.size = meta.len();
        }
        inode.file_type = if meta.is_file() {
            FileType::File
        } else if meta.is_dir() {
            FileType::Directory
        } else if meta.file_type().is_symlink() {
            FileType::Symlink
        } else {
            return Err(InodeError::UnsupportedFiletype(path.to_owned()));
        };
        if meta.file_type().is_symlink() {
            inode.symlink_target = Some(try!(fs::read_link(path).map_err(|e| InodeError::ReadLinkTarget(e, path.to_owned()))).to_string_lossy().to_string());
        }
        inode.mode = meta.st_mode();
        inode.user = meta.st_uid();
        inode.group = meta.st_gid();
        inode.timestamp = meta.st_mtime();
        Ok(inode)
    }

    #[allow(dead_code)]
    pub fn create_at<P: AsRef<Path>>(&self, path: P) -> Result<Option<File>, InodeError> {
        let full_path = path.as_ref().join(&self.name);
        let mut file = None;
        match self.file_type {
            FileType::File => {
                file = Some(try!(File::create(&full_path).map_err(|e| InodeError::Create(e, full_path.clone()))));
            },
            FileType::Directory => {
                try!(fs::create_dir(&full_path).map_err(|e| InodeError::Create(e, full_path.clone())));
            },
            FileType::Symlink => {
                if let Some(ref src) = self.symlink_target {
                    try!(symlink(src, &full_path).map_err(|e| InodeError::Create(e, full_path.clone())));
                } else {
                    return Err(InodeError::Integrity("Symlink without target"))
                }
            }
        }
        try!(fs::set_permissions(
            &full_path,
            Permissions::from_mode(self.mode)
        ).map_err(|e| InodeError::SetPermissions(e, full_path.clone(), self.mode)));
        let time = FileTime::from_seconds_since_1970(self.timestamp as u64, 0);
        try!(filetime::set_file_times(&full_path, time, time).map_err(|e| InodeError::SetTimes(e, full_path.clone())));
        try!(chown(&full_path, self.user, self.group).map_err(|e| InodeError::SetOwnership(e, full_path.clone())));
        Ok(file)
    }

    pub fn is_same_meta(&self, other: &Inode) -> bool {
        self.file_type == other.file_type && self.size == other.size && self.mode == other.mode
        && self.user == other.user && self.group == other.group && self.name == other.name
        && self.timestamp == other.timestamp && self.symlink_target == other.symlink_target
    }

    pub fn is_same_meta_quick(&self, other: &Inode) -> bool {
        self.timestamp == other.timestamp
        && self.file_type == other.file_type
        && self.size == other.size
    }

    #[inline]
    pub fn encode(&self) -> Result<Vec<u8>, InodeError> {
        Ok(try!(msgpack::encode(&self)))
    }

    #[inline]
    pub fn decode(data: &[u8]) -> Result<Self, InodeError> {
        Ok(try!(msgpack::decode(&data)))
    }
}


impl Repository {
    pub fn create_inode<P: AsRef<Path>>(&mut self, path: P, reference: Option<&Inode>) -> Result<Inode, RepositoryError> {
        let mut inode = try!(Inode::get_from(path.as_ref()));
        if inode.file_type == FileType::File && inode.size > 0 {
            if let Some(reference) = reference {
                if reference.is_same_meta_quick(&inode) {
                    inode.contents = reference.contents.clone();
                    return Ok(inode)
                }
            }
            let mut file = try!(File::open(path));
            if inode.size < 100 {
                let mut data = Vec::with_capacity(inode.size as usize);
                try!(file.read_to_end(&mut data));
                inode.contents = Some(FileContents::Inline(data.into()));
            } else {
                let mut chunks = try!(self.put_stream(BundleMode::Content, &mut file));
                if chunks.len() < 10 {
                    inode.contents = Some(FileContents::ChunkedDirect(chunks));
                } else {
                    let mut chunk_data = Vec::with_capacity(chunks.encoded_size());
                    chunks.write_to(&mut chunk_data).unwrap();
                    chunks = try!(self.put_data(BundleMode::Meta, &chunk_data));
                    inode.contents = Some(FileContents::ChunkedIndirect(chunks));
                }
            }
        }
        Ok(inode)
    }

    #[inline]
    pub fn put_inode(&mut self, inode: &Inode) -> Result<ChunkList, RepositoryError> {
        self.put_data(BundleMode::Meta, &try!(inode.encode()))
    }

    #[inline]
    pub fn get_inode(&mut self, chunks: &[Chunk]) -> Result<Inode, RepositoryError> {
        Ok(try!(Inode::decode(&try!(self.get_data(chunks)))))
    }

    #[inline]
    pub fn save_inode_at<P: AsRef<Path>>(&mut self, inode: &Inode, path: P) -> Result<(), RepositoryError> {
        if let Some(mut file) = try!(inode.create_at(path.as_ref())) {
            if let Some(ref contents) = inode.contents {
                match *contents {
                    FileContents::Inline(ref data) => {
                        try!(file.write_all(&data));
                    },
                    FileContents::ChunkedDirect(ref chunks) => {
                        try!(self.get_stream(chunks, &mut file));
                    },
                    FileContents::ChunkedIndirect(ref chunks) => {
                        let chunk_data = try!(self.get_data(chunks));
                        let chunks = ChunkList::read_from(&chunk_data);
                        try!(self.get_stream(&chunks, &mut file));
                    }
                }
            }
        }
        Ok(())
    }
}
