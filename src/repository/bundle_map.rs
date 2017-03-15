use std::collections::HashMap;
use std::path::Path;
use std::io::{BufReader, Read, Write, BufWriter};
use std::fs::File;

use rmp_serde;
use serde::Deserialize;
use serde::Serialize;

use ::bundle::{Bundle, BundleId, BundleInfo};


static HEADER_STRING: [u8; 7] = *b"zbunmap";
static HEADER_VERSION: u8 = 1;


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

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, &'static str> {
        let mut file = BufReader::new(try!(File::open(path.as_ref())
            .map_err(|_| "Failed to open bundle map file")));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header)
            .map_err(|_| "Failed to read bundle map header"));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err("Wrong header string")
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err("Unsupported bundle map file version")
        }
        let mut reader = rmp_serde::Deserializer::new(file);
        let map = try!(HashMap::deserialize(&mut reader)
            .map_err(|_| "Failed to read bundle map data"));
        Ok(BundleMap(map))
    }


    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), &'static str> {
        let mut file = BufWriter::new(try!(File::create(path)
            .map_err(|_| "Failed to create bundle file")));
        try!(file.write_all(&HEADER_STRING)
            .map_err(|_| "Failed to write bundle header"));
        try!(file.write_all(&[HEADER_VERSION])
            .map_err(|_| "Failed to write bundle header"));
        let mut writer = rmp_serde::Serializer::new(&mut file);
        self.0.serialize(&mut writer)
            .map_err(|_| "Failed to write bundle map data")
    }

    #[inline]
    pub fn get(&self, id: u32) -> Option<&BundleData> {
        self.0.get(&id)
    }

    #[inline]
    pub fn set(&mut self, id: u32, bundle: &Bundle) {
        let data = BundleData { info: bundle.info.clone() };
        self.0.insert(id, data);
    }

    #[inline]
    pub fn bundles(&self) -> Vec<&BundleData> {
        self.0.values().collect()
    }
}
