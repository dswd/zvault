use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write, Seek, SeekFrom, BufWriter, BufReader};
use std::cmp::max;
use std::fmt::{self, Debug, Write as FmtWrite};
use std::sync::{Arc, Mutex};

use serde::{self, Serialize, Deserialize};

use util::*;

static HEADER_STRING: [u8; 7] = *b"zbundle";
static HEADER_VERSION: u8 = 1;


quick_error!{
    #[derive(Debug)]
    pub enum BundleError {
        List(err: io::Error) {
            cause(err)
            description("Failed to list bundles")
        }
        Read(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to read bundle")
        }
        Decode(err: msgpack::DecodeError, path: PathBuf) {
            cause(err)
            description("Failed to decode bundle header")
        }
        Write(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to write bundle")
        }
        Encode(err: msgpack::EncodeError, path: PathBuf) {
            cause(err)
            description("Failed to encode bundle header")
        }
        WrongHeader(path: PathBuf) {
            description("Wrong header")
            display("Wrong header on bundle {:?}", path)
        }
        WrongVersion(path: PathBuf, version: u8) {
            description("Wrong version")
            display("Wrong version on bundle {:?}: {}", path, version)
        }
        Integrity(bundle: BundleId, reason: &'static str) {
            description("Bundle has an integrity error")
            display("Bundle {:?} has an integrity error: {}", bundle, reason)
        }
        NoSuchBundle(bundle: BundleId) {
            description("No such bundle")
            display("No such bundle: {:?}", bundle)
        }
        NoSuchChunk(bundle: BundleId, id: usize) {
            description("Bundle has no such chunk")
            display("Bundle {:?} has no chunk with that id: {}", bundle, id)
        }
        Compression(err: CompressionError) {
            from()
            cause(err)
        }
        Encryption(err: EncryptionError) {
            from()
            cause(err)
        }
        Remove(err: io::Error, bundle: BundleId) {
            cause(err)
            description("Failed to remove bundle")
            display("Failed to remove bundle {}", bundle)
        }
    }
}


#[derive(Hash, PartialEq, Eq, Clone, Default)]
pub struct BundleId(pub Vec<u8>);

impl Serialize for BundleId {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_bytes(&self.0)
    }
}

impl Deserialize for BundleId {
    fn deserialize<D: serde::Deserializer>(de: D) -> Result<Self, D::Error> {
        let bytes = try!(msgpack::Bytes::deserialize(de));
        Ok(BundleId(bytes.into()))
    }
}

impl BundleId {
    #[inline]
    fn to_string(&self) -> String {
        let mut buf = String::with_capacity(self.0.len()*2);
        for b in &self.0 {
            write!(&mut buf, "{:02x}", b).unwrap()
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
pub struct BundleInfo {
    pub id: BundleId,
    pub compression: Option<Compression>,
    pub encryption: Option<Encryption>,
    pub checksum: Checksum,
    pub raw_size: usize,
    pub encoded_size: usize,
    pub chunk_count: usize,
    pub chunk_sizes: Vec<usize>
}
serde_impl!(BundleInfo(u64) {
    id: BundleId => 0,
    compression: Option<Compression> => 1,
    encryption: Option<Encryption> => 2,
    checksum: Checksum => 3,
    raw_size: usize => 4,
    encoded_size: usize => 5,
    chunk_count: usize => 6,
    chunk_sizes: Vec<usize> => 7
});

impl Default for BundleInfo {
    fn default() -> Self {
        BundleInfo {
            id: BundleId(vec![]),
            compression: None,
            encryption: None,
            checksum: (ChecksumType::Blake2_256, msgpack::Bytes::new()),
            raw_size: 0,
            encoded_size: 0,
            chunk_count: 0,
            chunk_sizes: vec![]
        }
    }
}


pub struct Bundle {
    pub info: BundleInfo,
    pub version: u8,
    pub path: PathBuf,
    crypto: Arc<Mutex<Crypto>>,
    pub content_start: usize,
    pub chunk_positions: Vec<usize>
}

impl Bundle {
    fn new(path: PathBuf, version: u8, content_start: usize, crypto: Arc<Mutex<Crypto>>, info: BundleInfo) -> Self {
        let mut chunk_positions = Vec::with_capacity(info.chunk_sizes.len());
        let mut pos = 0;
        for len in &info.chunk_sizes {
            chunk_positions.push(pos);
            pos += *len;
        }
        Bundle {
            info: info,
            version: version,
            path: path,
            crypto: crypto,
            content_start: content_start,
            chunk_positions: chunk_positions
        }
    }

    #[inline]
    pub fn id(&self) -> BundleId {
        self.info.id.clone()
    }

    pub fn load(path: PathBuf, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleError> {
        let mut file = BufReader::new(try!(File::open(&path).map_err(|e| BundleError::Read(e, path.clone()))));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header).map_err(|e| BundleError::Read(e, path.clone())));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err(BundleError::WrongHeader(path.clone()))
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err(BundleError::WrongVersion(path.clone(), version))
        }
        let header = try!(msgpack::decode_from_stream(&mut file)
            .map_err(|e| BundleError::Decode(e, path.clone())));
        let content_start = file.seek(SeekFrom::Current(0)).unwrap() as usize;
        Ok(Bundle::new(path, version, content_start, crypto, header))
    }

    #[inline]
    fn load_encoded_contents(&self) -> Result<Vec<u8>, BundleError> {
        let mut file = BufReader::new(try!(File::open(&self.path).map_err(|e| BundleError::Read(e, self.path.clone()))));
        try!(file.seek(SeekFrom::Start(self.content_start as u64)).map_err(|e| BundleError::Read(e, self.path.clone())));
        let mut data = Vec::with_capacity(max(self.info.encoded_size, self.info.raw_size)+1024);
        try!(file.read_to_end(&mut data).map_err(|e| BundleError::Read(e, self.path.clone())));
        Ok(data)
    }

    #[inline]
    fn decode_contents(&self, mut data: Vec<u8>) -> Result<Vec<u8>, BundleError> {
        if let Some(ref encryption) = self.info.encryption {
            data = try!(self.crypto.lock().unwrap().decrypt(encryption.clone(), &data));
        }
        if let Some(ref compression) = self.info.compression {
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
        if id >= self.info.chunk_count {
            return Err(BundleError::NoSuchChunk(self.id(), id))
        }
        Ok((self.chunk_positions[id], self.info.chunk_sizes[id]))
    }

    pub fn check(&self, full: bool) -> Result<(), BundleError> {
        if self.info.chunk_count != self.info.chunk_sizes.len() {
            return Err(BundleError::Integrity(self.id(),
                "Chunk list size does not match chunk count"))
        }
        if self.info.chunk_sizes.iter().sum::<usize>() != self.info.raw_size {
            return Err(BundleError::Integrity(self.id(),
                "Individual chunk sizes do not add up to total size"))
        }
        if !full {
            let size = try!(fs::metadata(&self.path).map_err(|e| BundleError::Read(e, self.path.clone()))
            ).len();
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
        try!(fs::create_dir_all(&folder).map_err(|e| BundleError::Write(e, path.clone())));
        let mut file = BufWriter::new(try!(File::create(&path).map_err(|e| BundleError::Write(e, path.clone()))));
        try!(file.write_all(&HEADER_STRING).map_err(|e| BundleError::Write(e, path.clone())));
        try!(file.write_all(&[HEADER_VERSION]).map_err(|e| BundleError::Write(e, path.clone())));
        let header = BundleInfo {
            checksum: checksum,
            compression: self.compression,
            encryption: self.encryption,
            chunk_count: self.chunk_count,
            id: id.clone(),
            raw_size: self.raw_size,
            encoded_size: encoded_size,
            chunk_sizes: self.chunk_sizes
        };
        try!(msgpack::encode_to_stream(&header, &mut file)
            .map_err(|e| BundleError::Encode(e, path.clone())));
        let content_start = file.seek(SeekFrom::Current(0)).unwrap() as usize;
        try!(file.write_all(&self.data).map_err(|e| BundleError::Write(e, path.clone())));
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
        let mut file = bundle.to_string()[0..32].to_owned() + ".bundle";
        let mut count = self.bundles.len();
        while count >= 100 {
            if file.len() < 10 {
                break
            }
            folder = folder.join(&file[0..2]);
            file = file[2..].to_string();
            count /= 100;
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
                    self.bundles.insert(bundle.id(), bundle);
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
            .map_err(|e| BundleError::Write(e, path.clone())));
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
        let bundle = try!(self.bundles.get(bundle_id).ok_or(BundleError::NoSuchBundle(bundle_id.clone())));
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
        let id = bundle.id();
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
            fs::remove_file(&bundle.path).map_err(|e| BundleError::Remove(e, bundle.id()))
        } else {
            Err(BundleError::NoSuchBundle(bundle.clone()))
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
