mod writer;
mod reader;
mod db;
mod cache;

pub use self::cache::{StoredBundle, BundleCacheError};
pub use self::writer::{BundleWriter, BundleWriterError};
pub use self::reader::{BundleReader, BundleReaderError};
pub use self::db::*;

use ::prelude::*;

use std::fmt;
use serde;
use rand;


pub static HEADER_STRING: [u8; 7] = *b"zvault\x01";
pub static HEADER_VERSION: u8 = 1;


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
serde_impl!(BundleInfo(u64?) {
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
