use std::collections::HashMap;
use std::path::Path;
use std::io::{self, BufReader, Read, Write, BufWriter};
use std::fs::File;

use ::bundle::{Bundle, BundleId, BundleInfo};
use ::util::*;


static HEADER_STRING: [u8; 7] = *b"zbunmap";
static HEADER_VERSION: u8 = 1;


quick_error!{
    #[derive(Debug)]
    pub enum BundleMapError {
        Io(err: io::Error) {
            from()
            cause(err)
            description("Failed to read/write bundle map")
        }
        Decode(err: msgpack::DecodeError) {
            from()
            cause(err)
            description("Failed to decode bundle map")
        }
        Encode(err: msgpack::EncodeError) {
            from()
            cause(err)
            description("Failed to encode bundle map")
        }
        WrongHeader {
            description("Wrong header")
        }
        WrongVersion(version: u8) {
            description("Wrong version")
            display("Wrong version: {}", version)
        }
    }
}


#[derive(Default)]
pub struct BundleData {
    pub info: BundleInfo
}
serde_impl!(BundleData(u64) {
    info: BundleInfo => 0
});

impl BundleData {
    #[inline]
    pub fn id(&self) -> BundleId {
        self.info.id.clone()
    }
}


pub struct BundleMap(HashMap<u32, BundleData>);

impl BundleMap {
    pub fn create() -> Self {
        BundleMap(Default::default())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, BundleMapError> {
        let mut file = BufReader::new(try!(File::open(path.as_ref())));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err(BundleMapError::WrongHeader)
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err(BundleMapError::WrongVersion(version))
        }
        Ok(BundleMap(try!(msgpack::decode_from_stream(&mut file))))
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), BundleMapError> {
        let mut file = BufWriter::new(try!(File::create(path)));
        try!(file.write_all(&HEADER_STRING));
        try!(file.write_all(&[HEADER_VERSION]));
        msgpack::encode_to_stream(&self.0, &mut file).map_err(BundleMapError::Encode)
    }

    #[inline]
    pub fn get(&self, id: u32) -> Option<&BundleData> {
        self.0.get(&id)
    }

    #[inline]
    pub fn remove(&mut self, id: u32) -> Option<BundleData> {
        self.0.remove(&id)
    }

    #[inline]
    pub fn set(&mut self, id: u32, bundle: &Bundle) {
        let data = BundleData { info: bundle.info.clone() };
        self.0.insert(id, data);
    }

    #[inline]
    pub fn bundles(&self) -> Vec<(u32, &BundleData)> {
        self.0.iter().map(|(id, bundle)| (*id, bundle)).collect()
    }
}
