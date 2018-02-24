use std::collections::VecDeque;
use std::ptr;

use super::*;

// Rabin Chunker
// Paper: "Fingerprinting by Random Polynomials"
// Paper-URL: http://www.xmailserver.org/rabin.pdf
// Paper: "Redundancy Elimination Within Large Collections of Files"
// Paper-URL: https://www.usenix.org/legacy/event/usenix04/tech/general/full_papers/kulkarni/kulkarni_html/paper.html
// Wikipedia: https://en.wikipedia.org/wiki/Rabin_fingerprint


fn wrapping_pow(mut base: u32, mut exp: u32) -> u32 {
    let mut acc: u32 = 1;
    while exp > 0 {
        if exp % 2 == 1 {
            acc = acc.wrapping_mul(base)
        }
        base = base.wrapping_mul(base);
        exp /= 2;
    }
    acc
}

fn create_table(alpha: u32, window_size: usize) -> [u32; 256] {
    let mut table = [0u32; 256];
    let a = wrapping_pow(alpha, window_size as u32);
    for i in 0..table.len() as u32 {
        table[i as usize] = i.wrapping_mul(a);
    }
    table
}


pub struct RabinChunker {
    buffer: [u8; 0x1000],
    buffered: usize,
    seed: u32,
    alpha: u32,
    table: [u32; 256],
    min_size: usize,
    max_size: usize,
    window_size: usize,
    chunk_mask: u32,
}


impl RabinChunker {
    pub fn new(avg_size: usize, seed: u32) -> Self {
        let chunk_mask = (avg_size as u32).next_power_of_two() - 1;
        let window_size = avg_size/4-1;
        let alpha = 1_664_525;//153191;
        RabinChunker {
            buffer: [0; 0x1000],
            buffered: 0,
            table: create_table(alpha, window_size),
            alpha: alpha,
            seed: seed,
            min_size: avg_size/4,
            max_size: avg_size*4,
            window_size: window_size,
            chunk_mask: chunk_mask,
        }
    }
}

impl Chunker for RabinChunker {
    #[allow(unknown_lints,explicit_counter_loop)]
    fn chunk(&mut self, r: &mut Read, w: &mut Write) -> Result<ChunkerStatus, ChunkerError> {
        let mut max;
        let mut hash = 0u32;
        let mut pos = 0;
        let mut window = VecDeque::with_capacity(self.window_size);
        loop {
            // Fill the buffer, there might be some bytes still in there from last chunk
            max = try!(r.read(&mut self.buffer[self.buffered..]).map_err(ChunkerError::Read)) + self.buffered;
            // If nothing to do, finish
            if max == 0 {
                return Ok(ChunkerStatus::Finished)
            }
            for i in 0..max {
                let val = self.buffer[i];
                if pos >= self.max_size {
                    try!(w.write_all(&self.buffer[..i+1]).map_err(ChunkerError::Write));
                    unsafe { ptr::copy(self.buffer[i+1..].as_ptr(), self.buffer.as_mut_ptr(), max-i-1) };
                    self.buffered = max-i-1;
                    return Ok(ChunkerStatus::Continue);
                }
                // Hash update
                hash = hash.wrapping_mul(self.alpha).wrapping_add(u32::from(val));
                if pos >= self.window_size {
                    let take = window.pop_front().unwrap();
                    hash = hash.wrapping_sub(self.table[take as usize]);
                    if pos >= self.min_size && ((hash ^ self.seed) & self.chunk_mask) == 0 {
                        try!(w.write_all(&self.buffer[..i+1]).map_err(ChunkerError::Write));
                        unsafe { ptr::copy(self.buffer[i+1..].as_ptr(), self.buffer.as_mut_ptr(), max-i-1) };
                        self.buffered = max-i-1;
                        return Ok(ChunkerStatus::Continue);
                    }
                }
                pos += 1;
                window.push_back(val);
            }
            try!(w.write_all(&self.buffer[..max]).map_err(ChunkerError::Write));
            self.buffered = 0;
        }
    }
}
