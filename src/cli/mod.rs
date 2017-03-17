mod args;
mod algotest;

use chrono::prelude::*;

use ::chunker::ChunkerType;
use ::repository::{Repository, Config, Inode};
use ::util::{ChecksumType, Compression, HashMethod};
use ::util::cli::*;


pub fn run() {
    let args = args::parse();

    if args.cmd_algotest {
        let file = args.arg_file.unwrap();
        algotest::run(&file);
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
            println!("  - Mode: {:?}", bundle.mode);
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
        let dst = args.arg_dst.unwrap();
        if let Some(src) = args.arg_src {
            let inode = repo.get_backup_inode(&backup, src).unwrap();
            repo.restore_inode_tree(inode, &dst).unwrap();
        } else {
            repo.restore_backup(&backup, &dst).unwrap();
        }
        return
    }

    if args.cmd_list {
        let inode = repo.get_backup_inode(&backup, &args.arg_path.unwrap()).unwrap();
        println!("{}", format_inode_one_line(&inode));
        if let Some(children) = inode.children {
            for chunks in children.values() {
                let inode = repo.get_inode(chunks).unwrap();
                println!("- {}", format_inode_one_line(&inode));
            }
        }
        return
    }
}
