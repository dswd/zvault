use serde::{self, Serialize, Deserialize};
use serde::de::Error;
use serde_bytes::{ByteBuf, Bytes};

use murmurhash3::murmurhash3_x64_128;
use blake2::blake2b::blake2b;
use byteorder::{LittleEndian, ByteOrder, WriteBytesExt, ReadBytesExt};

use std::mem;
use std::fmt;
use std::u64;
use std::io::{self, Read, Write};


#[repr(packed)]
#[derive(Clone, Copy, PartialEq, Hash, Eq, Default, Ord, PartialOrd)]
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
        Hash { high: 0, low: 0 }
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
        Ok(Hash {
            high: high,
            low: low
        })
    }

    #[inline]
    pub fn from_string(val: &str) -> Result<Self, ()> {
        let high = try!(u64::from_str_radix(&val[..16], 16).map_err(|_| ()));
        let low = try!(u64::from_str_radix(&val[16..], 16).map_err(|_| ()));
        Ok(Self {
            high: high,
            low: low
        })
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
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut dat = [0u8; 16];
        LittleEndian::write_u64(&mut dat[..8], self.high);
        LittleEndian::write_u64(&mut dat[8..], self.low);
        Bytes::from(&dat as &[u8]).serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let dat: Vec<u8> = try!(ByteBuf::deserialize(deserializer)).into();
        if dat.len() != 16 {
            return Err(D::Error::custom("Invalid key length"));
        }
        Ok(Hash {
            high: LittleEndian::read_u64(&dat[..8]),
            low: LittleEndian::read_u64(&dat[8..])
        })
    }
}


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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
                let hash =
                    unsafe { &*mem::transmute::<_, *const (u64, u64)>(hash.as_bytes().as_ptr()) };
                Hash {
                    high: u64::from_be(hash.0),
                    low: u64::from_be(hash.1)
                }
            }
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
            _ => Err("Unsupported hash method"),
        }
    }

    #[inline]
    pub fn name(&self) -> &'static str {
        match *self {
            HashMethod::Blake2 => "blake2",
            HashMethod::Murmur3 => "murmur3",
        }
    }
}



mod tests {

    #[allow(unused_imports)]
    use super::*;


    #[test]
    fn test_parse() {
        assert_eq!(HashMethod::from("blake2"), Ok(HashMethod::Blake2));
        assert_eq!(HashMethod::from("murmur3"), Ok(HashMethod::Murmur3));
        assert!(HashMethod::from("foo").is_err());
    }

    #[test]
    fn test_to_str() {
        assert_eq!(HashMethod::Blake2.name(), "blake2");
        assert_eq!(HashMethod::Murmur3.name(), "murmur3");
    }

    #[test]
    fn test_blake2() {
        assert_eq!(
            HashMethod::Blake2.hash(b"abc"),
            Hash {
                high: 0xcf4ab791c62b8d2b,
                low: 0x2109c90275287816
            }
        );
    }

    #[test]
    fn test_murmur3() {
        assert_eq!(
            HashMethod::Murmur3.hash(b"123"),
            Hash {
                high: 10978418110857903978,
                low: 4791445053355511657
            }
        );
    }

}



#[cfg(feature = "bench")]
mod benches {

    #[allow(unused_imports)]
    use super::*;

    use test::Bencher;


    #[allow(dead_code, needless_range_loop)]
    fn test_data(n: usize) -> Vec<u8> {
        let mut input = vec![0; n];
        for i in 0..input.len() {
            input[i] = (i * i * i) as u8;
        }
        input
    }

    #[bench]
    fn bench_blake2(b: &mut Bencher) {
        let data = test_data(16 * 1024);
        b.bytes = data.len() as u64;
        b.iter(|| HashMethod::Blake2.hash(&data));
    }

    #[bench]
    fn bench_murmur3(b: &mut Bencher) {
        let data = test_data(16 * 1024);
        b.bytes = data.len() as u64;
        b.iter(|| HashMethod::Murmur3.hash(&data));
    }

}
