use serde::bytes::ByteBuf;
use serde::{Serialize, Deserialize};
use rmp_serde;

use std::collections::HashMap;
use std::path::Path;
use std::fs::{self, Metadata, File};
use std::os::linux::fs::MetadataExt;
use std::io::{Cursor, Read};

use ::util::Hash;
use super::{Repository, Mode, Chunk};


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


#[derive(Debug)]
pub enum FileContents {
    Inline(ByteBuf),
    Chunked(Vec<Chunk>)
}
serde_impl!(FileContents(u8) {
    Inline(ByteBuf) => 0,
    Chunked(Vec<Chunk>) => 1
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
    pub children: Option<HashMap<String, Vec<Chunk>>>
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
    children: HashMap<String, Vec<Chunk>> => 11
});

impl Inode {
    fn get_extended_attrs_from(&mut self, meta: &Metadata) -> Result<(), &'static str> {
        self.mode = meta.st_mode();
        self.user = meta.st_uid();
        self.group = meta.st_gid();
        self.access_time = meta.st_atime();
        self.modify_time = meta.st_mtime();
        self.create_time = meta.st_ctime();
        Ok(())
    }

    pub fn get_from<P: AsRef<Path>>(path: P) -> Result<Self, &'static str> {
        let name = try!(path.as_ref().file_name().ok_or("Not a file")).to_string_lossy().to_string();
        let meta = try!(fs::symlink_metadata(path.as_ref()).map_err(|_| "Failed to get metadata"));
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
            return Err("Unsupported file type");
        };
        if meta.file_type().is_symlink() {
            inode.symlink_target = Some(try!(fs::read_link(path).map_err(|_| "Failed to read symlink")).to_string_lossy().to_string());
        }
        try!(inode.get_extended_attrs_from(&meta));
        Ok(inode)
    }

    #[allow(dead_code)]
    pub fn create_at<P: AsRef<Path>>(&self, path: P) -> Result<(), &'static str> {
        let full_path = path.as_ref().join(&self.name);
        match self.file_type {
            FileType::File => {
                try!(File::create(&full_path).map_err(|_| "Failed to create file"));
            },
            FileType::Directory => {
                try!(fs::create_dir(&full_path).map_err(|_| "Failed to create directory"));
            },
            FileType::Symlink => {
                if let Some(ref src) = self.symlink_target {
                    try!(fs::soft_link(src, &full_path).map_err(|_| "Failed to create symlink"));
                } else {
                    return Err("Symlink without destination")
                }
            }
        }
        //FIXME: set times and permissions
        Ok(())
    }
}


impl Repository {
    pub fn put_inode<P: AsRef<Path>>(&mut self, path: P) -> Result<Vec<Chunk>, &'static str> {
        let mut inode = try!(Inode::get_from(path.as_ref()));
        if inode.file_type == FileType::File && inode.size > 0 {
            let mut file = try!(File::open(path).map_err(|_| "Failed to open file"));
            if inode.size < 100 {
                let mut data = Vec::with_capacity(inode.size as usize);
                try!(file.read_to_end(&mut data).map_err(|_| "Failed to read file contents"));
                inode.contents = Some(FileContents::Inline(data.into()));
            } else {
                let chunks = try!(self.put_stream(Mode::Content, &mut file));
                inode.contents = Some(FileContents::Chunked(chunks));
            }
        }
        let mut inode_data = Vec::new();
        {
            let mut writer = rmp_serde::Serializer::new(&mut inode_data);
            inode.serialize(&mut writer).map_err(|_| "Failed to write inode data");
        }
        self.put_data(Mode::Meta, &inode_data)
    }

    #[inline]
    pub fn get_inode(&mut self, chunks: &[Chunk]) -> Result<Inode, &'static str> {
        let data = Cursor::new(try!(self.get_data(chunks)));
        let mut reader = rmp_serde::Deserializer::new(data);
        Inode::deserialize(&mut reader).map_err(|_| "Failed to read inode data")
    }
}
