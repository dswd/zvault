use ::prelude::*;
use super::*;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{self, Write, BufWriter};
use std::sync::{Arc, Mutex};


quick_error!{
    #[derive(Debug)]
    pub enum BundleWriterError {
        CompressionSetup(err: CompressionError) {
            cause(err)
            description("Failed to setup compression")
            display("Bundle writer error: failed to setup compression\n\tcaused by: {}", err)
        }
        Compression(err: CompressionError) {
            cause(err)
            description("Failed to compress data")
            display("Bundle writer error: failed to compress data\n\tcaused by: {}", err)
        }
        Encryption(err: EncryptionError) {
            from()
            cause(err)
            description("Encryption failed")
            display("Bundle writer error: failed to encrypt data\n\tcaused by: {}", err)
        }
        Encode(err: msgpack::EncodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::EncodeError) -> (err, path.to_path_buf())
            description("Failed to encode bundle header to file")
            display("Bundle writer error: failed to encode bundle header to file {:?}\n\tcaused by: {}", path, err)
        }
        Write(err: io::Error, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: io::Error) -> (err, path.to_path_buf())
            description("Failed to write data to file")
            display("Bundle writer error: failed to write data to file {:?}\n\tcaused by: {}", path, err)
        }
    }
}


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
    pub fn new(mode: BundleMode, hash_method: HashMethod, compression: Option<Compression>, encryption: Option<Encryption>, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleWriterError> {
        let compression_stream = match compression {
            Some(ref compression) => Some(try!(compression.compress_stream().map_err(BundleWriterError::CompressionSetup))),
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

    pub fn add(&mut self, chunk: &[u8], hash: Hash) -> Result<usize, BundleWriterError> {
        if let Some(ref mut stream) = self.compression_stream {
            try!(stream.process(chunk, &mut self.data).map_err(BundleWriterError::Compression))
        } else {
            self.data.extend_from_slice(chunk)
        }
        self.raw_size += chunk.len();
        self.chunk_count += 1;
        self.chunks.push((hash, chunk.len() as u32));
        Ok(self.chunk_count-1)
    }

    pub fn finish(mut self, db: &BundleDb) -> Result<StoredBundle, BundleWriterError> {
        if let Some(stream) = self.compression_stream {
            try!(stream.finish(&mut self.data).map_err(BundleWriterError::Compression))
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
        let path = db.temp_bundle_path(&id);
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
        try!(file.write_all(&self.data).context(&path as &Path));
        Ok(StoredBundle { path: path, info: header })
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
