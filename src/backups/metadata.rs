use prelude::*;

use std::path::Path;
use std::fs::File;
use std::io::{Read, Write};

use super::*;


pub trait RepositoryMetadataIO {
    fn create_inode<P: AsRef<Path>>(&mut self, path: P, reference: Option<&Inode>, lock: &BackupMode) -> Result<Inode, RepositoryError>;
    fn put_inode(&mut self, inode: &Inode, lock: &BackupMode) -> Result<ChunkList, RepositoryError>;
    fn get_inode(&mut self, chunks: &[Chunk], lock: &OnlineMode) -> Result<Inode, RepositoryError>;
    fn save_inode_at<P: AsRef<Path>>(&mut self, inode: &Inode, path: P, lock: &OnlineMode) -> Result<(), RepositoryError>;
    fn get_inode_children(&mut self, inode: &Inode, lock: &OnlineMode) -> Result<Vec<Inode>, RepositoryError>;
}

impl RepositoryMetadataIO for Repository {
    fn create_inode<P: AsRef<Path>>(&mut self, path: P, reference: Option<&Inode>, lock: &BackupMode) -> Result<Inode, RepositoryError> {
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
                let mut chunks = try!(self.put_stream(BundleMode::Data, &mut file, lock));
                if chunks.len() < 10 {
                    inode.data = Some(FileData::ChunkedDirect(chunks));
                } else {
                    let mut chunk_data = Vec::with_capacity(chunks.encoded_size());
                    chunks.write_to(&mut chunk_data).unwrap();
                    chunks = try!(self.put_data(BundleMode::Meta, &chunk_data, lock));
                    inode.data = Some(FileData::ChunkedIndirect(chunks));
                }
            }
        }
        Ok(inode)
    }

    #[inline]
    fn put_inode(&mut self, inode: &Inode, lock: &BackupMode) -> Result<ChunkList, RepositoryError> {
        self.put_data(BundleMode::Meta, &try!(inode.encode()), lock)
    }

    #[inline]
    fn get_inode(&mut self, chunks: &[Chunk], lock: &OnlineMode) -> Result<Inode, RepositoryError> {
        Ok(try!(Inode::decode(&try!(self.get_data(chunks, lock)))))
    }

    #[inline]
    fn get_inode_children(&mut self, inode: &Inode, lock: &OnlineMode) -> Result<Vec<Inode>, RepositoryError> {
        let mut res = vec![];
        if let Some(ref children) = inode.children {
            for chunks in children.values() {
                res.push(try!(self.get_inode(chunks, lock)))
            }
        }
        Ok(res)
    }

    fn save_inode_at<P: AsRef<Path>>(&mut self, inode: &Inode, path: P, lock: &OnlineMode) -> Result<(), RepositoryError> {
        if let Some(mut file) = try!(inode.create_at(path.as_ref())) {
            if let Some(ref contents) = inode.data {
                match *contents {
                    FileData::Inline(ref data) => {
                        try!(file.write_all(data));
                    }
                    FileData::ChunkedDirect(ref chunks) => {
                        try!(self.get_stream(chunks, &mut file, lock));
                    }
                    FileData::ChunkedIndirect(ref chunks) => {
                        let chunk_data = try!(self.get_data(chunks, lock));
                        let chunks = ChunkList::read_from(&chunk_data);
                        try!(self.get_stream(&chunks, &mut file, lock));
                    }
                }
            }
        }
        Ok(())
    }
}
