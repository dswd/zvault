use super::*;

use std::cmp::min;


pub struct FixedChunker {
    buffer: [u8; 0x1000],
    size: usize
}

impl FixedChunker {
    pub fn new(avg_size: usize) -> FixedChunker {
        FixedChunker{
            buffer: [0; 0x1000],
            size: avg_size,
        }
    }
}

impl Chunker for FixedChunker {
    #[allow(unknown_lints,explicit_counter_loop)]
    fn chunk(&mut self, r: &mut Read, w: &mut Write) -> Result<ChunkerStatus, ChunkerError> {
        let mut todo = self.size;
        loop {
            // Fill the buffer, there might be some bytes still in there from last chunk
            let max_read = min(todo, self.buffer.len());
            let read = try!(r.read(&mut self.buffer[..max_read]).map_err(ChunkerError::Read));
            // If nothing to do, finish
            if read == 0 {
                return Ok(ChunkerStatus::Finished)
            }
            // Write all bytes from this chunk out to sink and store rest for next chunk
            try!(w.write_all(&self.buffer[..read]).map_err(ChunkerError::Write));
            todo -= read;
            if todo == 0 {
                return Ok(ChunkerStatus::Continue)
            }
        }
    }
}
