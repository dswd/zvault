mod writer;
mod reader;
mod db;
mod cache;
mod uploader;

pub use self::cache::{StoredBundle, BundleCacheError};
pub use self::writer::{BundleWriter, BundleWriterError};
pub use self::reader::{BundleReader, BundleReaderError};
pub use self::db::*;
pub use self::uploader::BundleUploader;

use ::prelude::*;

use std::fmt;
use serde;
use rand;


pub static HEADER_STRING: [u8; 7] = *b"zvault\x01";
pub static HEADER_VERSION: u8 = 1;


#[derive(Hash, PartialEq, Eq, Clone, Default, Ord, PartialOrd)]
pub struct BundleId(pub Hash);

impl Serialize for BundleId {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(ser)
    }
}

impl<'a> Deserialize<'a> for BundleId {
    #[inline]
    fn deserialize<D: serde::Deserializer<'a>>(de: D) -> Result<Self, D::Error> {
        let hash = try!(Hash::deserialize(de));
        Ok(BundleId(hash))
    }
}

impl BundleId {
    #[inline]
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }

    #[inline]
    pub fn random() -> Self {
        BundleId(Hash{high: rand::random(), low: rand::random()})
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
    Data, Meta
}
serde_impl!(BundleMode(u8) {
    Data => 0,
    Meta => 1
});


#[derive(Default, Debug, Clone)]
pub struct BundleHeader {
    pub encryption: Option<Encryption>,
    pub info_size: usize
}
serde_impl!(BundleHeader(u8) {
    encryption: Option<Encryption> => 0,
    info_size: usize => 1
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
    pub chunk_list_size: usize,
    pub timestamp: i64
}
serde_impl!(BundleInfo(u64?) {
    id: BundleId => 0,
    mode: BundleMode => 1,
    compression: Option<Compression> => 2,
    encryption: Option<Encryption> => 3,
    hash_method: HashMethod => 4,
    raw_size: usize => 6,
    encoded_size: usize => 7,
    chunk_count: usize => 8,
    chunk_list_size: usize => 9,
    timestamp: i64 => 10
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
            mode: BundleMode::Data,
            chunk_list_size: 0,
            timestamp: 0
        }
    }
}
