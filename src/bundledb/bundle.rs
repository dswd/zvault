use ::prelude::*;
use super::*;

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, BufReader};
use std::cmp::max;
use std::fmt::{self, Debug};
use std::sync::{Arc, Mutex};

use serde;


#[derive(Hash, PartialEq, Eq, Clone, Default)]
pub struct BundleId(pub Hash);

impl Serialize for BundleId {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(ser)
    }
}

impl Deserialize for BundleId {
    fn deserialize<D: serde::Deserializer>(de: D) -> Result<Self, D::Error> {
        let hash = try!(Hash::deserialize(de));
        Ok(BundleId(hash))
    }
}

impl BundleId {
    #[inline]
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for BundleId {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}", self.to_string())
    }
}

impl fmt::Debug for BundleId {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}", self.to_string())
    }
}


#[derive(Eq, Debug, PartialEq, Clone, Copy)]
pub enum BundleMode {
    Content, Meta
}
serde_impl!(BundleMode(u8) {
    Content => 0,
    Meta => 1
});


#[derive(Clone)]
pub struct BundleInfo {
    pub id: BundleId,
    pub mode: BundleMode,
    pub compression: Option<Compression>,
    pub encryption: Option<Encryption>,
    pub hash_method: HashMethod,
    pub raw_size: usize,
    pub encoded_size: usize,
    pub chunk_count: usize,
    pub chunk_info_size: usize
}
serde_impl!(BundleInfo(u64) {
    id: BundleId => 0,
    mode: BundleMode => 1,
    compression: Option<Compression> => 2,
    encryption: Option<Encryption> => 3,
    hash_method: HashMethod => 4,
    raw_size: usize => 6,
    encoded_size: usize => 7,
    chunk_count: usize => 8,
    chunk_info_size: usize => 9
});

impl Default for BundleInfo {
    fn default() -> Self {
        BundleInfo {
            id: BundleId(Hash::empty()),
            compression: None,
            encryption: None,
            hash_method: HashMethod::Blake2,
            raw_size: 0,
            encoded_size: 0,
            chunk_count: 0,
            mode: BundleMode::Content,
            chunk_info_size: 0
        }
    }
}


pub struct Bundle {
    pub info: BundleInfo,
    pub version: u8,
    pub path: PathBuf,
    crypto: Arc<Mutex<Crypto>>,
    pub content_start: usize,
    pub chunks: Option<ChunkList>,
    pub chunk_positions: Option<Vec<usize>>
}

impl Bundle {
    pub fn new(path: PathBuf, version: u8, content_start: usize, crypto: Arc<Mutex<Crypto>>, info: BundleInfo) -> Self {
        Bundle {
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

    pub fn load(path: PathBuf, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleError> {
        let mut file = BufReader::new(try!(File::open(&path).context(&path as &Path)));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header).context(&path as &Path));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err(BundleError::WrongHeader(path.clone()))
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err(BundleError::WrongVersion(path.clone(), version))
        }
        let header: BundleInfo = try!(msgpack::decode_from_stream(&mut file).context(&path as &Path));
        debug!("Load bundle {}", header.id);
        let content_start = file.seek(SeekFrom::Current(0)).unwrap() as usize + header.chunk_info_size;
        Ok(Bundle::new(path, version, content_start, crypto, header))
    }

    pub fn load_chunklist(&mut self) -> Result<(), BundleError> {
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
    fn load_encoded_contents(&self) -> Result<Vec<u8>, BundleError> {
        debug!("Load bundle data {} ({:?})", self.info.id, self.info.mode);
        let mut file = BufReader::new(try!(File::open(&self.path).context(&self.path as &Path)));
        try!(file.seek(SeekFrom::Start(self.content_start as u64)).context(&self.path as &Path));
        let mut data = Vec::with_capacity(max(self.info.encoded_size, self.info.raw_size)+1024);
        try!(file.read_to_end(&mut data).context(&self.path as &Path));
        Ok(data)
    }

    #[inline]
    fn decode_contents(&self, mut data: Vec<u8>) -> Result<Vec<u8>, BundleError> {
        if let Some(ref encryption) = self.info.encryption {
            data = try!(self.crypto.lock().unwrap().decrypt(&encryption, &data).context(&self.path as &Path));
        }
        if let Some(ref compression) = self.info.compression {
            data = try!(compression.decompress(&data).context(&self.path as &Path));
        }
        Ok(data)
    }

    #[inline]
    pub fn load_contents(&self) -> Result<Vec<u8>, BundleError> {
        self.load_encoded_contents().and_then(|data| self.decode_contents(data))
    }

    #[inline]
    pub fn get_chunk_position(&mut self, id: usize) -> Result<(usize, usize), BundleError> {
        if id >= self.info.chunk_count {
            return Err(BundleError::NoSuchChunk(self.id(), id))
        }
        if self.chunks.is_none() || self.chunk_positions.is_none() {
            try!(self.load_chunklist());
        }
        let pos = self.chunk_positions.as_ref().unwrap()[id];
        let len = self.chunks.as_ref().unwrap()[id].1 as usize;
        Ok((pos, len))
    }

    pub fn check(&mut self, full: bool) -> Result<(), BundleError> {
        if self.chunks.is_none() || self.chunk_positions.is_none() {
            try!(self.load_chunklist());
        }
        if self.info.chunk_count != self.chunks.as_ref().unwrap().len() {
            return Err(BundleError::Integrity(self.id(),
                "Chunk list size does not match chunk count"))
        }
        if self.chunks.as_ref().unwrap().iter().map(|c| c.1 as usize).sum::<usize>() != self.info.raw_size {
            return Err(BundleError::Integrity(self.id(),
                "Individual chunk sizes do not add up to total size"))
        }
        if !full {
            let size = try!(fs::metadata(&self.path).context(&self.path as &Path)).len();
            if size as usize != self.info.encoded_size + self.content_start {
                return Err(BundleError::Integrity(self.id(),
                    "File size does not match size in header, truncated file"))
            }
            return Ok(())
        }
        let encoded_contents = try!(self.load_encoded_contents());
        if self.info.encoded_size != encoded_contents.len() {
            return Err(BundleError::Integrity(self.id(),
                "Encoded data size does not match size in header, truncated bundle"))
        }
        let contents = try!(self.decode_contents(encoded_contents));
        if self.info.raw_size != contents.len() {
            return Err(BundleError::Integrity(self.id(),
                "Raw data size does not match size in header, truncated bundle"))
        }
        //TODO: verify checksum
        Ok(())
    }
}

impl Debug for Bundle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "Bundle(\n\tid: {}\n\tpath: {:?}\n\tchunks: {}\n\tsize: {}, encoded: {}\n\tcompression: {:?}\n)",
            self.info.id.to_string(), self.path, self.info.chunk_count, self.info.raw_size,
            self.info.encoded_size, self.info.compression)
    }
}
