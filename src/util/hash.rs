use serde::{self, Serialize, Deserialize};
use serde::de::Error;
use serde::bytes::{ByteBuf, Bytes};

use murmurhash3::murmurhash3_x64_128;
use blake2::blake2b::blake2b;
use byteorder::{LittleEndian, ByteOrder, WriteBytesExt, ReadBytesExt};

use std::mem;
use std::fmt;
use std::u64;
use std::io::{self, Read, Write};



#[repr(packed)]
#[derive(Clone, Copy, PartialEq, Hash, Eq, Default)]
pub struct Hash {
    pub high: u64,
    pub low: u64
}

impl Hash {
    #[inline]
    pub fn hash(&self) -> u64 {
        self.low
    }

    #[inline]
    pub fn empty() -> Self {
        Hash{high: 0, low: 0}
    }

    #[inline]
    pub fn to_string(&self) -> String {
        format!("{:016x}{:016x}", self.high, self.low)
    }

    #[inline]
    pub fn write_to(&self, dst: &mut Write) -> Result<(), io::Error> {
        try!(dst.write_u64::<LittleEndian>(self.high));
        dst.write_u64::<LittleEndian>(self.low)
    }

    #[inline]
    pub fn read_from(src: &mut Read) -> Result<Self, io::Error> {
        let high = try!(src.read_u64::<LittleEndian>());
        let low = try!(src.read_u64::<LittleEndian>());
        Ok(Hash { high: high, low: low })
    }
}

impl fmt::Display for Hash {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{:016x}{:016x}", self.high, self.low)
    }
}

impl fmt::Debug for Hash {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{:016x}{:016x}", self.high, self.low)
    }
}


impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
        let mut dat = [0u8; 16];
        LittleEndian::write_u64(&mut dat[..8], self.high);
        LittleEndian::write_u64(&mut dat[8..], self.low);
        Bytes::from(&dat as &[u8]).serialize(serializer)
    }
}

impl Deserialize for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::Deserializer {
        let dat: Vec<u8> = try!(ByteBuf::deserialize(deserializer)).into();
        if dat.len() != 16 {
            return Err(D::Error::custom("Invalid key length"));
        }
        Ok(Hash{
            high: LittleEndian::read_u64(&dat[..8]),
            low: LittleEndian::read_u64(&dat[8..])
        })
    }
}


#[derive(Debug, Clone, Copy)]
pub enum HashMethod {
    Blake2,
    Murmur3
}
serde_impl!(HashMethod(u64) {
    Blake2 => 1,
    Murmur3 => 2
});


impl HashMethod {
    #[inline]
    pub fn hash(&self, data: &[u8]) -> Hash {
        match *self {
            HashMethod::Blake2 => {
                let hash = blake2b(16, &[], data);
                let hash = unsafe { &*mem::transmute::<_, *mut (u64, u64)>(hash.as_bytes().as_ptr()) };
                Hash { high: u64::from_be(hash.0), low: u64::from_be(hash.1) }
            },
            HashMethod::Murmur3 => {
                let (a, b) = murmurhash3_x64_128(data, 0);
                Hash { high: a, low: b }
            }
        }
    }

    #[inline]
    pub fn from(name: &str) -> Result<Self, &'static str> {
        match name {
            "blake2" => Ok(HashMethod::Blake2),
            "murmur3" => Ok(HashMethod::Murmur3),
            _ => Err("Unsupported hash method")
        }
    }

    #[inline]
    pub fn name(&self) -> &'static str {
        match *self {
            HashMethod::Blake2 => "blake2",
            HashMethod::Murmur3 => "murmur3"
        }
    }

}
