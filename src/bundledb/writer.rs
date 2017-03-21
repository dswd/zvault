use ::prelude::*;
use super::*;

use std::path::Path;
use std::fs::{self, File};
use std::io::{Write, Seek, SeekFrom, BufWriter};
use std::sync::{Arc, Mutex};


pub struct BundleWriter {
    mode: BundleMode,
    hash_method: HashMethod,
    data: Vec<u8>,
    compression: Option<Compression>,
    compression_stream: Option<CompressionStream>,
    encryption: Option<Encryption>,
    crypto: Arc<Mutex<Crypto>>,
    raw_size: usize,
    chunk_count: usize,
    chunks: ChunkList,
}

impl BundleWriter {
    pub fn new(
        mode: BundleMode,
        hash_method: HashMethod,
        compression: Option<Compression>,
        encryption: Option<Encryption>,
        crypto: Arc<Mutex<Crypto>>
    ) -> Result<Self, BundleError> {
        let compression_stream = match compression {
            Some(ref compression) => Some(try!(compression.compress_stream())),
            None => None
        };
        Ok(BundleWriter {
            mode: mode,
            hash_method: hash_method,
            data: vec![],
            compression: compression,
            compression_stream: compression_stream,
            encryption: encryption,
            crypto: crypto,
            raw_size: 0,
            chunk_count: 0,
            chunks: ChunkList::new()
        })
    }

    pub fn add(&mut self, chunk: &[u8], hash: Hash) -> Result<usize, BundleError> {
        if let Some(ref mut stream) = self.compression_stream {
            try!(stream.process(chunk, &mut self.data))
        } else {
            self.data.extend_from_slice(chunk)
        }
        self.raw_size += chunk.len();
        self.chunk_count += 1;
        self.chunks.push((hash, chunk.len() as u32));
        Ok(self.chunk_count-1)
    }

    pub fn finish(mut self, db: &BundleDb) -> Result<Bundle, BundleError> {
        if let Some(stream) = self.compression_stream {
            try!(stream.finish(&mut self.data))
        }
        if let Some(ref encryption) = self.encryption {
            self.data = try!(self.crypto.lock().unwrap().encrypt(&encryption, &self.data));
        }
        let encoded_size = self.data.len();
        let mut chunk_data = Vec::with_capacity(self.chunks.encoded_size());
        self.chunks.write_to(&mut chunk_data).unwrap();
        let id = BundleId(self.hash_method.hash(&chunk_data));
        if let Some(ref encryption) = self.encryption {
            chunk_data = try!(self.crypto.lock().unwrap().encrypt(&encryption, &chunk_data));
        }
        let (folder, file) = db.bundle_path(&id);
        let path = folder.join(file);
        try!(fs::create_dir_all(&folder).context(&path as &Path));
        let mut file = BufWriter::new(try!(File::create(&path).context(&path as &Path)));
        try!(file.write_all(&HEADER_STRING).context(&path as &Path));
        try!(file.write_all(&[HEADER_VERSION]).context(&path as &Path));
        let header = BundleInfo {
            mode: self.mode,
            hash_method: self.hash_method,
            compression: self.compression,
            encryption: self.encryption,
            chunk_count: self.chunk_count,
            id: id.clone(),
            raw_size: self.raw_size,
            encoded_size: encoded_size,
            chunk_info_size: chunk_data.len()
        };
        try!(msgpack::encode_to_stream(&header, &mut file).context(&path as &Path));
        try!(file.write_all(&chunk_data).context(&path as &Path));
        let content_start = file.seek(SeekFrom::Current(0)).unwrap() as usize;
        try!(file.write_all(&self.data).context(&path as &Path));
        Ok(Bundle::new(path, HEADER_VERSION, content_start, self.crypto, header))
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn raw_size(&self) -> usize {
        self.raw_size
    }
}
