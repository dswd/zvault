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


impl FastCdcChunker {
    fn write_output(&mut self, w: &mut Write, pos: usize, max: usize) -> Result<ChunkerStatus, ChunkerError> {
        debug_assert!(max <= self.buffer.len());
        debug_assert!(pos <= self.buffer.len());
        try!(w.write_all(&self.buffer[..pos]).map_err(ChunkerError::Write));
        unsafe { ptr::copy(self.buffer[pos..].as_ptr(), self.buffer.as_mut_ptr(), max-pos) };
        self.buffered = max-pos;
        Ok(ChunkerStatus::Continue)
    }
}


impl Chunker for FastCdcChunker {
    #[allow(unknown_lints,explicit_counter_loop,needless_range_loop)]
    fn chunk(&mut self, r: &mut Read, w: &mut Write) -> Result<ChunkerStatus, ChunkerError> {
        let mut max;
        let mut hash = 0u64;
        let mut pos = 0;
        loop {
            // Fill the buffer, there might be some bytes still in there from last chunk
            max = try!(r.read(&mut self.buffer[self.buffered..]).map_err(ChunkerError::Read)) + self.buffered;
            // If nothing to do, finish
            if max == 0 {
                return Ok(ChunkerStatus::Finished)
            }
            let min_size_p = cmp::min(max, cmp::max(self.min_size as isize - pos as isize, 0) as usize);
            let avg_size_p = cmp::min(max, cmp::max(self.avg_size as isize - pos as isize, 0) as usize);
            let max_size_p = cmp::min(max, cmp::max(self.max_size as isize - pos as isize, 0) as usize);
            // Skipping first min_size bytes. This is ok as same data still results in same hash.
            if self.avg_size > pos {
                for i in min_size_p..avg_size_p {
                    hash = (hash << 1).wrapping_add(self.gear[self.buffer[i] as usize]);
                    if hash & self.mask_short == 0 {
                        return self.write_output(w, i + 1, max);
                    }
                }
            }
            if self.max_size > pos {
                for i in avg_size_p..max_size_p {
                    hash = (hash << 1).wrapping_add(self.gear[self.buffer[i] as usize]);
                    if hash & self.mask_long == 0 {
                        return self.write_output(w, i+1, max);
                    }
                }
            }
            if max + pos >= self.max_size {
                return self.write_output(w, max_size_p, max);
            }
            pos += max;
            try!(w.write_all(&self.buffer[..max]).map_err(ChunkerError::Write));
            self.buffered = 0;
        }
    }
}
