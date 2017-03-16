use std::io::{self, Write, Read};

mod ae;
mod rabin;
mod fastcdc;

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
            from(err)
            cause(err)
            description("Failed to read")
        }
        Write(err: io::Error) {
            from(err)
            cause(err)
            description("Failed to write")
        }
        Custom {
            from(&'static str)
            description("Custom error")
        }
    }
}


#[derive(Debug, Eq, PartialEq)]
pub enum ChunkerStatus {
    Continue,
    Finished
}

pub trait IChunker: Sized {
    fn chunk<R: Read, W: Write>(&mut self, r: &mut R, w: &mut W) -> Result<ChunkerStatus, ChunkerError>;
    fn get_type(&self) -> ChunkerType;
}

pub enum Chunker {
    Ae(Box<AeChunker>),
    Rabin(Box<RabinChunker>),
    FastCdc(Box<FastCdcChunker>)
}


impl IChunker for Chunker {
    #[inline]
    fn get_type(&self) -> ChunkerType {
        match *self {
            Chunker::Ae(ref c) => c.get_type(),
            Chunker::Rabin(ref c) => c.get_type(),
            Chunker::FastCdc(ref c) => c.get_type()
        }
    }

    #[inline]
    fn chunk<R: Read, W: Write>(&mut self, r: &mut R, w: &mut W) -> Result<ChunkerStatus, ChunkerError> {
        match *self {
            Chunker::Ae(ref mut c) => c.chunk(r, w),
            Chunker::Rabin(ref mut c) => c.chunk(r, w),
            Chunker::FastCdc(ref mut c) => c.chunk(r, w)
        }
    }
}


#[derive(Debug)]
pub enum ChunkerType {
    Ae(usize),
    Rabin((usize, u32)),
    FastCdc((usize, u64))
}
serde_impl!(ChunkerType(u64) {
    Ae(usize) => 1,
    Rabin((usize, u32)) => 2,
    FastCdc((usize, u64)) => 3
});


impl ChunkerType {
    #[inline]
    pub fn from(name: &str, avg_size: usize, seed: u64) -> Result<Self, &'static str> {
        match name {
            "ae" => Ok(ChunkerType::Ae(avg_size)),
            "rabin" => Ok(ChunkerType::Rabin((avg_size, seed as u32))),
            "fastcdc" => Ok(ChunkerType::FastCdc((avg_size, seed))),
            _ => Err("Unsupported chunker type")
        }
    }

    #[inline]
    pub fn create(&self) -> Chunker {
        match *self {
            ChunkerType::Ae(size) => Chunker::Ae(Box::new(AeChunker::new(size))),
            ChunkerType::Rabin((size, seed)) => Chunker::Rabin(Box::new(RabinChunker::new(size, seed))),
            ChunkerType::FastCdc((size, seed)) => Chunker::FastCdc(Box::new(FastCdcChunker::new(size, seed)))
        }
    }

    #[inline]
    pub fn name(&self) -> &'static str {
        match *self {
            ChunkerType::Ae(_size) => "ae",
            ChunkerType::Rabin((_size, _seed)) => "rabin",
            ChunkerType::FastCdc((_size, _seed)) => "fastcdc"
        }
    }

    #[inline]
    pub fn avg_size(&self) -> usize {
        match *self {
            ChunkerType::Ae(size) => size,
            ChunkerType::Rabin((size, _seed)) => size,
            ChunkerType::FastCdc((size, _seed)) => size
        }
    }

    #[inline]
    pub fn seed(&self) -> u64 {
        match *self {
            ChunkerType::Ae(_size) => 0,
            ChunkerType::Rabin((_size, seed)) => seed as u64,
            ChunkerType::FastCdc((_size, seed)) => seed
        }
    }
}
