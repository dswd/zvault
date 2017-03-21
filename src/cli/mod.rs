mod args;
mod logger;
mod algotest;

use chrono::prelude::*;
use std::process::exit;

use ::repository::{Repository, Config, Backup};
use ::util::*;
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

fn find_reference_backup(repo: &Repository, path: &str) -> Option<Backup> {
    let mut matching = Vec::new();
    let hostname = match get_hostname() {
        Ok(hostname) => hostname,
        Err(_) => return None
    };
    for (_name, backup) in repo.get_backups().unwrap().0 {
        if backup.host == hostname && backup.path == path {
            matching.push(backup);
        }
    }
    matching.sort_by_key(|b| b.date);
    matching.pop()
}


#[allow(unknown_lints,cyclomatic_complexity)]
pub fn run() {
    if let Err(err) = logger::init() {
        println!("Failed to initialize the logger: {}", err);
        exit(-1)
    }
    match args::parse() {
        Arguments::Init{repo_path, bundle_size, chunker, compression, encryption, hash} => {
            let mut repo = Repository::create(repo_path, Config {
                bundle_size: bundle_size,
                chunker: chunker,
                compression: compression,
                encryption: None,
                hash: hash
            }).unwrap();
            if encryption {
                let (public, secret) = gen_keypair();
                println!("Public key: {}", to_hex(&public[..]));
                println!("Secret key: {}", to_hex(&secret[..]));
                repo.set_encryption(Some(&public));
                repo.register_key(public, secret).unwrap();
                repo.save_config().unwrap();
            }
        },
        Arguments::Backup{repo_path, backup_name, src_path, full, reference} => {
            let mut repo = open_repository(&repo_path);
            let mut reference_backup = None;
            if !full {
                reference_backup = reference.map(|r| get_backup(&repo, &r));
                if reference_backup.is_none() {
                    reference_backup = find_reference_backup(&repo, &src_path);
                }
                if let Some(ref backup) = reference_backup {
                    info!("Using backup from {} as reference", Local.timestamp(backup.date, 0).to_rfc2822());
                } else {
                    info!("No reference backup found, doing a full scan instead");
                }
            }
            let backup = repo.create_backup(&src_path, reference_backup.as_ref()).unwrap();
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
        Arguments::Remove{repo_path, backup_name, inode} => {
            let repo = open_repository(&repo_path);
            if let Some(_inode) = inode {
                let _backup = get_backup(&repo, &backup_name);
                error!("Removing backup subtrees is not implemented yet");
                return
            } else {
                repo.delete_backup(&backup_name).unwrap();
                info!("The backup has been deleted, run vacuum to reclaim space");
            }
        },
        Arguments::Prune{repo_path, prefix, daily, weekly, monthly, yearly, force} => {
            let repo = open_repository(&repo_path);
            if daily.is_none() && weekly.is_none() && monthly.is_none() && yearly.is_none() {
                error!("This would remove all those backups");
                exit(1);
            }
            repo.prune_backups(&prefix, daily, weekly, monthly, yearly, force).unwrap();
            if !force {
                info!("Run with --force to actually execute this command");
            }
        },
        Arguments::Vacuum{repo_path, ratio, force} => {
            let mut repo = open_repository(&repo_path);
            repo.vacuum(ratio, force).unwrap();
            if !force {
                info!("Run with --force to actually execute this command");
            }
            return
        },
        Arguments::Check{repo_path, backup_name, inode, full} => {
            let mut repo = open_repository(&repo_path);
            if let Some(backup_name) = backup_name {
                let _backup = get_backup(&repo, &backup_name);
                if let Some(_inode) = inode {
                    error!("Checking backup subtrees is not implemented yet");
                    return
                } else {
                    error!("Checking backups is not implemented yet");
                    return
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
                for (name, backup) in repo.get_backups().unwrap().0 {
                    println!("{:25}  {:>32}  {:5} files, {:4} dirs, {:>10}",
                        name, Local.timestamp(backup.date, 0).to_rfc2822(), backup.file_count,
                        backup.dir_count, to_file_size(backup.total_data_size));
                }
            }
        },
        Arguments::Info{repo_path, backup_name, inode} => {
            let repo = open_repository(&repo_path);
            if let Some(backup_name) = backup_name {
                let backup = get_backup(&repo, &backup_name);
                if let Some(_inode) = inode {
                    error!("Displaying information on single inodes is not implemented yet");
                    return
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
        },
        Arguments::ListBundles{repo_path} => {
            let repo = open_repository(&repo_path);
            for bundle in repo.list_bundles() {
                println!("Bundle {}", bundle.id);
                println!("  - Mode: {:?}", bundle.mode);
                println!("  - Hash method: {:?}", bundle.hash_method);
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
        },
        Arguments::Import{..} => {
            error!("Import is not implemented yet");
            return
        },
        Arguments::Configure{repo_path, bundle_size, chunker, compression, encryption, hash} => {
            let mut repo = open_repository(&repo_path);
            if let Some(bundle_size) = bundle_size {
                repo.config.bundle_size = bundle_size
            }
            if let Some(chunker) = chunker {
                warn!("Changing the chunker makes it impossible to use existing data for deduplication");
                repo.config.chunker = chunker
            }
            if let Some(compression) = compression {
                repo.config.compression = compression
            }
            if let Some(encryption) = encryption {
                repo.set_encryption(encryption.as_ref())
            }
            if let Some(hash) = hash {
                warn!("Changing the hash makes it impossible to use existing data for deduplication");
                repo.config.hash = hash
            }
            repo.save_config().unwrap();
            println!("Bundle size: {}", to_file_size(repo.config.bundle_size as u64));
            println!("Chunker: {}", repo.config.chunker.to_string());
            if let Some(ref compression) = repo.config.compression {
                println!("Compression: {}", compression.to_string());
            } else {
                println!("Compression: none");
            }
            if let Some(ref encryption) = repo.config.encryption {
                println!("Encryption: {}", to_hex(&encryption.1[..]));
            } else {
                println!("Encryption: none");
            }
            println!("Hash method: {}", repo.config.hash.name());
        },
        Arguments::GenKey{} => {
            let (public, secret) = gen_keypair();
            println!("Public key: {}", to_hex(&public[..]));
            println!("Secret key: {}", to_hex(&secret[..]));
        },
        Arguments::AddKey{repo_path, set_default, key_pair} => {
            let mut repo = open_repository(&repo_path);
            let (public, secret) = if let Some(key_pair) = key_pair {
                key_pair
            } else {
                let (public, secret) = gen_keypair();
                println!("Public key: {}", to_hex(&public[..]));
                println!("Secret key: {}", to_hex(&secret[..]));
                (public, secret)
            };
            if set_default {
                repo.set_encryption(Some(&public));
                repo.save_config().unwrap();
            }
            repo.register_key(public, secret).unwrap();
        },
        Arguments::AlgoTest{bundle_size, chunker, compression, encrypt, hash, file} => {
            algotest::run(&file, bundle_size, chunker, compression, encrypt, hash);
        }
    }
}
