use ::prelude::*;

use std::io::{self, Cursor, Read, Write};
use std::fs::File;
use std::collections::HashSet;

use chrono::Duration;


struct ChunkSink {
    chunks: Vec<(usize, usize)>,
    pos: usize,
    written: usize
}

impl ChunkSink {
    fn end_chunk(&mut self) {
        self.chunks.push((self.pos, self.written));
        self.pos += self.written;
        self.written = 0;
    }
}

impl Write for ChunkSink {
    fn write(&mut self, data: &[u8]) -> Result<usize, io::Error> {
        self.written += data.len();
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

fn chunk(data: &[u8], mut chunker: Chunker, sink: &mut ChunkSink) {
    let mut cursor = Cursor::new(data);
    while chunker.chunk(&mut cursor, sink).unwrap() == ChunkerStatus::Continue {
        sink.end_chunk();
    }
    sink.end_chunk();
}

#[allow(dead_code)]
pub fn run(path: &str, bundle_size: usize, chunker: ChunkerType, compression: Option<Compression>, encrypt: bool,hash: HashMethod) {
    let mut total_write_time = 0.0;
    let mut total_read_time = 0.0;

    println!("Reading input file ...");
    let mut file = File::open(path).unwrap();
    let total_size = file.metadata().unwrap().len();
    let mut size = total_size;
    let mut data = Vec::with_capacity(size as usize);
    let read_time = Duration::span(|| {
        file.read_to_end(&mut data).unwrap();
    }).num_milliseconds() as f32 / 1_000.0;
    println!("- {}, {}", to_duration(read_time), to_speed(size, read_time));

    println!();

    println!("Chunking data with {}, avg chunk size {} ...", chunker.name(), to_file_size(chunker.avg_size() as u64));
    let mut chunk_sink = ChunkSink {
        chunks: Vec::with_capacity(2*size as usize/chunker.avg_size()),
        written: 0,
        pos: 0
    };
    let chunker = chunker.create();
    let chunk_time = Duration::span(|| {
        chunk(&data, chunker, &mut chunk_sink)
    }).num_milliseconds() as f32 / 1_000.0;
    total_write_time += chunk_time;
    println!("- {}, {}", to_duration(chunk_time), to_speed(size, chunk_time));
    let mut chunks = chunk_sink.chunks;
    assert_eq!(chunks.iter().map(|c| c.1).sum::<usize>(), size as usize);
    let chunk_size_avg = size as f32 / chunks.len() as f32;
    let chunk_size_stddev = (chunks.iter().map(|c| (c.1 as f32 - chunk_size_avg).powi(2)).sum::<f32>() / (chunks.len() as f32 - 1.0)).sqrt();
    println!("- {} chunks, avg size: {} ±{}", chunks.len(), to_file_size(chunk_size_avg as u64), to_file_size(chunk_size_stddev as u64));

    println!();

    println!("Hashing chunks with {} ...", hash.name());
    let mut hashes = Vec::with_capacity(chunks.len());
    let hash_time = Duration::span(|| {
        for &(pos, len) in &chunks {
            hashes.push(hash.hash(&data[pos..pos+len]))
        }
    }).num_milliseconds() as f32 / 1_000.0;
    total_write_time += hash_time;
    println!("- {}, {}", to_duration(hash_time), to_speed(size, hash_time));
    let mut seen_hashes = HashSet::with_capacity(hashes.len());
    let mut dups = Vec::new();
    for (i, hash) in hashes.into_iter().enumerate() {
        if !seen_hashes.insert(hash) {
            dups.push(i);
        }
    }
    let mut dup_size = 0;
    dups.reverse();
    for i in &dups {
        let (_, len) = chunks.remove(*i);
        dup_size += len;
    }
    println!("- {} duplicate chunks, {}, {:.1}% saved", dups.len(), to_file_size(dup_size as u64), dup_size as f32 / size as f32*100.0);
    size -= dup_size as u64;

    let mut bundles = Vec::new();

    if let Some(compression) = compression.clone() {
        println!();

        println!("Compressing chunks with {} ...", compression.to_string());
        let compress_time = Duration::span(|| {
            let mut bundle = Vec::with_capacity(bundle_size + 2*chunk_size_avg as usize);
            let mut c = compression.compress_stream().unwrap();
            for &(pos, len) in &chunks {
                c.process(&data[pos..pos+len], &mut bundle).unwrap();
                if bundle.len() >= bundle_size {
                    c.finish(&mut bundle).unwrap();
                    bundles.push(bundle);
                    bundle = Vec::with_capacity(bundle_size + 2*chunk_size_avg as usize);
                    c = compression.compress_stream().unwrap();
                }
            }
            c.finish(&mut bundle).unwrap();
            bundles.push(bundle);
        }).num_milliseconds() as f32 / 1_000.0;
        total_write_time += compress_time;
        println!("- {}, {}", to_duration(compress_time), to_speed(size, compress_time));
        let compressed_size = bundles.iter().map(|b| b.len()).sum::<usize>();
        println!("- {} bundles, {}, {:.1}% saved", bundles.len(), to_file_size(compressed_size as u64), (size as f32 - compressed_size as f32)/size as f32*100.0);
        size = compressed_size as u64;
    } else {
        let mut bundle = Vec::with_capacity(bundle_size + 2*chunk_size_avg as usize);
        for &(pos, len) in &chunks {
            bundle.extend_from_slice(&data[pos..pos+len]);
            if bundle.len() >= bundle_size {
                bundles.push(bundle);
                bundle = Vec::with_capacity(bundle_size + 2*chunk_size_avg as usize);
            }
        }
        bundles.push(bundle);
    }

    if encrypt {
        println!();

        let (public, secret) = Crypto::gen_keypair();
        let mut crypto = Crypto::dummy();
        crypto.add_secret_key(public, secret);
        let encryption = (EncryptionMethod::Sodium, public[..].to_vec().into());

        println!("Encrypting bundles...");
        let mut encrypted_bundles = Vec::with_capacity(bundles.len());

        let encrypt_time = Duration::span(|| {
            for bundle in bundles {
                encrypted_bundles.push(crypto.encrypt(&encryption, &bundle).unwrap());
            }
        }).num_milliseconds() as f32 / 1_000.0;
        println!("- {}, {}", to_duration(encrypt_time), to_speed(size, encrypt_time));
        total_write_time += encrypt_time;

        println!();

        println!("Decrypting bundles...");
        bundles = Vec::with_capacity(encrypted_bundles.len());
        let decrypt_time = Duration::span(|| {
            for bundle in encrypted_bundles {
                bundles.push(crypto.decrypt(&encryption, &bundle).unwrap());
            }
        }).num_milliseconds() as f32 / 1_000.0;
        println!("- {}, {}", to_duration(decrypt_time), to_speed(size, decrypt_time));
        total_read_time += decrypt_time;
    }

    if let Some(compression) = compression {
        println!();

        println!("Decompressing bundles with {} ...", compression.to_string());
        let mut dummy = ChunkSink { chunks: vec![], written: 0, pos: 0 };
        let decompress_time = Duration::span(|| {
            for bundle in &bundles {
                let mut c = compression.decompress_stream().unwrap();
                c.process(bundle, &mut dummy).unwrap();
                c.finish(&mut dummy).unwrap();
            }
        }).num_milliseconds() as f32 / 1_000.0;
        println!("- {}, {}", to_duration(decompress_time), to_speed(size, decompress_time));
        total_read_time += decompress_time;
    }

    println!();

    println!("Total storage size: {} / {}, ratio: {:.1}%", to_file_size(size as u64), to_file_size(total_size as u64), size as f32/total_size as f32*100.0);
    println!("Total processing speed: {}", to_speed(total_size, total_write_time));
    println!("Total read speed: {}", to_speed(total_size, total_read_time));
}
