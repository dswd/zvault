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

fn test_chunking(chunker: &mut Chunker, data: &[u8], chunk_lens: Option<&[usize]>) -> usize {
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
        assert!(pos+chunk.len() <= data.len());
        assert_eq!(&data[pos..pos+chunk.len()], chunk as &[u8]);
        pos += chunk.len();
    }
    if let Some(chunk_lens) = chunk_lens {
        assert_eq!(chunk_lens.len(), chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.len(), chunk_lens[i]);
        }
    }
    assert_eq!(pos, data.len());
    chunks.len()
}


#[test]
fn test_fixed() {
    test_chunking(&mut FixedChunker::new(8192), &random_data(0, 128*1024),
        Some(&[8192, 8192, 8192, 8192, 8192, 8192, 8192, 8192, 8192, 8192,
        8192, 8192, 8192, 8192, 8192, 8192, 0]));
    let data = random_data(0, 10*1024*1024);
    for n in &[1usize,2,4,8,16,32,64,128,256,512,1024] {
        let mut chunker = FixedChunker::new(1024*n);
        let len = test_chunking(&mut chunker, &data, None);
        assert!(len >= data.len()/n/1024/4);
        assert!(len <= data.len()/n/1024*4);
    }
}

#[test]
fn test_ae() {
    test_chunking(&mut AeChunker::new(8192), &random_data(0, 128*1024),
        Some(&[7979, 8046, 7979, 8192, 8192, 8192, 7965, 8158, 8404, 8241,
        8011, 8302, 8120, 8335, 8192, 8192, 572]));
    let data = random_data(0, 10*1024*1024);
    for n in &[1usize,2,4,8,16,32,64,128,256,512,1024] {
        let mut chunker = AeChunker::new(1024*n);
        let len = test_chunking(&mut chunker, &data, None);
        assert!(len >= data.len()/n/1024/4);
        assert!(len <= data.len()/n/1024*4);
    }
}

#[test]
fn test_rabin() {
    test_chunking(&mut RabinChunker::new(8192, 0), &random_data(0, 128*1024),
        Some(&[8604, 4190, 32769, 3680, 26732, 3152, 9947, 6487, 25439, 3944,
        6128]));
    let data = random_data(0, 10*1024*1024);
    for n in &[1usize,2,4,8,16,32,64,128,256,512,1024] {
        let mut chunker = RabinChunker::new(1024*n, 0);
        let len = test_chunking(&mut chunker, &data, None);
        assert!(len >= data.len()/n/1024/4);
        assert!(len <= data.len()/n/1024*4);
    }
}

#[test]
fn test_fastcdc() {
    test_chunking(&mut FastCdcChunker::new(8192, 0), &random_data(0, 128*1024),
        Some(&[8712, 8018, 2847, 9157, 8997, 8581, 8867, 5422, 5412, 9478,
        11553, 9206, 4606, 8529, 3821, 11342, 6524]));
    let data = random_data(0, 10*1024*1024);
    for n in &[1usize,2,4,8,16,32,64,128,256,512,1024] {
        let mut chunker = FastCdcChunker::new(1024*n, 0);
        let len = test_chunking(&mut chunker, &data, None);
        assert!(len >= data.len()/n/1024/4);
        assert!(len <= data.len()/n/1024*4);
    }
}
