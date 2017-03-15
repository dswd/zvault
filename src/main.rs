extern crate serde;
extern crate rmp_serde;
#[macro_use] extern crate serde_utils;
extern crate squash_sys as squash;
extern crate mmap;
extern crate blake2_rfc as blake2;
extern crate murmurhash3;
extern crate serde_yaml;
#[macro_use] extern crate quick_error;
extern crate docopt;
extern crate rustc_serialize;

mod errors;
pub mod util;
pub mod bundle;
pub mod index;
mod chunker;
mod repository;
mod algotest;

use std::fs::File;
use std::io::Read;
use std::time;

use docopt::Docopt;

use chunker::ChunkerType;
use repository::{Repository, Config, Mode, Inode, Backup};
use util::{ChecksumType, Compression, HashMethod, to_file_size};


static USAGE: &'static str = "
Usage:
    zvault init [--bundle-size SIZE] [--chunker METHOD] [--chunk-size SIZE] [--compression COMPRESSION] <repo>
    zvault backup [--full] <backup> <path>
    zvault restore <backup> <path>
    zvault check [--full] <repo>
    zvault list <repo>
    zvault info <backup>
    zvault stats <repo>
    zvault bundles <repo>
    zvault algotest <path>
    zvault test <repo> <path>
    zvault stat <path>
    zvault put <backup> <path>

Options:
    --full                         Whether to verify the repository by loading all bundles
    --bundle-size SIZE             The target size of a full bundle in MiB [default: 25]
    --chunker METHOD               The chunking algorithm to use [default: fastcdc]
    --chunk-size SIZE              The target average chunk size in KiB [default: 8]
    --compression COMPRESSION      The compression to use [default: brotli/3]
";


#[derive(RustcDecodable, Debug)]
struct Args {
    cmd_init: bool,
    cmd_backup: bool,
    cmd_restore: bool,
    cmd_check: bool,

    cmd_list: bool,
    cmd_info: bool,

    cmd_stats: bool,
    cmd_bundles: bool,

    cmd_algotest: bool,
    cmd_test: bool,
    cmd_stat: bool,
    cmd_put: bool,

    arg_repo: Option<String>,
    arg_path: Option<String>,
    arg_backup: Option<String>,

    flag_full: bool,
    flag_bundle_size: usize,
    flag_chunker: String,
    flag_chunk_size: usize,
    flag_compression: String
}


fn main() {
    let args: Args = Docopt::new(USAGE).and_then(|d| d.decode()).unwrap_or_else(|e| e.exit());
    //println!("{:?}", args);

    if args.cmd_algotest {
        algotest::run(&args.arg_path.unwrap());
        return
    }

    if args.cmd_init {
        let chunker = ChunkerType::from(&args.flag_chunker, args.flag_chunk_size*1024, 0).expect("No such chunk algorithm");
        let compression = if args.flag_compression == "none" {
            None
        } else {
            Some(Compression::from_string(&args.flag_compression).expect("Failed to parse compression"))
        };
        Repository::create(&args.arg_repo.unwrap(), Config {
            bundle_size: args.flag_bundle_size*1024*1024,
            checksum: ChecksumType::Blake2_256,
            chunker: chunker,
            compression: compression,
            hash: HashMethod::Blake2
        }).unwrap();
        return
    }

    if args.cmd_stat {
        println!("{:?}", Inode::get_from(&args.arg_path.unwrap()).unwrap());
        return
    }


    let mut repo;
    if let Some(path) = args.arg_repo {
        repo = Repository::open(path).unwrap();
    } else if let Some(ref backup) = args.arg_backup {
        let path = backup.splitn(2, "::").nth(0).unwrap();
        repo = Repository::open(path).unwrap();
    } else {
        panic!("Repository is needed");
    }

    if args.cmd_check {
        repo.check(args.flag_full).unwrap();
        return
    }

    if args.cmd_stats {
        let info = repo.info();
        println!("Bundles: {}", info.bundle_count);
        println!("Total size: {}", to_file_size(info.encoded_data_size));
        println!("Uncompressed size: {}", to_file_size(info.raw_data_size));
        println!("Compression ratio: {:.1}", info.compression_ratio * 100.0);
        println!("Chunk count: {}", info.chunk_count);
        println!("Average chunk size: {}", to_file_size(info.avg_chunk_size as u64));
        let index_usage = info.index_entries as f32 / info.index_capacity as f32;
        println!("Index: {}, {}% full", to_file_size(info.index_size as u64), index_usage * 100.0);
        return
    }

    if args.cmd_list {
        for backup in repo.list_backups().unwrap() {
            println!("{}", backup);
        }
        return
    }

    if args.cmd_bundles {
        for bundle in repo.list_bundles() {
            println!("Bundle {}", bundle.id);
            println!("  - Chunks: {}", bundle.chunk_count);
            println!("  - Size: {}", to_file_size(bundle.encoded_size as u64));
            println!("  - Data size: {}", to_file_size(bundle.raw_size as u64));
            let ratio = bundle.encoded_size as f32 / bundle.raw_size as f32;
            let compression = if let Some(ref c) = bundle.compression {
                c.to_string()
            } else {
                "none".to_string()
            };
            println!("  - Compression: {}, ratio: {:.1}%", compression, ratio * 100.0);
            println!();
        }
        return
    }

    if args.cmd_test {
        print!("Integrity check before...");
        repo.check(true).unwrap();
        println!(" done.");

        let file_path = args.arg_path.unwrap();
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
        return
    }

    let backup_name = args.arg_backup.unwrap().splitn(2, "::").nth(1).unwrap().to_string();

    if args.cmd_put {
        let chunks = repo.put_inode(&args.arg_path.unwrap()).unwrap();
        repo.save_backup(&Backup{root: chunks, ..Default::default()}, &backup_name).unwrap();
        return
    }

    if args.cmd_backup {
        unimplemented!()
    }

    let backup = repo.get_backup(&backup_name).unwrap();

    if args.cmd_info {
        println!("{:?}", backup.root);
        return
    }

    if args.cmd_restore {
        repo.restore_backup(&backup, &args.arg_path.unwrap()).unwrap();
    }
}
