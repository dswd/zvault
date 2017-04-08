use super::*;

use std::ptr;

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
    seed: u64
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
            seed: seed
        }
    }
}

impl IChunker for FastCdcChunker {
    #[inline]
    fn get_type(&self) -> ChunkerType {
        ChunkerType::FastCdc((self.avg_size, self.seed))
    }


    #[allow(unknown_lints,explicit_counter_loop,needless_range_loop)]
    fn chunk<R: Read, W: Write>(&mut self, r: &mut R, mut w: &mut W) -> Result<ChunkerStatus, ChunkerError> {
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
            for i in 0..max {
                if pos >= min_size {
                    // Hash update
                    hash = (hash << 1).wrapping_add(gear[buffer[i] as usize]);
                    // 3 options for break point
                    // 1) mask_short matches and chunk is smaller than average
                    // 2) mask_long matches and chunk is longer or equal to average
                    // 3) chunk reached max_size
                    if pos < avg_size && hash & mask_short == 0
                    || pos >= avg_size && hash & mask_long == 0
                    || pos >= max_size {
                        // Write all bytes from this chunk out to sink and store rest for next chunk
                        try!(w.write_all(&buffer[..i+1]).map_err(ChunkerError::Write));
                        unsafe { ptr::copy(buffer[i+1..].as_ptr(), buffer.as_mut_ptr(), max-i-1) };
                        self.buffered = max-i-1;
                        return Ok(ChunkerStatus::Continue);
                    }
                }
                pos += 1;
            }
            try!(w.write_all(&buffer[..max]).map_err(ChunkerError::Write));
            self.buffered = 0;
        }
    }
}
