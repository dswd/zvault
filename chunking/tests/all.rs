extern crate chunking;

use chunking::*;

use std::io::Cursor;


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

fn test_chunking(chunker: &mut Chunker, data: &[u8]) -> usize {
    let mut cursor = Cursor::new(&data);
    let mut chunks = vec![];
    let mut chunk = vec![];
    while chunker.chunk(&mut cursor, &mut chunk).unwrap() == ChunkerStatus::Continue {
        chunks.push(chunk);
        chunk = vec![];
    }
    chunks.push(chunk);
    let mut pos = 0;
    for chunk in &chunks {
        assert_eq!(&data[pos..pos+chunk.len()], chunk as &[u8]);
        pos += chunk.len();
    }
    chunks.len()
}


#[test]
fn test_fixed() {
    let data = random_data(0, 10*1024*1024);
    for n in &[1usize,2,4,8,16,32,64,128,256,512,1024] {
        let mut chunker = FixedChunker::new(1024*n);
        let len = test_chunking(&mut chunker, &data);
        assert!(len >= data.len()/n/1024/4);
        assert!(len <= data.len()/n/1024*4);
    }
}

#[test]
fn test_ae() {
    let data = random_data(0, 10*1024*1024);
    for n in &[1usize,2,4,8,16,32,64,128,256,512,1024] {
        let mut chunker = AeChunker::new(1024*n);
        let len = test_chunking(&mut chunker, &data);
        assert!(len >= data.len()/n/1024/4);
        assert!(len <= data.len()/n/1024*4);
    }
}

#[test]
fn test_rabin() {
    let data = random_data(0, 10*1024*1024);
    for n in &[1usize,2,4,8,16,32,64,128,256,512,1024] {
        let mut chunker = RabinChunker::new(1024*n, 0);
        let len = test_chunking(&mut chunker, &data);
        assert!(len >= data.len()/n/1024/4);
        assert!(len <= data.len()/n/1024*4);
    }
}

#[test]
fn test_fastcdc() {
    let data = random_data(0, 10*1024*1024);
    for n in &[1usize,2,4,8,16,32,64,128,256,512,1024] {
        let mut chunker = FastCdcChunker::new(1024*n, 0);
        let len = test_chunking(&mut chunker, &data);
        assert!(len >= data.len()/n/1024/4);
        assert!(len <= data.len()/n/1024*4);
    }
}
