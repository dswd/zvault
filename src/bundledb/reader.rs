use ::prelude::*;
use super::*;

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, BufReader};
use std::cmp::max;
use std::fmt::{self, Debug};
use std::sync::{Arc, Mutex};


quick_error!{
    #[derive(Debug)]
    pub enum BundleReaderError {
        Read(err: io::Error, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: io::Error) -> (err, path.to_path_buf())
            description("Failed to read data from file")
            display("Bundle reader error: failed to read data from file {:?}\n\tcaused by: {}", path, err)
        }
        WrongHeader(path: PathBuf) {
            description("Wrong header")
            display("Bundle reader error: wrong header on bundle {:?}", path)
        }
        UnsupportedVersion(path: PathBuf, version: u8) {
            description("Wrong version")
            display("Bundle reader error: unsupported version on bundle {:?}: {}", path, version)
        }
        NoSuchChunk(bundle: BundleId, id: usize) {
            description("Bundle has no such chunk")
            display("Bundle reader error: bundle {:?} has no chunk with id {}", bundle, id)
        }
        Decode(err: msgpack::DecodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::DecodeError) -> (err, path.to_path_buf())
            description("Failed to decode bundle header")
            display("Bundle reader error: failed to decode bundle header of {:?}\n\tcaused by: {}", path, err)
        }
        Decompression(err: CompressionError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: CompressionError) -> (err, path.to_path_buf())
            description("Decompression failed")
            display("Bundle reader error: decompression failed on bundle {:?}\n\tcaused by: {}", path, err)
        }
        Decryption(err: EncryptionError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: EncryptionError) -> (err, path.to_path_buf())
            description("Decryption failed")
            display("Bundle reader error: decryption failed on bundle {:?}\n\tcaused by: {}", path, err)
        }
        Integrity(bundle: BundleId, reason: &'static str) {
            description("Bundle has an integrity error")
            display("Bundle reader error: bundle {:?} has an integrity error: {}", bundle, reason)
        }
    }
}


pub struct BundleReader {
    pub info: BundleInfo,
    pub version: u8,
    pub path: PathBuf,
    crypto: Arc<Mutex<Crypto>>,
    pub content_start: usize,
    pub chunks: Option<ChunkList>,
    pub chunk_positions: Option<Vec<usize>>
}

impl BundleReader {
    pub fn new(path: PathBuf, version: u8, content_start: usize, crypto: Arc<Mutex<Crypto>>, info: BundleInfo) -> Self {
        BundleReader {
            info: info,
            chunks: None,
            version: version,
            path: path,
            crypto: crypto,
            content_start: content_start,
            chunk_positions: None
        }
    }

    #[inline]
    pub fn id(&self) -> BundleId {
        self.info.id.clone()
    }

    fn load_header<P: AsRef<Path>>(path: P) -> Result<(BundleInfo, u8, usize), BundleReaderError> {
        let path = path.as_ref();
        let mut file = BufReader::new(try!(File::open(path).context(path)));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header).context(path));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err(BundleReaderError::WrongHeader(path.to_path_buf()))
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err(BundleReaderError::UnsupportedVersion(path.to_path_buf(), version))
        }
        let header: BundleInfo = try!(msgpack::decode_from_stream(&mut file).context(path));
        debug!("Load bundle {}", header.id);
        let content_start = file.seek(SeekFrom::Current(0)).unwrap() as usize + header.chunk_info_size;
        Ok((header, version, content_start))
    }

    #[inline]
    pub fn load_info<P: AsRef<Path>>(path: P) -> Result<BundleInfo, BundleReaderError> {
        Self::load_header(path).map(|b| b.0)
    }

    #[inline]
    pub fn load(path: PathBuf, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleReaderError> {
        let (header, version, content_start) = try!(Self::load_header(&path));
        Ok(BundleReader::new(path, version, content_start, crypto, header))
    }

    pub fn load_chunklist(&mut self) -> Result<(), BundleReaderError> {
        debug!("Load bundle chunklist {} ({:?})", self.info.id, self.info.mode);
        let mut file = BufReader::new(try!(File::open(&self.path).context(&self.path as &Path)));
        let len = self.info.chunk_info_size;
        let start = self.content_start - len;
        try!(file.seek(SeekFrom::Start(start as u64)).context(&self.path as &Path));
        let mut chunk_data = Vec::with_capacity(len);
        chunk_data.resize(self.info.chunk_info_size, 0);
        try!(file.read_exact(&mut chunk_data).context(&self.path as &Path));
        if let Some(ref encryption) = self.info.encryption {
            chunk_data = try!(self.crypto.lock().unwrap().decrypt(&encryption, &chunk_data).context(&self.path as &Path));
        }
        let chunks = ChunkList::read_from(&chunk_data);
        let mut chunk_positions = Vec::with_capacity(chunks.len());
        let mut pos = 0;
        for &(_, len) in (&chunks).iter() {
            chunk_positions.push(pos);
            pos += len as usize;
        }
        self.chunks = Some(chunks);
        self.chunk_positions = Some(chunk_positions);
        Ok(())
    }

    #[inline]
    fn load_encoded_contents(&self) -> Result<Vec<u8>, BundleReaderError> {
        debug!("Load bundle data {} ({:?})", self.info.id, self.info.mode);
        let mut file = BufReader::new(try!(File::open(&self.path).context(&self.path as &Path)));
        try!(file.seek(SeekFrom::Start(self.content_start as u64)).context(&self.path as &Path));
        let mut data = Vec::with_capacity(max(self.info.encoded_size, self.info.raw_size)+1024);
        try!(file.read_to_end(&mut data).context(&self.path as &Path));
        Ok(data)
    }

    #[inline]
    fn decode_contents(&self, mut data: Vec<u8>) -> Result<Vec<u8>, BundleReaderError> {
        if let Some(ref encryption) = self.info.encryption {
            data = try!(self.crypto.lock().unwrap().decrypt(&encryption, &data).context(&self.path as &Path));
        }
        if let Some(ref compression) = self.info.compression {
            data = try!(compression.decompress(&data).context(&self.path as &Path));
        }
        Ok(data)
    }

    #[inline]
    pub fn load_contents(&self) -> Result<Vec<u8>, BundleReaderError> {
        self.load_encoded_contents().and_then(|data| self.decode_contents(data))
    }

    #[inline]
    pub fn get_chunk_position(&mut self, id: usize) -> Result<(usize, usize), BundleReaderError> {
        if id >= self.info.chunk_count {
            return Err(BundleReaderError::NoSuchChunk(self.id(), id))
        }
        if self.chunks.is_none() || self.chunk_positions.is_none() {
            try!(self.load_chunklist());
        }
        let pos = self.chunk_positions.as_ref().unwrap()[id];
        let len = self.chunks.as_ref().unwrap()[id].1 as usize;
        Ok((pos, len))
    }

    pub fn check(&mut self, full: bool) -> Result<(), BundleReaderError> {
        if self.chunks.is_none() || self.chunk_positions.is_none() {
            try!(self.load_chunklist());
        }
        if self.info.chunk_count != self.chunks.as_ref().unwrap().len() {
            return Err(BundleReaderError::Integrity(self.id(),
                "Chunk list size does not match chunk count"))
        }
        if self.chunks.as_ref().unwrap().iter().map(|c| c.1 as usize).sum::<usize>() != self.info.raw_size {
            return Err(BundleReaderError::Integrity(self.id(),
                "Individual chunk sizes do not add up to total size"))
        }
        if !full {
            let size = try!(fs::metadata(&self.path).context(&self.path as &Path)).len();
            if size as usize != self.info.encoded_size + self.content_start {
                return Err(BundleReaderError::Integrity(self.id(),
                    "File size does not match size in header, truncated file"))
            }
            return Ok(())
        }
        let encoded_contents = try!(self.load_encoded_contents());
        if self.info.encoded_size != encoded_contents.len() {
            return Err(BundleReaderError::Integrity(self.id(),
                "Encoded data size does not match size in header, truncated bundle"))
        }
        let contents = try!(self.decode_contents(encoded_contents));
        if self.info.raw_size != contents.len() {
            return Err(BundleReaderError::Integrity(self.id(),
                "Raw data size does not match size in header, truncated bundle"))
        }
        //TODO: verify checksum
        Ok(())
    }
}

impl Debug for BundleReader {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "Bundle(\n\tid: {}\n\tpath: {:?}\n\tchunks: {}\n\tsize: {}, encoded: {}\n\tcompression: {:?}\n)",
            self.info.id.to_string(), self.path, self.info.chunk_count, self.info.raw_size,
            self.info.encoded_size, self.info.compression)
    }
}
