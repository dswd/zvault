use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write, Seek, SeekFrom, BufWriter, BufReader};
use std::cmp::max;
use std::fmt::{self, Debug, Write as FmtWrite};
use std::sync::{Arc, Mutex};

use serde::{self, Serialize, Deserialize};
use serde::bytes::ByteBuf;
use rmp_serde;

use errors::BundleError;
use util::*;

static HEADER_STRING: [u8; 7] = *b"zbundle";
static HEADER_VERSION: u8 = 1;


// TODO: Test cases
// TODO: Benchmarks


#[derive(Hash, PartialEq, Eq, Clone, Default)]
pub struct BundleId(pub Vec<u8>);

impl Serialize for BundleId {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_bytes(&self.0)
    }
}

impl Deserialize for BundleId {
    fn deserialize<D: serde::Deserializer>(de: D) -> Result<Self, D::Error> {
        let bytes = try!(ByteBuf::deserialize(de));
        Ok(BundleId(bytes.into()))
    }
}

impl BundleId {
    #[inline]
    fn to_string(&self) -> String {
        let mut buf = String::with_capacity(self.0.len()*2);
        for b in &self.0 {
            write!(&mut buf, "{:2x}", b).unwrap()
        }
        buf
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



#[derive(Clone)]
pub struct BundleHeader {
    pub id: BundleId,
    pub compression: Option<Compression>,
    pub encryption: Option<Encryption>,
    pub checksum: Checksum,
    pub raw_size: usize,
    pub encoded_size: usize,
    pub chunk_count: usize,
    pub chunk_sizes: Vec<usize>
}
serde_impl!(BundleHeader(u64) {
    id: BundleId => 0,
    compression: Option<Compression> => 1,
    encryption: Option<Encryption> => 2,
    checksum: Checksum => 3,
    raw_size: usize => 4,
    encoded_size: usize => 5,
    chunk_count: usize => 6,
    chunk_sizes: Vec<usize> => 7
});

impl Default for BundleHeader {
    fn default() -> Self {
        BundleHeader {
            id: BundleId(vec![]),
            compression: None,
            encryption: None,
            checksum: (ChecksumType::Sha3_256, ByteBuf::new()),
            raw_size: 0,
            encoded_size: 0,
            chunk_count: 0,
            chunk_sizes: vec![]
        }
    }
}


pub struct Bundle {
    pub id: BundleId,
    pub version: u8,
    pub path: PathBuf,
    crypto: Arc<Mutex<Crypto>>,
    pub compression: Option<Compression>,
    pub encryption: Option<Encryption>,
    pub raw_size: usize,
    pub encoded_size: usize,
    pub checksum: Checksum,
    pub content_start: usize,
    pub chunk_count: usize,
    pub chunk_sizes: Vec<usize>,
    pub chunk_positions: Vec<usize>
}

impl Bundle {
    fn new(path: PathBuf, version: u8, content_start: usize, crypto: Arc<Mutex<Crypto>>, header: BundleHeader) -> Self {
        let mut chunk_positions = Vec::with_capacity(header.chunk_sizes.len());
        let mut pos = 0;
        for len in &header.chunk_sizes {
            chunk_positions.push(pos);
            pos += *len;
        }
        Bundle {
            id: header.id,
            version: version,
            path: path,
            crypto: crypto,
            compression: header.compression,
            encryption: header.encryption,
            raw_size: header.raw_size,
            encoded_size: header.encoded_size,
            chunk_count: header.chunk_count,
            checksum: header.checksum,
            content_start: content_start,
            chunk_sizes: header.chunk_sizes,
            chunk_positions: chunk_positions
        }
    }

    pub fn load(path: PathBuf, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleError> {
        let mut file = BufReader::new(try!(File::open(&path)
            .map_err(|e| BundleError::Read(e, path.clone(), "Failed to open bundle file"))));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header)
            .map_err(|e| BundleError::Read(e, path.clone(), "Failed to read bundle header")));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err(BundleError::Format(path.clone(), "Wrong header string"))
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err(BundleError::Format(path.clone(), "Unsupported bundle file version"))
        }
        let mut reader = rmp_serde::Deserializer::new(file);
        let header = try!(BundleHeader::deserialize(&mut reader)
            .map_err(|e| BundleError::Decode(e, path.clone())));
        file = reader.into_inner();
        let content_start = file.seek(SeekFrom::Current(0)).unwrap() as usize;
        Ok(Bundle::new(path, version, content_start, crypto, header))
    }

    #[inline]
    fn load_encoded_contents(&self) -> Result<Vec<u8>, BundleError> {
        let mut file = BufReader::new(try!(File::open(&self.path)
            .map_err(|e| BundleError::Read(e, self.path.clone(), "Failed to open bundle file"))));
        try!(file.seek(SeekFrom::Start(self.content_start as u64))
            .map_err(|e| BundleError::Read(e, self.path.clone(), "Failed to seek to data")));
        let mut data = Vec::with_capacity(max(self.encoded_size, self.raw_size)+1024);
        try!(file.read_to_end(&mut data).map_err(|_| "Failed to read data"));
        Ok(data)
    }

    #[inline]
    fn decode_contents(&self, mut data: Vec<u8>) -> Result<Vec<u8>, BundleError> {
        if let Some(ref encryption) = self.encryption {
            data = try!(self.crypto.lock().unwrap().decrypt(encryption.clone(), &data));
        }
        if let Some(ref compression) = self.compression {
            data = try!(compression.decompress(&data));
        }
        Ok(data)
    }

    #[inline]
    pub fn load_contents(&self) -> Result<Vec<u8>, BundleError> {
        self.load_encoded_contents().and_then(|data| self.decode_contents(data))
    }

    #[inline]
    pub fn get_chunk_position(&self, id: usize) -> Result<(usize, usize), BundleError> {
        if id >= self.chunk_count {
            return Err("Invalid chunk id".into())
        }
        Ok((self.chunk_positions[id], self.chunk_sizes[id]))
    }

    pub fn check(&self, full: bool) -> Result<(), BundleError> {
        if self.chunk_count != self.chunk_sizes.len() {
            return Err(BundleError::Integrity(self.id.clone(),
                "Chunk list size does not match chunk count"))
        }
        if self.chunk_sizes.iter().sum::<usize>() != self.raw_size {
            return Err(BundleError::Integrity(self.id.clone(),
                "Individual chunk sizes do not add up to total size"))
        }
        if !full {
            let size = try!(fs::metadata(&self.path)
                .map_err(|e| BundleError::Read(e, self.path.clone(), "Failed to get size of file"))
            ).len();
            if size as usize != self.encoded_size + self.content_start {
                return Err(BundleError::Integrity(self.id.clone(),
                    "File size does not match size in header, truncated file"))
            }
            return Ok(())
        }
        let encoded_contents = try!(self.load_encoded_contents());
        if self.encoded_size != encoded_contents.len() {
            return Err(BundleError::Integrity(self.id.clone(),
                "Encoded data size does not match size in header, truncated bundle"))
        }
        let contents = try!(self.decode_contents(encoded_contents));
        if self.raw_size != contents.len() {
            return Err(BundleError::Integrity(self.id.clone(),
                "Raw data size does not match size in header, truncated bundle"))
        }
        //TODO: verify checksum
        Ok(())
    }
}

impl Debug for Bundle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "Bundle(\n\tid: {}\n\tpath: {:?}\n\tchunks: {}\n\tsize: {}, encoded: {}\n\tcompression: {:?}\n)",
        self.id.to_string(), self.path, self.chunk_count, self.raw_size, self.encoded_size, self.compression)
    }
}



pub struct BundleWriter {
    data: Vec<u8>,
    compression: Option<Compression>,
    compression_stream: Option<CompressionStream>,
    encryption: Option<Encryption>,
    crypto: Arc<Mutex<Crypto>>,
    checksum: ChecksumCreator,
    raw_size: usize,
    chunk_count: usize,
    chunk_sizes: Vec<usize>
}

impl BundleWriter {
    fn new(compression: Option<Compression>, encryption: Option<Encryption>, crypto: Arc<Mutex<Crypto>>, checksum: ChecksumType) -> Result<Self, BundleError> {
        let compression_stream = match compression {
            Some(ref compression) => Some(try!(compression.compress_stream())),
            None => None
        };
        Ok(BundleWriter {
            data: vec![],
            compression: compression,
            compression_stream: compression_stream,
            encryption: encryption,
            crypto: crypto,
            checksum: ChecksumCreator::new(checksum),
            raw_size: 0,
            chunk_count: 0,
            chunk_sizes: vec![]
        })
    }

    pub fn add(&mut self, chunk: &[u8]) -> Result<usize, BundleError> {
        if let Some(ref mut stream) = self.compression_stream {
            try!(stream.process(chunk, &mut self.data))
        } else {
            self.data.extend_from_slice(chunk)
        }
        self.checksum.update(chunk);
        self.raw_size += chunk.len();
        self.chunk_count += 1;
        self.chunk_sizes.push(chunk.len());
        Ok(self.chunk_count-1)
    }

    fn finish(mut self, db: &BundleDb) -> Result<Bundle, BundleError> {
        if let Some(stream) = self.compression_stream {
            try!(stream.finish(&mut self.data))
        }
        if let Some(ref encryption) = self.encryption {
            self.data = try!(self.crypto.lock().unwrap().encrypt(encryption.clone(), &self.data));
        }
        let encoded_size = self.data.len();
        let checksum = self.checksum.finish();
        let id = BundleId(checksum.1.to_vec());
        let (folder, file) = db.bundle_path(&id);
        let path = folder.join(file);
        try!(fs::create_dir_all(&folder)
            .map_err(|e| BundleError::Write(e, path.clone(), "Failed to create folder")));
        let mut file = BufWriter::new(try!(File::create(&path)
            .map_err(|e| BundleError::Write(e, path.clone(), "Failed to create bundle file"))));
        try!(file.write_all(&HEADER_STRING)
            .map_err(|e| BundleError::Write(e, path.clone(), "Failed to write bundle header")));
        try!(file.write_all(&[HEADER_VERSION])
            .map_err(|e| BundleError::Write(e, path.clone(), "Failed to write bundle header")));
        let header = BundleHeader {
            checksum: checksum,
            compression: self.compression,
            encryption: self.encryption,
            chunk_count: self.chunk_count,
            id: id.clone(),
            raw_size: self.raw_size,
            encoded_size: encoded_size,
            chunk_sizes: self.chunk_sizes
        };
        {
            let mut writer = rmp_serde::Serializer::new(&mut file);
            try!(header.serialize(&mut writer)
                .map_err(|e| BundleError::Encode(e, path.clone())));
        }
        let content_start = file.seek(SeekFrom::Current(0)).unwrap() as usize;
        try!(file.write_all(&self.data)
            .map_err(|e| BundleError::Write(e, path.clone(), "Failed to write bundle data")));
        Ok(Bundle::new(path, HEADER_VERSION, content_start, self.crypto, header))
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.data.len()
    }
}


pub struct BundleDb {
    path: PathBuf,
    compression: Option<Compression>,
    encryption: Option<Encryption>,
    crypto: Arc<Mutex<Crypto>>,
    checksum: ChecksumType,
    bundles: HashMap<BundleId, Bundle>,
    bundle_cache: LruCache<BundleId, Vec<u8>>
}


impl BundleDb {
    fn new(path: PathBuf, compression: Option<Compression>, encryption: Option<Encryption>, checksum: ChecksumType) -> Self {
        BundleDb {
            path: path,
            compression:
            compression,
            crypto: Arc::new(Mutex::new(Crypto::new())),
            encryption: encryption,
            checksum: checksum,
            bundles: HashMap::new(),
            bundle_cache: LruCache::new(5, 10)
        }
    }

    fn bundle_path(&self, bundle: &BundleId) -> (PathBuf, PathBuf) {
        let mut folder = self.path.clone();
        let mut file = bundle.to_string() + ".bundle";
        let mut count = self.bundles.len();
        while count >= 1000 {
            if file.len() < 10 {
                break
            }
            folder = folder.join(&file[0..3]);
            file = file[3..].to_string();
            count /= 1000;
        }
        (folder, file.into())
    }

    fn load_bundle_list(&mut self) -> Result<(), BundleError> {
        self.bundles.clear();
        let mut paths = Vec::new();
        paths.push(self.path.clone());
        while let Some(path) = paths.pop() {
            for entry in try!(fs::read_dir(path).map_err(BundleError::List)) {
                let entry = try!(entry.map_err(BundleError::List));
                let path = entry.path();
                if path.is_dir() {
                    paths.push(path);
                } else {
                    let bundle = try!(Bundle::load(path, self.crypto.clone()));
                    self.bundles.insert(bundle.id.clone(), bundle);
                }
            }
        }
        Ok(())
    }

    #[inline]
    pub fn open<P: AsRef<Path>>(path: P, compression: Option<Compression>, encryption: Option<Encryption>, checksum: ChecksumType) -> Result<Self, BundleError> {
        let path = path.as_ref().to_owned();
        let mut self_ = Self::new(path, compression, encryption, checksum);
        try!(self_.load_bundle_list());
        Ok(self_)
    }

    #[inline]
    pub fn create<P: AsRef<Path>>(path: P, compression: Option<Compression>, encryption: Option<Encryption>, checksum: ChecksumType) -> Result<Self, BundleError> {
        let path = path.as_ref().to_owned();
        try!(fs::create_dir_all(&path)
            .map_err(|e| BundleError::Write(e, path.clone(), "Failed to create folder")));
        Ok(Self::new(path, compression, encryption, checksum))
    }

    #[inline]
    pub fn open_or_create<P: AsRef<Path>>(path: P, compression: Option<Compression>, encryption: Option<Encryption>, checksum: ChecksumType) -> Result<Self, BundleError> {
        if path.as_ref().exists() {
            Self::open(path, compression, encryption, checksum)
        } else {
            Self::create(path, compression, encryption, checksum)
        }
    }

    #[inline]
    pub fn create_bundle(&self) -> Result<BundleWriter, BundleError> {
        BundleWriter::new(self.compression.clone(), self.encryption.clone(), self.crypto.clone(), self.checksum)
    }

    pub fn get_chunk(&mut self, bundle_id: &BundleId, id: usize) -> Result<Vec<u8>, BundleError> {
        let bundle = try!(self.bundles.get(bundle_id).ok_or("Bundle not found"));
        let (pos, len) = try!(bundle.get_chunk_position(id));
        let mut chunk = Vec::with_capacity(len);
        if let Some(data) = self.bundle_cache.get(bundle_id) {
            chunk.extend_from_slice(&data[pos..pos+len]);
            return Ok(chunk);
        }
        let data = try!(bundle.load_contents());
        chunk.extend_from_slice(&data[pos..pos+len]);
        self.bundle_cache.put(bundle_id.clone(), data);
        Ok(chunk)
    }

    #[inline]
    pub fn add_bundle(&mut self, bundle: BundleWriter) -> Result<&Bundle, BundleError> {
        let bundle = try!(bundle.finish(&self));
        let id = bundle.id.clone();
        self.bundles.insert(id.clone(), bundle);
        Ok(self.get_bundle(&id).unwrap())
    }

    #[inline]
    pub fn get_bundle(&self, bundle: &BundleId) -> Option<&Bundle> {
        self.bundles.get(bundle)
    }

    #[inline]
    pub fn list_bundles(&self) -> Vec<&Bundle> {
        self.bundles.values().collect()
    }

    #[inline]
    pub fn delete_bundle(&mut self, bundle: &BundleId) -> Result<(), BundleError> {
        if let Some(bundle) = self.bundles.remove(bundle) {
            fs::remove_file(&bundle.path).map_err(|e| BundleError::Remove(e, bundle.id.clone()))
        } else {
            Err("No such bundle".into())
        }
    }

    #[inline]
    pub fn check(&self, full: bool) -> Result<(), BundleError> {
        for bundle in self.bundles.values() {
            try!(bundle.check(full))
        }
        Ok(())
    }
}
