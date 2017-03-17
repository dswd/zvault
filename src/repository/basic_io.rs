use std::mem;
use std::io::{Read, Write, Cursor};

use super::{Repository, RepositoryError};
use ::index::Location;
use ::bundle::{BundleId, BundleMode};
use super::integrity::RepositoryIntegrityError;

use ::util::Hash;
use ::chunker::{IChunker, ChunkerStatus};


pub type Chunk = (Hash, usize);


impl Repository {
    pub fn get_bundle_id(&self, id: u32) -> Result<BundleId, RepositoryError> {
        if let Some(bundle_info) = self.bundle_map.get(id) {
            Ok(bundle_info.id())
        } else {
            Err(RepositoryIntegrityError::MissingBundleId(id).into())
        }
    }

    pub fn get_chunk(&mut self, hash: Hash) -> Result<Option<Vec<u8>>, RepositoryError> {
        // Find bundle and chunk id in index
        let found = if let Some(found) = self.index.get(&hash) {
            found
        } else {
            return Ok(None)
        };
        // Lookup bundle id from map
        let bundle_id = try!(self.get_bundle_id(found.bundle));
        // Get chunk from bundle
        Ok(Some(try!(self.bundles.get_chunk(&bundle_id, found.chunk as usize))))
    }

    pub fn put_chunk(&mut self, mode: BundleMode, hash: Hash, data: &[u8]) -> Result<(), RepositoryError> {
        // If this chunk is in the index, ignore it
        if self.index.contains(&hash) {
            return Ok(())
        }
        // Calculate the next free bundle id now (late lifetime prevents this)
        let next_free_bundle_id = self.next_free_bundle_id();
        // Select a bundle writer according to the mode and...
        let writer = match mode {
            BundleMode::Content => &mut self.content_bundle,
            BundleMode::Meta => &mut self.meta_bundle
        };
        // ...alocate one if needed
        if writer.is_none() {
            *writer = Some(try!(self.bundles.create_bundle(mode)));
        }
        debug_assert!(writer.is_some());
        let chunk_id;
        let size;
        let raw_size;
        {
            // Add chunk to bundle writer and determine the size of the bundle
            let writer_obj = writer.as_mut().unwrap();
            chunk_id = try!(writer_obj.add(data));
            size = writer_obj.size();
            raw_size = writer_obj.raw_size();
        }
        let bundle_id = match mode {
            BundleMode::Content => self.next_content_bundle,
            BundleMode::Meta => self.next_meta_bundle
        };
        // Finish bundle if over maximum size
        if size >= self.config.bundle_size || raw_size >= 4 * self.config.bundle_size {
            let mut finished = None;
            mem::swap(writer, &mut finished);
            let bundle = try!(self.bundles.add_bundle(finished.unwrap()));
            self.bundle_map.set(bundle_id, bundle);
            if self.next_meta_bundle == bundle_id {
                self.next_meta_bundle = next_free_bundle_id
            }
            if self.next_content_bundle == bundle_id {
                self.next_content_bundle = next_free_bundle_id
            }
            // Not saving the bundle map, this will be done by flush
        }
        // Add location to the index
        try!(self.index.set(&hash, &Location::new(bundle_id, chunk_id as u32)));
        Ok(())
    }

    #[inline]
    pub fn put_data(&mut self, mode: BundleMode, data: &[u8]) -> Result<Vec<Chunk>, RepositoryError> {
        let mut input = Cursor::new(data);
        self.put_stream(mode, &mut input)
    }

    pub fn put_stream<R: Read>(&mut self, mode: BundleMode, data: &mut R) -> Result<Vec<Chunk>, RepositoryError> {
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
            chunks.push((hash, chunk.len()));
            if res == ChunkerStatus::Finished {
                break
            }
        }
        Ok(chunks)
    }

    #[inline]
    pub fn get_data(&mut self, chunks: &[Chunk]) -> Result<Vec<u8>, RepositoryError> {
        let mut data = Vec::with_capacity(chunks.iter().map(|&(_, size)| size).sum());
        try!(self.get_stream(chunks, &mut data));
        Ok(data)
    }

    #[inline]
    pub fn get_stream<W: Write>(&mut self, chunks: &[Chunk], w: &mut W) -> Result<(), RepositoryError> {
        for &(ref hash, len) in chunks {
            let data = try!(try!(self.get_chunk(*hash)).ok_or_else(|| RepositoryIntegrityError::MissingChunk(hash.clone())));
            debug_assert_eq!(data.len(), len);
            try!(w.write_all(&data));
        }
        Ok(())
    }
}
