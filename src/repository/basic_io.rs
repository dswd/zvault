use std::mem;
use std::io::{Read, Write, Cursor};

use super::{Repository, Mode};
use super::bundle_map::BundleInfo;
use ::index::Location;

use ::util::Hash;
use ::chunker::{IChunker, ChunkerStatus};


impl Repository {
    pub fn get_chunk(&mut self, hash: Hash) -> Result<Option<Vec<u8>>, &'static str> {
        // Find bundle and chunk id in index
        let found = if let Some(found) = self.index.get(&hash) {
            found
        } else {
            return Ok(None)
        };
        // Lookup bundle id from map
        let bundle_id = if let Some(bundle_info) = self.bundle_map.get(found.bundle) {
            bundle_info.id.clone()
        } else {
            return Err("Bundle id not found in map")
        };
        // Get chunk from bundle
        if let Ok(chunk) = self.bundles.get_chunk(&bundle_id, found.chunk as usize) {
            Ok(Some(chunk))
        } else {
            Err("Failed to load chunk from bundle")
        }
    }

    pub fn put_chunk(&mut self, mode: Mode, hash: Hash, data: &[u8]) -> Result<(), &'static str> {
        // If this chunk is in the index, ignore it
        if self.index.contains(&hash) {
            return Ok(())
        }
        // Calculate the next free bundle id now (late lifetime prevents this)
        let next_free_bundle_id = self.next_free_bundle_id();
        // Select a bundle writer according to the mode and...
        let writer = match mode {
            Mode::Content => &mut self.content_bundle,
            Mode::Meta => &mut self.meta_bundle
        };
        // ...alocate one if needed
        if writer.is_none() {
            *writer = Some(try!(self.bundles.create_bundle().map_err(|_| "Failed to create new bundle")));
        }
        debug_assert!(writer.is_some());
        let chunk_id;
        let size;
        {
            // Add chunk to bundle writer and determine the size of the bundle
            let writer_obj = writer.as_mut().unwrap();
            chunk_id = try!(writer_obj.add(data).map_err(|_| "Failed to write chunk"));
            size = writer_obj.size();
        }
        let bundle_id = match mode {
            Mode::Content => self.next_content_bundle,
            Mode::Meta => self.next_meta_bundle
        };
        // Finish bundle if over maximum size
        if size >= self.config.bundle_size {
            let mut finished = None;
            mem::swap(writer, &mut finished);
            let bundle = try!(self.bundles.add_bundle(finished.unwrap()).map_err(|_| "Failed to write finished bundle"));
            let bundle_info = BundleInfo{id: bundle.id.clone()};
            self.bundle_map.set(bundle_id, bundle_info);
            if self.next_meta_bundle == bundle_id {
                self.next_meta_bundle = next_free_bundle_id
            }
            if self.next_content_bundle == bundle_id {
                self.next_content_bundle = next_free_bundle_id
            }
            // Not saving the bundle map, this will be done by flush
        }
        // Add location to the index
        try!(self.index.set(&hash, &Location::new(bundle_id, chunk_id as u32)).map_err(|_| "Failed to add chunk location to index"));
        Ok(())
    }

    #[inline]
    pub fn put_data(&mut self, mode: Mode, data: &[u8]) -> Result<Vec<(Hash, usize)>, &'static str> {
        let mut input = Cursor::new(data);
        self.put_stream(mode, &mut input)
    }

    pub fn put_stream<R: Read>(&mut self, mode: Mode, data: &mut R) -> Result<Vec<(Hash, usize)>, &'static str> {
        let avg_size = self.config.chunker.avg_size();
        let mut chunks = Vec::new();
        let mut chunk = Vec::with_capacity(avg_size * 2);
        loop {
            chunk.clear();
            let mut output = Cursor::new(chunk);
            let res = try!(self.chunker.chunk(data, &mut output).map_err(|_| "Failed to chunk"));
            chunk = output.into_inner();
            let hash = self.config.hash.hash(&chunk);
            try!(self.put_chunk(mode, hash, &chunk).map_err(|_| "Failed to store chunk"));
            chunks.push((hash, chunk.len()));
            if res == ChunkerStatus::Finished {
                break
            }
        }
        Ok(chunks)
    }

    #[inline]
    pub fn get_data(&mut self, chunks: &[(Hash, usize)]) -> Result<Vec<u8>, &'static str> {
        let mut data = Vec::with_capacity(chunks.iter().map(|&(_, size)| size).sum());
        try!(self.get_stream(chunks, &mut data));
        Ok(data)
    }

    #[inline]
    pub fn get_stream<W: Write>(&mut self, chunks: &[(Hash, usize)], w: &mut W) -> Result<(), &'static str> {
        for &(ref hash, len) in chunks {
            let data = try!(try!(self.get_chunk(*hash).map_err(|_| "Failed to load chunk")).ok_or("Chunk missing"));
            debug_assert_eq!(data.len(), len);
            try!(w.write_all(&data).map_err(|_| "Failed to write to sink"));
        }
        Ok(())
    }
}
