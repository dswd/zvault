mod args;
mod logger;
mod algotest;

use chrono::prelude::*;
use std::process::exit;

use ::repository::{Repository, Config, Inode, Backup};
use ::util::ChecksumType;
use ::util::cli::*;
use self::args::Arguments;


fn open_repository(path: &str) -> Repository {
    match Repository::open(path) {
        Ok(repo) => repo,
        Err(err) => {
            error!("Failed to open repository: {}", err);
            exit(2);
        }
    }
}

fn get_backup(repo: &Repository, backup_name: &str) -> Backup {
    match repo.get_backup(backup_name) {
        Ok(backup) => backup,
        Err(err) => {
            error!("Failed to load backup: {}", err);
            exit(3);
        }
    }
}

pub fn run() {
    if let Err(err) = logger::init() {
        println!("Failed to initialize the logger: {}", err);
        exit(-1)
    }
    match args::parse() {
        Arguments::Init{repo_path, bundle_size, chunker, compression, hash} => {
            Repository::create(repo_path, Config {
                bundle_size: bundle_size,
                checksum: ChecksumType::Blake2_256,
                chunker: chunker,
                compression: compression,
                hash: hash
            }).unwrap();
        },
        Arguments::Backup{repo_path, backup_name, src_path, full} => {
            let mut repo = open_repository(&repo_path);
            if !full {
                warn!("Partial backups are not implemented yet, creating full backup");
            }
            let backup = repo.create_full_backup(&src_path).unwrap();
            repo.save_backup(&backup, &backup_name).unwrap();
        },
        Arguments::Restore{repo_path, backup_name, inode, dst_path} => {
            let mut repo = open_repository(&repo_path);
            let backup = get_backup(&repo, &backup_name);
            if let Some(inode) = inode {
                let inode = repo.get_backup_inode(&backup, &inode).unwrap();
                repo.restore_inode_tree(inode, &dst_path).unwrap();
            } else {
                repo.restore_backup(&backup, &dst_path).unwrap();
            }
        },
        Arguments::Check{repo_path, backup_name, inode, full} => {
            let mut repo = open_repository(&repo_path);
            if let Some(backup_name) = backup_name {
                let backup = get_backup(&repo, &backup_name);
                if let Some(inode) = inode {
                    unimplemented!()
                } else {
                    unimplemented!()
                }
            } else {
                repo.check(full).unwrap()
            }
        },
        Arguments::List{repo_path, backup_name, inode} => {
            let mut repo = open_repository(&repo_path);
            if let Some(backup_name) = backup_name {
                let backup = get_backup(&repo, &backup_name);
                let inode = repo.get_backup_inode(&backup, inode.as_ref().map(|v| v as &str).unwrap_or("/")).unwrap();
                println!("{}", format_inode_one_line(&inode));
                if let Some(children) = inode.children {
                    for chunks in children.values() {
                        let inode = repo.get_inode(chunks).unwrap();
                        println!("- {}", format_inode_one_line(&inode));
                    }
                }
            } else {
                for backup in repo.list_backups().unwrap() {
                    println!("{}", backup);
                }
            }
        }
        Arguments::Info{repo_path, backup_name, inode} => {
            let repo = open_repository(&repo_path);
            if let Some(backup_name) = backup_name {
                let backup = get_backup(&repo, &backup_name);
                if let Some(inode) = inode {
                    unimplemented!()
                } else {
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
                }
            } else {
                let info = repo.info();
                println!("Bundles: {}", info.bundle_count);
                println!("Total size: {}", to_file_size(info.encoded_data_size));
                println!("Uncompressed size: {}", to_file_size(info.raw_data_size));
                println!("Compression ratio: {:.1}%", info.compression_ratio * 100.0);
                println!("Chunk count: {}", info.chunk_count);
                println!("Average chunk size: {}", to_file_size(info.avg_chunk_size as u64));
                let index_usage = info.index_entries as f32 / info.index_capacity as f32;
                println!("Index: {}, {:.0}% full", to_file_size(info.index_size as u64), index_usage * 100.0);
            }
        }
        Arguments::ListBundles{repo_path} => {
            let repo = open_repository(&repo_path);
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
        }
        Arguments::AlgoTest{bundle_size, chunker, compression, hash, file} => {
            algotest::run(&file, bundle_size, chunker, compression, hash);
        }
    }
}
