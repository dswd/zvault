use ::prelude::*;

use std::collections::HashMap;
use std::path::Path;
use std::fs::{self, Metadata, File, Permissions};
use std::os::linux::fs::MetadataExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::io::{Read, Write};


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
    pub children: Option<HashMap<String, ChunkList>>
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
    children: HashMap<String, ChunkList> => 11
});


impl Inode {
    #[inline]
    fn get_extended_attrs_from(&mut self, meta: &Metadata) -> Result<(), RepositoryError> {
        self.mode = meta.st_mode();
        self.user = meta.st_uid();
        self.group = meta.st_gid();
        self.access_time = meta.st_atime();
        self.modify_time = meta.st_mtime();
        self.create_time = meta.st_ctime();
        Ok(())
    }

    pub fn get_from<P: AsRef<Path>>(path: P) -> Result<Self, RepositoryError> {
        let name = try!(path.as_ref().file_name()
            .ok_or_else(|| RepositoryError::InvalidFileType(path.as_ref().to_owned())))
            .to_string_lossy().to_string();
        let meta = try!(fs::symlink_metadata(path.as_ref()));
        let mut inode = Inode::default();
        inode.name = name;
        inode.size = meta.len();
        inode.file_type = if meta.is_file() {
            FileType::File
        } else if meta.is_dir() {
            FileType::Directory
        } else if meta.file_type().is_symlink() {
            FileType::Symlink
        } else {
            return Err(RepositoryError::InvalidFileType(path.as_ref().to_owned()));
        };
        if meta.file_type().is_symlink() {
            inode.symlink_target = Some(try!(fs::read_link(path)).to_string_lossy().to_string());
        }
        try!(inode.get_extended_attrs_from(&meta));
        Ok(inode)
    }

    #[allow(dead_code)]
    pub fn create_at<P: AsRef<Path>>(&self, path: P) -> Result<Option<File>, RepositoryError> {
        let full_path = path.as_ref().join(&self.name);
        let mut file = None;
        match self.file_type {
            FileType::File => {
                file = Some(try!(File::create(&full_path)));
            },
            FileType::Directory => {
                try!(fs::create_dir(&full_path));
            },
            FileType::Symlink => {
                if let Some(ref src) = self.symlink_target {
                    try!(symlink(src, &full_path));
                } else {
                    return Err(RepositoryIntegrityError::SymlinkWithoutTarget.into())
                }
            }
        }
        try!(fs::set_permissions(&full_path, Permissions::from_mode(self.mode)));
        //FIXME: set times and gid/uid
        // https://crates.io/crates/filetime
        Ok(file)
    }

    pub fn is_unchanged(&self, other: &Inode) -> bool {
        self.modify_time == other.modify_time
        && self.create_time == other.create_time
        && self.file_type == other.file_type
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
        self.put_data(BundleMode::Meta, &try!(msgpack::encode(inode)))
    }

    #[inline]
    pub fn get_inode(&mut self, chunks: &[Chunk]) -> Result<Inode, RepositoryError> {
        Ok(try!(msgpack::decode(&try!(self.get_data(chunks)))))
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
