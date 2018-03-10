use prelude::*;

use std::path::Path;
use std::fs::File;
use std::io::{Read, Write};


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
