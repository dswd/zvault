use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write, Seek, SeekFrom, BufWriter, BufReader};
use std::cmp::max;
use std::fmt::{self, Debug};
use std::sync::{Arc, Mutex};

use serde::{self, Serialize, Deserialize};
use quick_error::ResultExt;

use util::*;

static HEADER_STRING: [u8; 7] = *b"zvault\x01";
static HEADER_VERSION: u8 = 1;

/*

Bundle format
- Magic header + version
- Encoded header structure (contains size of next structure)
- Encoded chunk list (with chunk hashes and sizes)
- Chunk data

*/



quick_error!{
    #[derive(Debug)]
    pub enum BundleError {
        List(err: io::Error) {
            cause(err)
            description("Failed to list bundles")
            display("Failed to list bundles: {}", err)
        }
        Io(err: io::Error, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: io::Error) -> (err, path.to_path_buf())
            description("Failed to read/write bundle")
            display("Failed to read/write bundle {:?}: {}", path, err)
        }
        Decode(err: msgpack::DecodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::DecodeError) -> (err, path.to_path_buf())
            description("Failed to decode bundle header")
            display("Failed to decode bundle header of {:?}: {}", path, err)
        }
        Encode(err: msgpack::EncodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::EncodeError) -> (err, path.to_path_buf())
            description("Failed to encode bundle header")
            display("Failed to encode bundle header of {:?}: {}", path, err)
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
        Decompression(err: CompressionError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: CompressionError) -> (err, path.to_path_buf())
            description("Decompression failed")
            display("Decompression failed on bundle {:?}: {}", path, err)
        }
        Compression(err: CompressionError) {
            from()
            cause(err)
            description("Compression failed")
            display("Compression failed: {}", err)
        }
        Decryption(err: EncryptionError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: EncryptionError) -> (err, path.to_path_buf())
            description("Decryption failed")
            display("Decryption failed on bundle {:?}: {}", path, err)
        }
        Encryption(err: EncryptionError) {
            from()
            cause(err)
            description("Encryption failed")
            display("Encryption failed: {}", err)
        }
        Remove(err: io::Error, bundle: BundleId) {
            cause(err)
            description("Failed to remove bundle")
            display("Failed to remove bundle {}", bundle)
        }
    }
}


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
    pub chunks: ChunkList,
    pub version: u8,
    pub path: PathBuf,
    crypto: Arc<Mutex<Crypto>>,
    pub content_start: usize,
    pub chunk_positions: Vec<usize>
}

impl Bundle {
    fn new(path: PathBuf, version: u8, content_start: usize, crypto: Arc<Mutex<Crypto>>, info: BundleInfo, chunks: ChunkList) -> Self {
        let mut chunk_positions = Vec::with_capacity(chunks.len());
        let mut pos = 0;
        for &(_, len) in (&chunks).iter() {
            chunk_positions.push(pos);
            pos += len as usize;
        }
        Bundle {
            info: info,
            chunks: chunks,
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
        let mut chunk_data = Vec::with_capacity(header.chunk_info_size);
        chunk_data.resize(header.chunk_info_size, 0);
        try!(file.read_exact(&mut chunk_data).context(&path as &Path));
        if let Some(ref encryption) = header.encryption {
            chunk_data = try!(crypto.lock().unwrap().decrypt(&encryption, &chunk_data).context(&path as &Path));
        }
        let chunks = ChunkList::read_from(&chunk_data);
        let content_start = file.seek(SeekFrom::Current(0)).unwrap() as usize;
        Ok(Bundle::new(path, version, content_start, crypto, header, chunks))
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
    pub fn get_chunk_position(&self, id: usize) -> Result<(usize, usize), BundleError> {
        if id >= self.info.chunk_count {
            return Err(BundleError::NoSuchChunk(self.id(), id))
        }
        Ok((self.chunk_positions[id], self.chunks[id].1 as usize))
    }

    pub fn check(&self, full: bool) -> Result<(), BundleError> {
        //FIXME: adapt to new format
        if self.info.chunk_count != self.chunks.len() {
            return Err(BundleError::Integrity(self.id(),
                "Chunk list size does not match chunk count"))
        }
        if self.chunks.iter().map(|c| c.1 as usize).sum::<usize>() != self.info.raw_size {
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
    fn new(
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

    fn finish(mut self, db: &BundleDb) -> Result<Bundle, BundleError> {
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
        Ok(Bundle::new(path, HEADER_VERSION, content_start, self.crypto, header, self.chunks))
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
    crypto: Arc<Mutex<Crypto>>,
    bundles: HashMap<BundleId, Bundle>,
    bundle_cache: LruCache<BundleId, Vec<u8>>
}


impl BundleDb {
    fn new(path: PathBuf, crypto: Arc<Mutex<Crypto>>) -> Self {
        BundleDb {
            path: path,
            crypto: crypto,
            bundles: HashMap::new(),
            bundle_cache: LruCache::new(5, 10)
        }
    }

    fn bundle_path(&self, bundle: &BundleId) -> (PathBuf, PathBuf) {
        let mut folder = self.path.clone();
        let mut file = bundle.to_string().to_owned() + ".bundle";
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
    pub fn open<P: AsRef<Path>>(path: P, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleError> {
        let path = path.as_ref().to_owned();
        let mut self_ = Self::new(path, crypto);
        try!(self_.load_bundle_list());
        Ok(self_)
    }

    #[inline]
    pub fn create<P: AsRef<Path>>(path: P, crypto: Arc<Mutex<Crypto>>) -> Result<Self, BundleError> {
        let path = path.as_ref().to_owned();
        try!(fs::create_dir_all(&path).context(&path as &Path));
        Ok(Self::new(path, crypto))
    }

    #[inline]
    pub fn create_bundle(
        &self,
        mode: BundleMode,
        hash_method: HashMethod,
        compression: Option<Compression>,
        encryption: Option<Encryption>
    ) -> Result<BundleWriter, BundleError> {
        BundleWriter::new(mode, hash_method, compression, encryption, self.crypto.clone())
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
