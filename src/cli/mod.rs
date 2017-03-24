mod args;
mod logger;
mod algotest;

use ::prelude::*;

use chrono::prelude::*;
use std::process::exit;
use std::collections::HashMap;
use std::fmt::Display;

use self::args::Arguments;


pub const DEFAULT_CHUNKER: &'static str = "fastcdc/16";
pub const DEFAULT_HASH: &'static str = "blake2";
pub const DEFAULT_COMPRESSION: &'static str = "brotli/3";
pub const DEFAULT_BUNDLE_SIZE: usize = 25;
pub const DEFAULT_VACUUM_RATIO: f32 = 0.5;


fn checked<T, E: Display>(result: Result<T, E>, msg: &'static str) -> T {
    match result {
        Ok(val) => val,
        Err(err) => {
            error!("Failed to {}\n\tcaused by: {}", msg, err);
            exit(3);
        }
    }
}

fn open_repository(path: &str) -> Repository {
    checked(Repository::open(path), "load repository")
}

fn get_backup(repo: &Repository, backup_name: &str) -> Backup {
    checked(repo.get_backup(backup_name), "load backup")
}

fn find_reference_backup(repo: &Repository, path: &str) -> Option<Backup> {
    let mut matching = Vec::new();
    let hostname = match get_hostname() {
        Ok(hostname) => hostname,
        Err(_) => return None
    };
    let backup_map = match repo.get_backups() {
        Ok(backup_map) => backup_map,
        Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
            warn!("Some backups could not be read, ignoring them");
            backup_map
        },
        Err(err) => {
            error!("Failed to load backup files: {}", err);
            exit(3)
        }
    };
    for (_name, backup) in backup_map {
        if backup.host == hostname && backup.path == path {
            matching.push(backup);
        }
    }
    matching.sort_by_key(|b| b.date);
    matching.pop()
}

fn print_backup(backup: &Backup) {
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

fn print_inode(inode: &Inode) {
    println!("Name: {}", inode.name);
    println!("Type: {}", inode.file_type);
    println!("Size: {}", to_file_size(inode.size));
    println!("Permissions: {:3o}", inode.mode);
    println!("User: {}", inode.user);
    println!("Group: {}", inode.group);
    println!("Access time: {}", Local.timestamp(inode.access_time, 0).to_rfc2822());
    println!("Modification time: {}", Local.timestamp(inode.modify_time, 0).to_rfc2822());
    if let Some(ref target) = inode.symlink_target {
        println!("Symlink target: {}", target);
    }
    if let Some(ref children) = inode.children {
        println!("Children:");
        for name in children.keys() {
            println!("  - {}", name);
        }
    }
}

fn print_backups(backup_map: &HashMap<String, Backup>) {
    for (name, backup) in backup_map {
        println!("{:25}  {:>32}  {:5} files, {:4} dirs, {:>10}",
            name, Local.timestamp(backup.date, 0).to_rfc2822(), backup.file_count,
            backup.dir_count, to_file_size(backup.total_data_size));
    }
}

fn print_repoinfo(info: &RepositoryInfo) {
    println!("Bundles: {}", info.bundle_count);
    println!("Total size: {}", to_file_size(info.encoded_data_size));
    println!("Uncompressed size: {}", to_file_size(info.raw_data_size));
    println!("Compression ratio: {:.1}%", info.compression_ratio * 100.0);
    println!("Chunk count: {}", info.chunk_count);
    println!("Average chunk size: {}", to_file_size(info.avg_chunk_size as u64));
    let index_usage = info.index_entries as f32 / info.index_capacity as f32;
    println!("Index: {}, {:.0}% full", to_file_size(info.index_size as u64), index_usage * 100.0);
}

fn print_bundle(bundle: &BundleInfo) {
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
}

fn print_config(config: &Config) {
    println!("Bundle size: {}", to_file_size(config.bundle_size as u64));
    println!("Chunker: {}", config.chunker.to_string());
    if let Some(ref compression) = config.compression {
        println!("Compression: {}", compression.to_string());
    } else {
        println!("Compression: none");
    }
    if let Some(ref encryption) = config.encryption {
        println!("Encryption: {}", to_hex(&encryption.1[..]));
    } else {
        println!("Encryption: none");
    }
    println!("Hash method: {}", config.hash.name());
}


#[allow(unknown_lints,cyclomatic_complexity)]
pub fn run() {
    if let Err(err) = logger::init() {
        println!("Failed to initialize the logger: {}", err);
        exit(-1)
    }
    match args::parse() {
        Arguments::Init{repo_path, bundle_size, chunker, compression, encryption, hash, remote_path} => {
            let mut repo = checked(Repository::create(repo_path, Config {
                bundle_size: bundle_size,
                chunker: chunker,
                compression: compression,
                encryption: None,
                hash: hash
            }, remote_path), "create repository");
            if encryption {
                let (public, secret) = gen_keypair();
                println!("public: {}", to_hex(&public[..]));
                println!("secret: {}", to_hex(&secret[..]));
                repo.set_encryption(Some(&public));
                checked(repo.register_key(public, secret), "add key");
                checked(repo.save_config(), "save config");
                println!();
            }
            print_config(&repo.config);
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
            let backup = match repo.create_backup_recursively(&src_path, reference_backup.as_ref()) {
                Ok(backup) => backup,
                Err(RepositoryError::Backup(BackupError::FailedPaths(backup, _failed_paths))) => {
                    warn!("Some files are missing from the backup");
                    backup
                },
                Err(err) => {
                    error!("Backup failed: {}", err);
                    exit(3)
                }
            };
            checked(repo.save_backup(&backup, &backup_name), "save backup file");
            print_backup(&backup);
        },
        Arguments::Restore{repo_path, backup_name, inode, dst_path} => {
            let mut repo = open_repository(&repo_path);
            let backup = get_backup(&repo, &backup_name);
            if let Some(inode) = inode {
                let inode = checked(repo.get_backup_inode(&backup, &inode), "load subpath inode");
                checked(repo.restore_inode_tree(inode, &dst_path), "restore subpath");
            } else {
                checked(repo.restore_backup(&backup, &dst_path), "restore backup");
            }
        },
        Arguments::Remove{repo_path, backup_name, inode} => {
            let mut repo = open_repository(&repo_path);
            if let Some(inode) = inode {
                let mut backup = get_backup(&repo, &backup_name);
                checked(repo.remove_backup_path(&mut backup, inode), "remove backup subpath");
                checked(repo.save_backup(&backup, &backup_name), "save backup file");
                info!("The backup subpath has been deleted, run vacuum to reclaim space");
            } else {
                checked(repo.delete_backup(&backup_name), "delete backup");
                info!("The backup has been deleted, run vacuum to reclaim space");
            }
        },
        Arguments::Prune{repo_path, prefix, daily, weekly, monthly, yearly, force} => {
            let repo = open_repository(&repo_path);
            if daily.is_none() && weekly.is_none() && monthly.is_none() && yearly.is_none() {
                error!("This would remove all those backups");
                exit(1);
            }
            checked(repo.prune_backups(&prefix, daily, weekly, monthly, yearly, force), "prune backups");
            if !force {
                info!("Run with --force to actually execute this command");
            }
        },
        Arguments::Vacuum{repo_path, ratio, force} => {
            let mut repo = open_repository(&repo_path);
            checked(repo.vacuum(ratio, force), "vacuum");
            if !force {
                info!("Run with --force to actually execute this command");
            }
        },
        Arguments::Check{repo_path, backup_name, inode, full} => {
            let mut repo = open_repository(&repo_path);
            if let Some(backup_name) = backup_name {
                let backup = get_backup(&repo, &backup_name);
                if let Some(inode) = inode {
                    let inode = checked(repo.get_backup_inode(&backup, inode), "load subpath inode");
                    checked(repo.check_inode(&inode), "check inode")
                } else {
                    checked(repo.check_backup(&backup), "check backup")
                }
            } else {
                checked(repo.check(full), "check repository")
            }
            info!("Integrity verified")
        },
        Arguments::List{repo_path, backup_name, inode} => {
            let mut repo = open_repository(&repo_path);
            if let Some(backup_name) = backup_name {
                let backup = get_backup(&repo, &backup_name);
                let inode = checked(repo.get_backup_inode(&backup, inode.as_ref().map(|v| v as &str).unwrap_or("/")), "load subpath inode");
                println!("{}", format_inode_one_line(&inode));
                if let Some(children) = inode.children {
                    for chunks in children.values() {
                        let inode = checked(repo.get_inode(chunks), "load child inode");
                        println!("- {}", format_inode_one_line(&inode));
                    }
                }
            } else {
                let backup_map = match repo.get_backups() {
                    Ok(backup_map) => backup_map,
                    Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
                        warn!("Some backups could not be read, ignoring them");
                        backup_map
                    },
                    Err(err) => {
                        error!("Failed to load backup files: {}", err);
                        exit(3)
                    }
                };
                print_backups(&backup_map);
            }
        },
        Arguments::Info{repo_path, backup_name, inode} => {
            let mut repo = open_repository(&repo_path);
            if let Some(backup_name) = backup_name {
                let backup = get_backup(&repo, &backup_name);
                if let Some(inode) = inode {
                    let inode = checked(repo.get_backup_inode(&backup, inode), "load subpath inode");
                    print_inode(&inode);
                } else {
                    print_backup(&backup);
                }
            } else {
                print_repoinfo(&repo.info());
            }
        },
        Arguments::ListBundles{repo_path} => {
            let repo = open_repository(&repo_path);
            for bundle in repo.list_bundles() {
                print_bundle(bundle);
                println!();
            }
        },
        Arguments::Import{repo_path, remote_path, key_files} => {
            checked(Repository::import(repo_path, remote_path, key_files), "import repository");
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
            checked(repo.save_config(), "save config");
            print_config(&repo.config);
        },
        Arguments::GenKey{file} => {
            let (public, secret) = gen_keypair();
            println!("public: {}", to_hex(&public[..]));
            println!("secret: {}", to_hex(&secret[..]));
            if let Some(file) = file {
                checked(Crypto::save_keypair_to_file(&public, &secret, file), "save key pair");
            }
        },
        Arguments::AddKey{repo_path, set_default, file} => {
            let mut repo = open_repository(&repo_path);
            let (public, secret) = if let Some(file) = file {
                checked(Crypto::load_keypair_from_file(file), "load key pair")
            } else {
                let (public, secret) = gen_keypair();
                println!("public: {}", to_hex(&public[..]));
                println!("secret: {}", to_hex(&secret[..]));
                (public, secret)
            };
            checked(repo.register_key(public, secret), "add key pair");
            if set_default {
                repo.set_encryption(Some(&public));
                checked(repo.save_config(), "save config");
            }
        },
        Arguments::AlgoTest{bundle_size, chunker, compression, encrypt, hash, file} => {
            algotest::run(&file, bundle_size, chunker, compression, encrypt, hash);
        }
    }
}
