use prelude::*;

use std::collections::HashMap;
use std::path::Path;
use std::io::{self, BufReader, Read, Write, BufWriter};
use std::fs::File;


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


pub struct BundleMap(HashMap<u32, BundleId>);

impl BundleMap {
    pub fn create() -> Self {
        BundleMap(Default::default())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, BundleMapError> {
        let mut file = BufReader::new(try!(File::open(path.as_ref())));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err(BundleMapError::WrongHeader);
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err(BundleMapError::WrongVersion(version));
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
    pub fn get(&self, id: u32) -> Option<BundleId> {
        self.0.get(&id).cloned()
    }

    #[inline]
    pub fn remove(&mut self, id: u32) -> Option<BundleId> {
        self.0.remove(&id)
    }

    pub fn find(&self, bundle: &BundleId) -> Option<u32> {
        for (id, bundle_id) in &self.0 {
            if bundle == bundle_id {
                return Some(*id);
            }
        }
        None
    }

    #[inline]
    pub fn set(&mut self, id: u32, bundle: BundleId) {
        self.0.insert(id, bundle);
    }

    pub fn bundles(&self) -> Vec<(u32, BundleId)> {
        self.0
            .iter()
            .map(|(id, bundle)| (*id, bundle.clone()))
            .collect()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}
