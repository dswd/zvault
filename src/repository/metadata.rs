use ::prelude::*;

use filetime::{self, FileTime};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs::{self, File, Permissions};
use std::os::linux::fs::MetadataExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::io::{self, Read, Write};


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


#[derive(Debug, Eq, PartialEq)]
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


#[derive(Debug, Clone)]
pub enum FileContents {
    Inline(msgpack::Bytes),
    ChunkedDirect(ChunkList),
    ChunkedIndirect(ChunkList)
}
serde_impl!(FileContents(u8) {
    Inline(ByteBuf) => 0,
    ChunkedDirect(Vec<Chunk>) => 1,
    ChunkedIndirect(Vec<Chunk>) => 2
});


#[derive(Debug)]
pub struct Inode {
    pub name: String,
    pub size: u64,
    pub file_type: FileType,
    pub mode: u32,
    pub user: u32,
    pub group: u32,
    pub access_time: i64,
    pub modify_time: i64,
    pub create_time: i64,
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
            access_time: 0,
            modify_time: 0,
            create_time: 0,
            symlink_target: None,
            contents: None,
            children: None
        }
    }
}
serde_impl!(Inode(u8) {
    name: String => 0,
    size: u64 => 1,
    file_type: FileType => 2,
    mode: u32 => 3,
    user: u32 => 4,
    group: u32 => 5,
    access_time: i64 => 6,
    modify_time: i64 => 7,
    create_time: i64 => 8,
    symlink_target: Option<String> => 9,
    contents: Option<FileContents> => 10,
    children: BTreeMap<String, ChunkList> => 11
});


impl Inode {
    pub fn get_from<P: AsRef<Path>>(path: P) -> Result<Self, InodeError> {
        let path = path.as_ref();
        let name = try!(path.file_name()
            .ok_or_else(|| InodeError::UnsupportedFiletype(path.to_owned())))
            .to_string_lossy().to_string();
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
        inode.access_time = meta.st_atime();
        inode.modify_time = meta.st_mtime();
        inode.create_time = meta.st_ctime();
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
        try!(filetime::set_file_times(
            &full_path,
            FileTime::from_seconds_since_1970(self.access_time as u64, 0),
            FileTime::from_seconds_since_1970(self.modify_time as u64, 0)
        ).map_err(|e| InodeError::SetTimes(e, full_path.clone())));
        try!(chown(&full_path, self.user, self.group).map_err(|e| InodeError::SetOwnership(e, full_path.clone())));
        Ok(file)
    }

    pub fn is_unchanged(&self, other: &Inode) -> bool {
        self.modify_time == other.modify_time
        && self.file_type == other.file_type
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
                if reference.is_unchanged(&inode) {
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
