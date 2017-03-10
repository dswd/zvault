use super::*;

//use std::f64::consts;
use std::ptr;

// AE Chunker
// Paper: "AE: An Asymmetric Extremum Content Defined Chunking Algorithm for Fast and Bandwidth-Efficient Data Deduplication"


pub struct AeChunker {
    buffer: [u8; 4096],
    buffered: usize,
    avg_size: usize,
    window_size: usize
}

impl AeChunker {
    pub fn new(avg_size: usize) -> AeChunker {
        // Experiments show that this claim from the paper is wrong and results in smaller chunks
        //let window_size = (avg_size as f64 / (consts::E - 1.0)) as usize;
        let window_size = avg_size - 256;
        AeChunker{
            buffer: [0; 4096],
            buffered: 0,
            window_size: window_size,
            avg_size: avg_size
        }
    }
}

impl IChunker for AeChunker {
    #[inline]
    fn get_type(&self) -> ChunkerType {
        ChunkerType::Ae(self.avg_size)
    }

    #[allow(unknown_lints,explicit_counter_loop)]
    fn chunk<R: Read, W: Write>(&mut self, r: &mut R, mut w: &mut W) -> Result<ChunkerStatus, ChunkerError> {
        let mut max;
        let mut pos = 0;
        let mut max_pos = 0;
        let mut max_val = 0;
        loop {
            // Fill the buffer, there might be some bytes still in there from last chunk
            max = try!(r.read(&mut self.buffer[self.buffered..]).map_err(ChunkerError::Read)) + self.buffered;
            // If nothing to do, finish
            if max == 0 {
                return Ok(ChunkerStatus::Finished)
            }
            for i in 0..max {
                let val = self.buffer[i];
                if val <= max_val {
                    if pos == max_pos + self.window_size {
                        // Write all bytes from this chunk out to sink and store rest for next chunk
                        try!(w.write_all(&self.buffer[..i+1]).map_err(ChunkerError::Write));
                        unsafe { ptr::copy(self.buffer[i+1..].as_ptr(), self.buffer.as_mut_ptr(), max-i-1) };
                        self.buffered = max-i-1;
                        return Ok(ChunkerStatus::Continue);
                    }
                } else {
                    max_val = val;
                    max_pos = pos;
                }
                pos += 1;
            }
            try!(w.write_all(&self.buffer[..max]).map_err(ChunkerError::Write));
            self.buffered = 0;
        }
    }
}
