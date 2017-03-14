extern crate serde;
extern crate rmp_serde;
#[macro_use] extern crate serde_utils;
extern crate squash_sys as squash;
extern crate mmap;
extern crate blake2_rfc as blake2;
extern crate murmurhash3;
extern crate serde_yaml;
#[macro_use] extern crate quick_error;

mod errors;
mod util;
mod bundle;
mod index;
mod chunker;
mod repository;
mod algotest;

use chunker::ChunkerType;
use repository::{Repository, Config, Mode};
use util::{ChecksumType, Compression, HashMethod};

use std::path::Path;
use std::fs::File;
use std::env;
use std::io::Read;
use std::time;


fn main() {
    let path: &Path = "test_data".as_ref();
    let mut repo = if path.exists() {
        Repository::open(path).unwrap()
    } else {
        Repository::create(path, Config {
            bundle_size: 25*1024*1024,
            checksum: ChecksumType::Blake2_256,
            chunker: ChunkerType::FastCdc((8*1024, 0)),
            compression: Some(Compression::Brotli(1)),
            hash: HashMethod::Blake2
        }).unwrap()
    };
    print!("Integrity check before...");
    repo.check(true).unwrap();
    println!(" done.");

    let file_path = env::args().nth(1).expect("Need file as argument");
    print!("Reading file {}...", file_path);
    let mut data = Vec::new();
    let mut file = File::open(file_path).unwrap();
    file.read_to_end(&mut data).unwrap();
    println!(" done. {} bytes", data.len());

    print!("Adding data to repository...");
    let start = time::Instant::now();
    let chunks = repo.put_data(Mode::Content, &data).unwrap();
    repo.flush().unwrap();
    let elapsed = start.elapsed();
    let duration = elapsed.as_secs() as f64 * 1.0 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
    let write_speed = data.len() as f64 / duration;
    println!(" done. {} chunks, {:.1} MB/s", chunks.len(), write_speed / 1_000_000.0);

    println!("Integrity check after...");
    repo.check(true).unwrap();
    println!(" done.");

    print!("Reading data from repository...");
    let start = time::Instant::now();
    let data2 = repo.get_data(&chunks).unwrap();
    let elapsed = start.elapsed();
    let duration = elapsed.as_secs() as f64 * 1.0 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
    let read_speed = data.len() as f64 / duration;
    assert_eq!(data.len(), data2.len());
    println!(" done. {:.1} MB/s", read_speed / 1_000_000.0);

    //algotest::run("test.tar");

}
