use std::io::{Cursor, Read};
use std::fs::File;
use std::time;

use ::chunker::*;
use ::util::*;

fn speed_chunk<C: IChunker>(chunker: &mut C, data: &[u8]) {
    let mut input = Cursor::new(data);
    let mut chunk = Vec::with_capacity(1_000_000);
    loop {
        chunk.clear();
        let result = chunker.chunk(&mut input, &mut chunk).unwrap();
        if result == ChunkerStatus::Finished {
            return
        }
    }
}

fn chunk<C: IChunker>(chunker: &mut C, data: &[u8]) -> Vec<Vec<u8>> {
    let mut input = Cursor::new(data);
    let mut chunks = Vec::with_capacity(100_000);
    loop {
        let mut chunk = Vec::with_capacity(100_000);
        let result = chunker.chunk(&mut input, &mut chunk).unwrap();
        chunks.push(chunk);
        if result == ChunkerStatus::Finished {
            return chunks;
        }
    }
}

fn analyze_chunks(mut chunks: Vec<Vec<u8>>) -> (usize, f64, f64, f64) {
    let count = chunks.len();
    let total = chunks.iter().map(|c| c.len()).sum::<usize>();
    let avg_size = total as f64 / count as f64;
    let stddev = (chunks.iter().map(|c| (c.len() as f64 - avg_size).powi(2)).sum::<f64>() / (count as f64 - 1.0)).sqrt();
    chunks.sort();
    chunks.dedup();
    let non_dup: usize = chunks.iter().map(|c| c.len()).sum();
    let saved = 1.0 - non_dup as f64 / total as f64;
    (count, avg_size, stddev, saved)
}

fn compare_chunker<C: IChunker>(name: &str, mut chunker: C, data: &[u8]) {
    let start = time::Instant::now();
    speed_chunk(&mut chunker, data);
    let elapsed = start.elapsed();
    let chunks = chunk(&mut chunker, data);
    let duration = elapsed.as_secs() as f64 * 1.0 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
    let speed = data.len() as f64 / duration;
    assert_eq!(chunks.iter().map(|c| c.len()).sum::<usize>(), data.len());
    let (_count, avg_size, stddev, saved) = analyze_chunks(chunks);
    println!("{}: \tavg chunk size {:.1}\t± {:.1} bytes, \t{:.1}% saved,\tspeed {:.1} MB/s",
        name, avg_size, stddev, saved * 100.0, speed / 1_000_000.0);
}

fn compare_hash(name: &str, hash: HashMethod, data: &[u8]) {
    let start = time::Instant::now();
    let _ = hash.hash(data);
    let elapsed = start.elapsed();
    let duration = elapsed.as_secs() as f64 * 1.0 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
    let speed = data.len() as f64 / duration;
    println!("{}: {:.1} MB/s", name, speed / 1_000_000.0);
}

fn compare_compression(name: &str, method: Compression, data: &[u8]) {
    let start = time::Instant::now();
    let compressed = method.compress(data).unwrap();
    let elapsed = start.elapsed();
    let duration = elapsed.as_secs() as f64 * 1.0 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
    let cspeed = data.len() as f64 / duration;
    let ratio = compressed.len() as f64 / data.len() as f64;
    let start = time::Instant::now();
    let uncompressed = method.decompress(&compressed).unwrap();
    if uncompressed != data {
        panic!("{} did not uncompress to the same value", name);
    }
    let elapsed = start.elapsed();
    let duration = elapsed.as_secs() as f64 * 1.0 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
    let dspeed = data.len() as f64 / duration;
    println!("{}:\tratio: {:.1}%,\tcompress: {:.1} MB/s,\tdecompress: {:.1} MB/s",
        name, ratio * 100.0, cspeed / 1_000_000.0, dspeed / 1_000_000.0);
}

#[allow(dead_code)]
pub fn run(path: &str) {
    println!("Algorithm comparison on file {}", path);
    println!();
    print!("Reading input file...");
    let mut file = File::open(path).unwrap();
    let mut data = Vec::new();
    file.read_to_end(&mut data).unwrap();
    println!(" done. {} bytes", data.len());
    println!();
    println!("Chunker algorithms");
    for size in &[4usize, 8, 16, 32, 64] {
        println!("  Chunk size: {} KiB", size);
        compare_chunker("    AE", AeChunker::new(size*1024), &data);
        compare_chunker("    Rabin", RabinChunker::new(size*1024, 0), &data);
        compare_chunker("    FastCdc", FastCdcChunker::new(size*1024, 0), &data);
    }
    println!();
    println!("Hash algorithms");
    compare_hash("  Blake2", HashMethod::Blake2, &data);
    compare_hash("  Murmur3", HashMethod::Murmur3, &data);
    println!();
    println!("Compression algorithms");
    compare_compression("  Snappy", Compression::Snappy(()), &data);
    for level in 1..10 {
        compare_compression(&format!("  ZStd/{}", level), Compression::ZStd(level), &data);
    }
    for level in 1..10 {
        compare_compression(&format!("  Deflate/{}", level), Compression::Deflate(level), &data);
    }
    for level in 1..10 {
        compare_compression(&format!("  Brotli/{}", level), Compression::Brotli(level), &data);
    }
    for level in 1..7 {
        compare_compression(&format!("  Lzma2/{}", level), Compression::Lzma2(level), &data);
    }
}
