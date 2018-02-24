use prelude::*;

use filetime::{self, FileTime};
use xattr;
use libc;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs::{self, File, Permissions};
use std::os::linux::fs::MetadataExt;
use std::os::unix::fs::{FileTypeExt, PermissionsExt, MetadataExt as UnixMetadataExt, symlink};
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::fmt;
use std::ffi;


quick_error!{
    #[derive(Debug)]
    pub enum InodeError {
        UnsupportedFiletype(path: PathBuf) {
            description(tr!("Unsupported file type"))
            display("{}", tr_format!("Inode error: file {:?} has an unsupported type", path))
        }
        ReadMetadata(err: io::Error, path: PathBuf) {
            cause(err)
            description(tr!("Failed to obtain metadata for file"))
            display("{}", tr_format!("Inode error: failed to obtain metadata for file {:?}\n\tcaused by: {}", path, err))
        }
        ReadXattr(err: io::Error, path: PathBuf) {
            cause(err)
            description(tr!("Failed to obtain xattr for file"))
            display("{}", tr_format!("Inode error: failed to obtain xattr for file {:?}\n\tcaused by: {}", path, err))
        }
        ReadLinkTarget(err: io::Error, path: PathBuf) {
            cause(err)
            description(tr!("Failed to obtain link target for file"))
            display("{}", tr_format!("Inode error: failed to obtain link target for file {:?}\n\tcaused by: {}", path, err))
        }
        Create(err: io::Error, path: PathBuf) {
            cause(err)
            description(tr!("Failed to create entity"))
            display("{}", tr_format!("Inode error: failed to create entity {:?}\n\tcaused by: {}", path, err))
        }
        Integrity(reason: &'static str) {
            description(tr!("Integrity error"))
            display("{}", tr_format!("Inode error: inode integrity error: {}", reason))
        }
        Decode(err: msgpack::DecodeError) {
            from()
            cause(err)
            description(tr!("Failed to decode metadata"))
            display("{}", tr_format!("Inode error: failed to decode metadata\n\tcaused by: {}", err))
        }
        Encode(err: msgpack::EncodeError) {
            from()
            cause(err)
            description(tr!("Failed to encode metadata"))
            display("{}", tr_format!("Inode error: failed to encode metadata\n\tcaused by: {}", err))
        }
    }
}


#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    BlockDevice,
    CharDevice,
    NamedPipe
}
serde_impl!(FileType(u8) {
    File => 0,
    Directory => 1,
    Symlink => 2,
    BlockDevice => 3,
    CharDevice => 4,
    NamedPipe => 5
});
impl fmt::Display for FileType {
    fn fmt(&self, format: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            FileType::File => write!(format, "{}", tr!("file")),
            FileType::Directory => write!(format, "{}", tr!("directory")),
            FileType::Symlink => write!(format, "{}", tr!("symlink")),
            FileType::BlockDevice => write!(format, "{}", tr!("block device")),
            FileType::CharDevice => write!(format, "{}", tr!("char device")),
            FileType::NamedPipe => write!(format, "{}", tr!("named pipe")),
        }
    }
}


#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum FileData {
    Inline(msgpack::Bytes),
    ChunkedDirect(ChunkList),
    ChunkedIndirect(ChunkList)
}
serde_impl!(FileData(u8) {
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
    pub timestamp: i64,
    pub symlink_target: Option<String>,
    pub data: Option<FileData>,
    pub children: Option<BTreeMap<String, ChunkList>>,
    pub cum_size: u64,
    pub cum_dirs: usize,
    pub cum_files: usize,
    pub xattrs: BTreeMap<String, msgpack::Bytes>,
    pub device: Option<(u32, u32)>
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
            timestamp: 0,
            symlink_target: None,
            data: None,
            children: None,
            cum_size: 0,
            cum_dirs: 0,
            cum_files: 0,
            xattrs: BTreeMap::new(),
            device: None
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
    timestamp: i64 => 7,
    symlink_target: Option<String> => 9,
    data: Option<FileData> => 10,
    children: Option<BTreeMap<String, ChunkList>> => 11,
    cum_size: u64 => 12,
    cum_dirs: usize => 13,
    cum_files: usize => 14,
    xattrs: BTreeMap<String, msgpack::Bytes> => 15,
    device: Option<(u32, u32)> => 16
});


impl Inode {
    pub fn get_from<P: AsRef<Path>>(path: P) -> Result<Self, InodeError> {
        let path = path.as_ref();
        let name = path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "_".to_string());
        let meta = try!(fs::symlink_metadata(path).map_err(|e| {
            InodeError::ReadMetadata(e, path.to_owned())
        }));
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
        } else if meta.file_type().is_block_device() {
            FileType::BlockDevice
        } else if meta.file_type().is_char_device() {
            FileType::CharDevice
        } else if meta.file_type().is_fifo() {
            FileType::NamedPipe
        } else {
            return Err(InodeError::UnsupportedFiletype(path.to_owned()));
        };
        if meta.file_type().is_symlink() {
            inode.symlink_target = Some(
                try!(fs::read_link(path).map_err(|e| {
                    InodeError::ReadLinkTarget(e, path.to_owned())
                })).to_string_lossy()
                    .to_string()
            );
        }
        if meta.file_type().is_block_device() || meta.file_type().is_char_device() {
            let rdev = meta.rdev();
            let major = (rdev >> 8) as u32;
            let minor = (rdev & 0xff) as u32;
            inode.device = Some((major, minor));
        }
        inode.mode = meta.permissions().mode();
        inode.user = meta.st_uid();
        inode.group = meta.st_gid();
        inode.timestamp = meta.st_mtime();
        if xattr::SUPPORTED_PLATFORM {
            if let Ok(attrs) = xattr::list(path) {
                for name in attrs {
                    if let Some(data) = try!(xattr::get(path, &name).map_err(|e| {
                        InodeError::ReadXattr(e, path.to_owned())
                    }))
                    {
                        inode.xattrs.insert(
                            name.to_string_lossy().to_string(),
                            data.into()
                        );
                    }
                }
            }
        }
        Ok(inode)
    }

    pub fn create_at<P: AsRef<Path>>(&self, path: P) -> Result<Option<File>, InodeError> {
        let full_path = path.as_ref().join(&self.name);
        let mut file = None;
        match self.file_type {
            FileType::File => {
                file = Some(try!(File::create(&full_path).map_err(|e| {
                    InodeError::Create(e, full_path.clone())
                })));
            }
            FileType::Directory => {
                try!(fs::create_dir(&full_path).map_err(|e| {
                    InodeError::Create(e, full_path.clone())
                }));
            }
            FileType::Symlink => {
                if let Some(ref src) = self.symlink_target {
                    try!(symlink(src, &full_path).map_err(|e| {
                        InodeError::Create(e, full_path.clone())
                    }));
                } else {
                    return Err(InodeError::Integrity(tr!("Symlink without target")));
                }
            }
            FileType::NamedPipe => {
                let name = try!(
                    ffi::CString::new(full_path.as_os_str().as_bytes())
                        .map_err(|_| InodeError::Integrity(tr!("Name contains nulls")))
                );
                let mode = self.mode | libc::S_IFIFO;
                if unsafe { libc::mkfifo(name.as_ptr(), mode) } != 0 {
                    return Err(InodeError::Create(
                        io::Error::last_os_error(),
                        full_path.clone()
                    ));
                }
            }
            FileType::BlockDevice | FileType::CharDevice => {
                let name = try!(
                    ffi::CString::new(full_path.as_os_str().as_bytes())
                        .map_err(|_| InodeError::Integrity(tr!("Name contains nulls")))
                );
                let mode = self.mode |
                    match self.file_type {
                        FileType::BlockDevice => libc::S_IFBLK,
                        FileType::CharDevice => libc::S_IFCHR,
                        _ => unreachable!(),
                    };
                let device = if let Some((major, minor)) = self.device {
                    unsafe { libc::makedev(major, minor) }
                } else {
                    return Err(InodeError::Integrity(tr!("Device without id")));
                };
                if unsafe { libc::mknod(name.as_ptr(), mode, device) } != 0 {
                    return Err(InodeError::Create(
                        io::Error::last_os_error(),
                        full_path.clone()
                    ));
                }
            }
        }
        let time = FileTime::from_seconds_since_1970(self.timestamp as u64, 0);
        if let Err(err) = filetime::set_file_times(&full_path, time, time) {
            tr_warn!("Failed to set file time on {:?}: {}", full_path, err);
        }
        if !self.xattrs.is_empty() {
            if xattr::SUPPORTED_PLATFORM {
                for (name, data) in &self.xattrs {
                    if let Err(err) = xattr::set(&full_path, name, data) {
                        tr_warn!("Failed to set xattr {} on {:?}: {}", name, full_path, err);
                    }
                }
            } else {
                tr_warn!("Not setting xattr on {:?}", full_path);
            }
        }
        if let Err(err) = fs::set_permissions(&full_path, Permissions::from_mode(self.mode)) {
            tr_warn!(
                "Failed to set permissions {:o} on {:?}: {}",
                self.mode,
                full_path,
                err
            );
        }
        if let Err(err) = chown(&full_path, self.user, self.group) {
            tr_warn!(
                "Failed to set user {} and group {} on {:?}: {}",
                self.user,
                self.group,
                full_path,
                err
            );
        }
        Ok(file)
    }

    #[inline]
    pub fn is_same_meta(&self, other: &Inode) -> bool {
        self.file_type == other.file_type && self.size == other.size &&
            self.mode == other.mode && self.user == other.user &&
            self.group == other.group && self.name == other.name &&
            self.timestamp == other.timestamp && self.symlink_target == other.symlink_target
    }

    #[inline]
    pub fn is_same_meta_quick(&self, other: &Inode) -> bool {
        self.timestamp == other.timestamp && self.file_type == other.file_type &&
            self.size == other.size
    }

    #[inline]
    pub fn encode(&self) -> Result<Vec<u8>, InodeError> {
        Ok(try!(msgpack::encode(&self)))
    }

    #[inline]
    pub fn decode(data: &[u8]) -> Result<Self, InodeError> {
        Ok(try!(msgpack::decode(data)))
    }
}


impl Repository {
    pub fn create_inode<P: AsRef<Path>>(
        &mut self,
        path: P,
        reference: Option<&Inode>,
    ) -> Result<Inode, RepositoryError> {
        let mut inode = try!(Inode::get_from(path.as_ref()));
        if inode.file_type == FileType::File && inode.size > 0 {
            if let Some(reference) = reference {
                if reference.is_same_meta_quick(&inode) {
                    inode.data = reference.data.clone();
                    return Ok(inode);
                }
            }
            let mut file = try!(File::open(path));
            if inode.size < 100 {
                let mut data = Vec::with_capacity(inode.size as usize);
                try!(file.read_to_end(&mut data));
                inode.data = Some(FileData::Inline(data.into()));
            } else {
                let mut chunks = try!(self.put_stream(BundleMode::Data, &mut file));
                if chunks.len() < 10 {
                    inode.data = Some(FileData::ChunkedDirect(chunks));
                } else {
                    let mut chunk_data = Vec::with_capacity(chunks.encoded_size());
                    chunks.write_to(&mut chunk_data).unwrap();
                    chunks = try!(self.put_data(BundleMode::Meta, &chunk_data));
                    inode.data = Some(FileData::ChunkedIndirect(chunks));
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

    pub fn save_inode_at<P: AsRef<Path>>(
        &mut self,
        inode: &Inode,
        path: P,
    ) -> Result<(), RepositoryError> {
        if let Some(mut file) = try!(inode.create_at(path.as_ref())) {
            if let Some(ref contents) = inode.data {
                match *contents {
                    FileData::Inline(ref data) => {
                        try!(file.write_all(data));
                    }
                    FileData::ChunkedDirect(ref chunks) => {
                        try!(self.get_stream(chunks, &mut file));
                    }
                    FileData::ChunkedIndirect(ref chunks) => {
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
