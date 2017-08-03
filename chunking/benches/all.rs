#![feature(test)]

extern crate test;
extern crate chunking;

use chunking::*;

use std::io::{self, Write, Cursor};
use test::Bencher;


fn random_data(seed: u64, size: usize) -> Vec<u8> {
    assert_eq!(size % 4, 0);
    let mut data = vec![0; size];
    let a = 6364136223846793005;
    let c = 1442695040888963407;
    let mut v = seed;
    for i in 0..size/4 {
        v = v.wrapping_mul(a).wrapping_add(c);
        data[4*i] = ((v >> 24) & 0xff) as u8;
        data[4*i+1] = ((v >> 16) & 0xff) as u8;
        data[4*i+2] = ((v >> 8) & 0xff) as u8;
        data[4*i+3] = (v & 0xff) as u8;
    }
    data
}


struct CutPositions(Vec<u64>, u64);

impl CutPositions {
    pub fn new() -> Self {
        CutPositions(vec![], 0)
    }

    pub fn positions(&self) -> &[u64] {
        &self.0
    }
}

impl Write for CutPositions {
    fn write(&mut self, data: &[u8]) -> Result<usize, io::Error> {
        self.1 += data.len() as u64;
        self.0.push(self.1);
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}


#[bench]
fn test_fixed_init(b: &mut Bencher) {
    b.iter(|| {
        FixedChunker::new(8*1024);
    })
}

#[bench]
fn test_fixed_8192(b: &mut Bencher) {
    let data = random_data(0, 1024*1024);
    b.bytes = data.len() as u64;
    b.iter(|| {
        let mut chunker = FixedChunker::new(8*1024);
        let mut cursor = Cursor::new(&data);
        let mut sink = CutPositions::new();
        while chunker.chunk(&mut cursor, &mut sink).unwrap() == ChunkerStatus::Continue {};
        test::black_box(sink.positions().len())
    })
}


#[bench]
fn test_ae_init(b: &mut Bencher) {
    b.iter(|| {
        AeChunker::new(8*1024);
    })
}

#[bench]
fn test_ae_8192(b: &mut Bencher) {
    let data = random_data(0, 1024*1024);
    b.bytes = data.len() as u64;
    b.iter(|| {
        let mut chunker = AeChunker::new(8*1024);
        let mut cursor = Cursor::new(&data);
        let mut sink = CutPositions::new();
        while chunker.chunk(&mut cursor, &mut sink).unwrap() == ChunkerStatus::Continue {};
        test::black_box(sink.positions().len())
    })
}


#[bench]
fn test_rabin_init(b: &mut Bencher) {
    b.iter(|| {
        RabinChunker::new(8*1024, 0);
    })
}

#[bench]
fn test_rabin_8192(b: &mut Bencher) {
    let data = random_data(0, 1024*1024);
    b.bytes = data.len() as u64;
    b.iter(|| {
        let mut chunker = RabinChunker::new(8*1024, 0);
        let mut cursor = Cursor::new(&data);
        let mut sink = CutPositions::new();
        while chunker.chunk(&mut cursor, &mut sink).unwrap() == ChunkerStatus::Continue {};
        test::black_box(sink.positions().len())
    })
}


#[bench]
fn test_fastcdc_init(b: &mut Bencher) {
    b.iter(|| {
        FastCdcChunker::new(8*1024, 0);
    })
}

#[bench]
fn test_fastcdc_8192(b: &mut Bencher) {
    let data = random_data(0, 1024*1024);
    b.bytes = data.len() as u64;
    b.iter(|| {
        let mut chunker = FastCdcChunker::new(8*1024, 0);
        let mut cursor = Cursor::new(&data);
        let mut sink = CutPositions::new();
        while chunker.chunk(&mut cursor, &mut sink).unwrap() == ChunkerStatus::Continue {};
        test::black_box(sink.positions().len())
    })
}
