#[macro_use] extern crate quick_error;

use std::io::{self, Write, Read};

mod fixed;
mod ae;
mod rabin;
mod fastcdc;

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
            description("Failed to read input")
            display("Chunker error: failed to read input\n\tcaused by: {}", err)
        }
        Write(err: io::Error) {
            cause(err)
            description("Failed to write to output")
            display("Chunker error: failed to write to output\n\tcaused by: {}", err)
        }
        Custom(reason: &'static str) {
            from()
            description("Custom error")
            display("Chunker error: {}", reason)
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
