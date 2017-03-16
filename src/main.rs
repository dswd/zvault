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
extern crate chrono;

pub mod util;
pub mod bundle;
pub mod index;
mod chunker;
mod repository;
mod algotest;

use docopt::Docopt;
use chrono::prelude::*;

use chunker::ChunkerType;
use repository::{Repository, Config, Inode};
use util::{ChecksumType, Compression, HashMethod, to_file_size, to_duration};


static USAGE: &'static str = "
Usage:
    zvault init [--bundle-size SIZE] [--chunker METHOD] [--chunk-size SIZE] [--compression COMPRESSION] <repo>
    zvault backup [--full] <backup> <path>
    zvault restore <backup> <path>
    zvault check [--full] <repo>
    zvault backups <repo>
    zvault info <backup>
    zvault list <backup> <path>
    zvault stats <repo>
    zvault bundles <repo>
    zvault algotest <path>
    zvault stat <path>

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

    cmd_backups: bool,
    cmd_info: bool,
    cmd_list: bool,

    cmd_stats: bool,
    cmd_bundles: bool,

    cmd_algotest: bool,
    cmd_stat: bool,

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
        println!("Compression ratio: {:.1}%", info.compression_ratio * 100.0);
        println!("Chunk count: {}", info.chunk_count);
        println!("Average chunk size: {}", to_file_size(info.avg_chunk_size as u64));
        let index_usage = info.index_entries as f32 / info.index_capacity as f32;
        println!("Index: {}, {:.0}% full", to_file_size(info.index_size as u64), index_usage * 100.0);
        return
    }

    if args.cmd_backups {
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

    let backup_name = args.arg_backup.unwrap().splitn(2, "::").nth(1).unwrap().to_string();

    if args.cmd_backup {
        let backup = repo.create_full_backup(&args.arg_path.unwrap()).unwrap();
        repo.save_backup(&backup, &backup_name).unwrap();
        return
    }

    let backup = repo.get_backup(&backup_name).unwrap();

    if args.cmd_info {
        println!("Date: {}", Local.timestamp(backup.date, 0).to_rfc2822());
        println!("Duration: {}", to_duration(backup.duration));
        println!("Entries: {} files, {} dirs", backup.file_count, backup.dir_count);
        println!("Total backup size: {}", to_file_size(backup.total_data_size));
        println!("Modified data size: {}", to_file_size(backup.changed_data_size));
        let dedup_ratio = backup.deduplicated_data_size as f32 / backup.changed_data_size as f32;
        println!("Deduplicated size: {}, {:.1}% saved", to_file_size(backup.deduplicated_data_size), (1.0 - dedup_ratio)*100.0);
        let compress_ratio = backup.encoded_data_size as f32 / backup.deduplicated_data_size as f32;
        println!("Compressed size: {} in {} bundles, {:.1}% saved", to_file_size(backup.encoded_data_size), backup.bundle_count, (1.0 - compress_ratio)*100.0);
        println!("Chunk count: {}, avg size: {}", backup.chunk_count, to_file_size(backup.avg_chunk_size as u64));
        return
    }

    if args.cmd_restore {
        repo.restore_backup(&backup, &args.arg_path.unwrap()).unwrap();
        return
    }

    if args.cmd_list {
        let inode = repo.get_backup_inode(&backup, &args.arg_path.unwrap()).unwrap();
        println!("{}", inode.format_one_line());
        if let Some(children) = inode.children {
            for chunks in children.values() {
                let inode = repo.get_inode(&chunks).unwrap();
                println!("- {}", inode.format_one_line());
            }
        }
        return
    }
}
