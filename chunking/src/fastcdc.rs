use super::*;

use std::ptr;
use std::cmp;

// FastCDC
// Paper: "FastCDC: a Fast and Efficient Content-Defined Chunking Approach for Data Deduplication"
// Paper-URL: https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf
// Presentation: https://www.usenix.org/sites/default/files/conference/protected-files/atc16_slides_xia.pdf


// Creating 256 pseudo-random values (based on Knuth's MMIX)
fn create_gear(seed: u64) -> [u64; 256] {
    let mut table = [0u64; 256];
    let a = 6364136223846793005;
    let c = 1442695040888963407;
    let mut v = seed;
    for t in &mut table.iter_mut() {
        v = v.wrapping_mul(a).wrapping_add(c);
        *t = v;
    }
    table
}

fn get_masks(avg_size: usize, nc_level: usize, seed: u64) -> (u64, u64) {
    let bits = (avg_size.next_power_of_two() - 1).count_ones();
    if bits == 13 {
        // From the paper
        return (0x0003590703530000, 0x0000d90003530000);
    }
    let mut mask = 0u64;
    let mut v = seed;
    let a = 6364136223846793005;
    let c = 1442695040888963407;
    while mask.count_ones() < bits - nc_level as u32 {
        v = v.wrapping_mul(a).wrapping_add(c);
        mask = (mask | 1).rotate_left(v as u32 & 0x3f);
    }
    let mask_long = mask;
    while mask.count_ones() < bits + nc_level as u32 {
        v = v.wrapping_mul(a).wrapping_add(c);
        mask = (mask | 1).rotate_left(v as u32 & 0x3f);
    }
    let mask_short = mask;
    (mask_short, mask_long)
}

pub struct FastCdcChunker {
    buffer: [u8; 4096],
    buffered: usize,
    gear: [u64; 256],
    min_size: usize,
    max_size: usize,
    avg_size: usize,
    mask_long: u64,
    mask_short: u64,
}


impl FastCdcChunker {
    pub fn new(avg_size: usize, seed: u64) -> Self {
        let (mask_short, mask_long) = get_masks(avg_size, 2, seed);
        FastCdcChunker {
            buffer: [0; 4096],
            buffered: 0,
            gear: create_gear(seed),
            min_size: avg_size/4,
            max_size: avg_size*8,
            avg_size: avg_size,
            mask_long: mask_long,
            mask_short: mask_short,
        }
    }
}

impl Chunker for FastCdcChunker {
    #[allow(unknown_lints,explicit_counter_loop,needless_range_loop)]
    fn chunk(&mut self, r: &mut Read, mut w: &mut Write) -> Result<ChunkerStatus, ChunkerError> {
        let mut max;
        let mut hash = 0u64;
        let mut pos = 0;
        let gear = &self.gear;
        let buffer = &mut self.buffer;
        let min_size = self.min_size;
        let mask_short = self.mask_short;
        let mask_long = self.mask_long;
        let avg_size = self.avg_size;
        let max_size = self.max_size;
        loop {
            // Fill the buffer, there might be some bytes still in there from last chunk
            max = try!(r.read(&mut buffer[self.buffered..]).map_err(ChunkerError::Read)) + self.buffered;
            // If nothing to do, finish
            if max == 0 {
                return Ok(ChunkerStatus::Finished)
            }
            let min_size_p = cmp::min(max, cmp::max(min_size as isize - pos as isize, 0) as usize);
            let avg_size_p = cmp::min(max, cmp::max(avg_size as isize - pos as isize, 0) as usize);
            let max_size_p = cmp::min(max, cmp::max(max_size as isize - pos as isize, 0) as usize);
            if min_size > pos {
                for i in 0..min_size_p {
                    hash = (hash << 1).wrapping_add(gear[buffer[i] as usize]);
                }
            }
            if avg_size > pos {
                for i in min_size_p..avg_size_p {
                    hash = (hash << 1).wrapping_add(gear[buffer[i] as usize]);
                    if hash & mask_short == 0 {
                        try!(w.write_all(&buffer[..i+1]).map_err(ChunkerError::Write));
                        unsafe { ptr::copy(buffer[i+1..].as_ptr(), buffer.as_mut_ptr(), max-i-1) };
                        self.buffered = max-i-1;
                        return Ok(ChunkerStatus::Continue);
                    }
                }
            }
            if max_size > pos {
                for i in avg_size_p..max_size_p {
                    hash = (hash << 1).wrapping_add(gear[buffer[i] as usize]);
                    if hash & mask_long == 0 {
                        try!(w.write_all(&buffer[..i+1]).map_err(ChunkerError::Write));
                        unsafe { ptr::copy(buffer[i+1..].as_ptr(), buffer.as_mut_ptr(), max-i-1) };
                        self.buffered = max-i-1;
                        return Ok(ChunkerStatus::Continue);
                    }
                }
            }
            if max + pos >= max_size {
                let i = max_size_p;
                try!(w.write_all(&buffer[..i]).map_err(ChunkerError::Write));
                unsafe { ptr::copy(buffer[i..].as_ptr(), buffer.as_mut_ptr(), max-i) };
                self.buffered = max-i;
                return Ok(ChunkerStatus::Continue);
            }
            pos += max;
            try!(w.write_all(&buffer[..max]).map_err(ChunkerError::Write));
            self.buffered = 0;
        }
    }
}
