pub use chunking::*;

use std::str::FromStr;


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ChunkerType {
    Ae(usize),
    Rabin((usize, u32)),
    FastCdc((usize, u64)),
    Fixed(usize)
}
serde_impl!(ChunkerType(u64) {
    Ae(usize) => 1,
    Rabin((usize, u32)) => 2,
    FastCdc((usize, u64)) => 3,
    Fixed(usize) => 4
});


impl ChunkerType {
    pub fn from(name: &str, avg_size: usize, seed: u64) -> Result<Self, &'static str> {
        match name {
            "ae" => Ok(ChunkerType::Ae(avg_size)),
            "rabin" => Ok(ChunkerType::Rabin((avg_size, seed as u32))),
            "fastcdc" => Ok(ChunkerType::FastCdc((avg_size, seed))),
            "fixed" => Ok(ChunkerType::Fixed(avg_size)),
            _ => Err("Unsupported chunker type"),
        }
    }

    pub fn from_string(name: &str) -> Result<Self, &'static str> {
        let (name, size) = if let Some(pos) = name.find('/') {
            let size = try!(usize::from_str(&name[pos + 1..]).map_err(
                |_| "Chunk size must be a number"
            ));
            let name = &name[..pos];
            (name, size)
        } else {
            (name, 8)
        };
        Self::from(name, size * 1024, 0)
    }


    #[inline]
    pub fn create(&self) -> Box<Chunker> {
        match *self {
            ChunkerType::Ae(size) => Box::new(AeChunker::new(size)),
            ChunkerType::Rabin((size, seed)) => Box::new(RabinChunker::new(size, seed)),
            ChunkerType::FastCdc((size, seed)) => Box::new(FastCdcChunker::new(size, seed)),
            ChunkerType::Fixed(size) => Box::new(FixedChunker::new(size)),
        }
    }

    pub fn name(&self) -> &'static str {
        match *self {
            ChunkerType::Ae(_size) => "ae",
            ChunkerType::Rabin((_size, _seed)) => "rabin",
            ChunkerType::FastCdc((_size, _seed)) => "fastcdc",
            ChunkerType::Fixed(_size) => "fixed",
        }
    }

    pub fn avg_size(&self) -> usize {
        match *self {
            ChunkerType::Ae(size) |
            ChunkerType::Fixed(size) => size,
            ChunkerType::Rabin((size, _seed)) => size,
            ChunkerType::FastCdc((size, _seed)) => size,
        }
    }

    pub fn to_string(&self) -> String {
        format!("{}/{}", self.name(), self.avg_size() / 1024)
    }

    pub fn seed(&self) -> u64 {
        match *self {
            ChunkerType::Ae(_size) |
            ChunkerType::Fixed(_size) => 0,
            ChunkerType::Rabin((_size, seed)) => seed as u64,
            ChunkerType::FastCdc((_size, seed)) => seed,
        }
    }
}
