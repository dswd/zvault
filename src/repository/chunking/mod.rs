use std::io::{self, Write, Read};
use std::str::FromStr;

mod fixed;
mod ae;
mod rabin;
mod fastcdc;
#[cfg(test)] mod test;
#[cfg(feature = "bench")] mod benches;

pub use self::fixed::FixedChunker;
pub use self::ae::AeChunker;
pub use self::rabin::RabinChunker;
pub use self::fastcdc::FastCdcChunker;

// https://moinakg.wordpress.com/2013/06/22/high-performance-content-defined-chunking/

// Paper: "A Comprehensive Study of the Past, Present, and Future of Data Deduplication"
// Paper-URL: http://wxia.hustbackup.cn/IEEE-Survey-final.pdf

// https://borgbackup.readthedocs.io/en/stable/internals.html#chunks
// https://github.com/bup/bup/blob/master/lib/bup/bupsplit.c

quick_error!{
    #[derive(Debug)]
    pub enum ChunkerError {
        Read(err: io::Error) {
            cause(err)
            description(tr!("Failed to read input"))
            display("{}", tr_format!("Chunker error: failed to read input\n\tcaused by: {}", err))
        }
        Write(err: io::Error) {
            cause(err)
            description(tr!("Failed to write to output"))
            display("{}", tr_format!("Chunker error: failed to write to output\n\tcaused by: {}", err))
        }
        Custom(reason: &'static str) {
            from()
            description(tr!("Custom error"))
            display("{}", tr_format!("Chunker error: {}", reason))
        }
    }
}


#[derive(Debug, Eq, PartialEq)]
pub enum ChunkerStatus {
    Continue,
    Finished
}

pub trait Chunker {
    fn chunk(&mut self, r: &mut Read, w: &mut Write) -> Result<ChunkerStatus, ChunkerError>;
}


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
            _ => Err(tr!("Unsupported chunker type")),
        }
    }

    pub fn from_string(name: &str) -> Result<Self, &'static str> {
        let (name, size) = if let Some(pos) = name.find('/') {
            let size = try!(usize::from_str(&name[pos + 1..]).map_err(
                |_| tr!("Chunk size must be a number")
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
            ChunkerType::Rabin((_size, seed)) => u64::from(seed),
            ChunkerType::FastCdc((_size, seed)) => seed,
        }
    }
}
