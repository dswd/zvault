use prelude::*;

use std::mem;
use std::cmp::min;
use std::collections::VecDeque;
use std::io::{self, Read, Write, Cursor};


pub struct ChunkReader<'a> {
    chunks: VecDeque<Chunk>,
    data: Vec<u8>,
    pos: usize,
    repo: &'a mut Repository
}

impl<'a> ChunkReader<'a> {
    pub fn new(repo: &'a mut Repository, chunks: ChunkList) -> Self {
        ChunkReader {
            repo,
            chunks: chunks.into_inner().into(),
            data: vec![],
            pos: 0
        }
    }
}

impl<'a> Read for ChunkReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let mut bpos = 0;
        loop {
            if buf.len() == bpos {
                break;
            }
            if self.data.len() == self.pos {
                if let Some(chunk) = self.chunks.pop_front() {
                    self.data = match self.repo.get_chunk(chunk.0) {
                        Ok(Some(data)) => data,
                        Ok(None) => {
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                IntegrityError::MissingChunk(chunk.0)
                            ))
                        }
                        Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err)),
                    };
                    self.pos = 0;
                } else {
                    break;
                }
            }
            let l = min(self.data.len() - self.pos, buf.len() - bpos);
            buf[bpos..bpos + l].copy_from_slice(&self.data[self.pos..self.pos + l]);
            bpos += l;
            self.pos += l;
        }
        Ok(bpos)
    }
}


impl Repository {
    #[inline]
    pub fn get_bundle_id(&self, id: u32) -> Result<BundleId, RepositoryError> {
        self.bundle_map.get(id).ok_or_else(|| {
            IntegrityError::MissingBundleId(id).into()
        })
    }

    pub fn get_chunk(&mut self, hash: Hash) -> Result<Option<Vec<u8>>, RepositoryError> {
        // Find bundle and chunk id in index
        let found = if let Some(found) = self.index.get(&hash) {
            found
        } else {
            return Ok(None);
        };
        // Lookup bundle id from map
        let bundle_id = try!(self.get_bundle_id(found.bundle));
        // Get chunk from bundle
        Ok(Some(try!(
            self.bundles.get_chunk(&bundle_id, found.chunk as usize)
        )))
    }

    #[inline]
    pub fn put_chunk(
        &mut self,
        mode: BundleMode,
        hash: Hash,
        data: &[u8],
    ) -> Result<(), RepositoryError> {
        // If this chunk is in the index, ignore it
        if self.index.contains(&hash) {
            return Ok(());
        }
        self.put_chunk_override(mode, hash, data)
    }

    fn write_chunk_to_bundle_and_index(
        &mut self,
        mode: BundleMode,
        hash: Hash,
        data: &[u8],
    ) -> Result<(), RepositoryError> {
        let writer = match mode {
            BundleMode::Data => &mut self.data_bundle,
            BundleMode::Meta => &mut self.meta_bundle,
        };
        // ...alocate one if needed
        if writer.is_none() {
            *writer = Some(try!(self.bundles.create_bundle(
                mode,
                self.config.hash,
                self.config.compression.clone(),
                self.config.encryption.clone()
            )));
        }
        debug_assert!(writer.is_some());
        // Add chunk to bundle writer and determine the size of the bundle
        let writer_obj = writer.as_mut().unwrap();
        let chunk_id = try!(writer_obj.add(data, hash));
        let bundle_id = match mode {
            BundleMode::Data => self.next_data_bundle,
            BundleMode::Meta => self.next_meta_bundle,
        };
        // Add location to the index
        try!(self.index.set(
            &hash,
            &Location::new(bundle_id, chunk_id as u32)
        ));
        Ok(())
    }

    fn finish_bundle(&mut self, mode: BundleMode) -> Result<(), RepositoryError> {
        // Calculate the next free bundle id now (late lifetime prevents this)
        let next_free_bundle_id = self.next_free_bundle_id();
        let writer = match mode {
            BundleMode::Data => &mut self.data_bundle,
            BundleMode::Meta => &mut self.meta_bundle,
        };
        if writer.is_none() {
            return Ok(());
        }
        let bundle_id = match mode {
            BundleMode::Data => self.next_data_bundle,
            BundleMode::Meta => self.next_meta_bundle,
        };
        let mut finished = None;
        mem::swap(writer, &mut finished);
        let bundle = try!(self.bundles.add_bundle(finished.unwrap()));
        self.bundle_map.set(bundle_id, bundle.id.clone());
        if self.next_meta_bundle == bundle_id {
            self.next_meta_bundle = next_free_bundle_id
        }
        if self.next_data_bundle == bundle_id {
            self.next_data_bundle = next_free_bundle_id
        }
        Ok(())
    }

    fn finish_bundle_if_needed(&mut self, mode: BundleMode) -> Result<(), RepositoryError> {
        let (size, raw_size) = {
            let writer = match mode {
                BundleMode::Data => &mut self.data_bundle,
                BundleMode::Meta => &mut self.meta_bundle,
            };
            if let Some(ref writer) = *writer {
                (writer.estimate_final_size(), writer.raw_size())
            } else {
                return Ok(());
            }
        };
        if size >= self.config.bundle_size || raw_size >= 4 * self.config.bundle_size {
            if mode == BundleMode::Meta {
                //First store the current data bundle as meta referrs to those chunks
                try!(self.finish_bundle(BundleMode::Data))
            }
            try!(self.finish_bundle(mode))
        }
        Ok(())
    }

    #[inline]
    pub fn put_chunk_override(
        &mut self,
        mode: BundleMode,
        hash: Hash,
        data: &[u8],
    ) -> Result<(), RepositoryError> {
        try!(self.write_chunk_to_bundle_and_index(mode, hash, data));
        self.finish_bundle_if_needed(mode)
    }

    #[inline]
    pub fn put_data(
        &mut self,
        mode: BundleMode,
        data: &[u8],
    ) -> Result<ChunkList, RepositoryError> {
        let mut input = Cursor::new(data);
        self.put_stream(mode, &mut input)
    }

    pub fn put_stream<R: Read>(
        &mut self,
        mode: BundleMode,
        data: &mut R,
    ) -> Result<ChunkList, RepositoryError> {
        let avg_size = self.config.chunker.avg_size();
        let mut chunks = Vec::new();
        let mut chunk = Vec::with_capacity(avg_size * 2);
        loop {
            chunk.clear();
            let mut output = Cursor::new(chunk);
            let res = try!(self.chunker.chunk(data, &mut output));
            chunk = output.into_inner();
            let hash = self.config.hash.hash(&chunk);
            try!(self.put_chunk(mode, hash, &chunk));
            chunks.push((hash, chunk.len() as u32));
            if res == ChunkerStatus::Finished {
                break;
            }
        }
        Ok(chunks.into())
    }

    pub fn get_data(&mut self, chunks: &[Chunk]) -> Result<Vec<u8>, RepositoryError> {
        let mut data =
            Vec::with_capacity(chunks.iter().map(|&(_, size)| size).sum::<u32>() as usize);
        try!(self.get_stream(chunks, &mut data));
        Ok(data)
    }

    #[inline]
    pub fn get_reader(&mut self, chunks: ChunkList) -> ChunkReader {
        ChunkReader::new(self, chunks)
    }

    pub fn get_stream<W: Write>(
        &mut self,
        chunks: &[Chunk],
        w: &mut W,
    ) -> Result<(), RepositoryError> {
        for &(ref hash, len) in chunks {
            let data = try!(try!(self.get_chunk(*hash)).ok_or_else(|| {
                IntegrityError::MissingChunk(*hash)
            }));
            debug_assert_eq!(data.len() as u32, len);
            try!(w.write_all(&data));
        }
        Ok(())
    }
}
