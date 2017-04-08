mod args;
mod logger;
mod algotest;

use ::prelude::*;

use chrono::prelude::*;
use regex::{self, RegexSet};

use std::collections::HashMap;
use std::io::{BufReader, BufRead};
use std::fs::File;
use std::env;

use self::args::Arguments;


pub enum ErrorCode {
    UnsafeArgs, InvalidArgs,
    InitializeLogger,
    CreateRepository,
    LoadRepository, SaveBackup, LoadBackup, LoadInode, LoadBundle,
    AddKey, LoadKey, SaveKey,
    SaveConfig,
    LoadExcludes, InvalidExcludes,
    BackupRun, RestoreRun, RemoveRun, PruneRun, VacuumRun, CheckRun, AnalyzeRun, DiffRun,
    VersionsRun, ImportRun, FuseMount, RebuildIndexRun
}
impl ErrorCode {
    pub fn code(&self) -> i32 {
        match *self {
            // Crazy stuff
            ErrorCode::InitializeLogger | ErrorCode::InvalidExcludes => -1,
            // Arguments
            ErrorCode::InvalidArgs => 1,
            ErrorCode::UnsafeArgs => 2,
            // Load things
            ErrorCode::LoadRepository => 3,
            ErrorCode::LoadBackup => 4,
            ErrorCode::LoadInode => 5,
            ErrorCode::LoadBundle => 6,
            ErrorCode::LoadKey => 7,
            ErrorCode::LoadExcludes => 8,
            // Minor operations
            ErrorCode::SaveBackup => 9,
            ErrorCode::AddKey => 10,
            ErrorCode::SaveKey => 11,
            ErrorCode::SaveConfig => 12,
            // Main operation
            ErrorCode::CreateRepository => 13,
            ErrorCode::BackupRun => 14,
            ErrorCode::RestoreRun => 15,
            ErrorCode::RemoveRun => 16,
            ErrorCode::PruneRun => 17,
            ErrorCode::VacuumRun => 18,
            ErrorCode::CheckRun => 19,
            ErrorCode::AnalyzeRun => 20,
            ErrorCode::DiffRun => 21,
            ErrorCode::VersionsRun => 22,
            ErrorCode::ImportRun => 23,
            ErrorCode::FuseMount => 24,
            ErrorCode::RebuildIndexRun => 25
        }
    }
}


pub const DEFAULT_CHUNKER: &'static str = "fastcdc/16";
pub const DEFAULT_HASH: &'static str = "blake2";
pub const DEFAULT_COMPRESSION: &'static str = "brotli/3";
pub const DEFAULT_BUNDLE_SIZE_STR: &'static str = "25";
pub const DEFAULT_VACUUM_RATIO_STR: &'static str = "0";
lazy_static! {
    pub static ref DEFAULT_REPOSITORY: String = {
        env::home_dir().unwrap().join(".zvault").to_string_lossy().to_string()
    };
}

macro_rules! checked {
    ($expr:expr, $msg:expr, $code:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                error!("Failed to {}\n\tcaused by: {}", $msg, err);
                return Err($code)
            }
        }
    };
}

fn open_repository(path: &str) -> Result<Repository, ErrorCode> {
    Ok(checked!(Repository::open(path), "load repository", ErrorCode::LoadRepository))
}

fn get_backup(repo: &Repository, backup_name: &str) -> Result<Backup, ErrorCode> {
    Ok(checked!(repo.get_backup(backup_name), "load backup", ErrorCode::LoadBackup))
}

fn find_reference_backup(repo: &Repository, path: &str) -> Result<Option<(String, Backup)>, ErrorCode> {
    let mut matching = Vec::new();
    let hostname = match get_hostname() {
        Ok(hostname) => hostname,
        Err(_) => return Ok(None)
    };
    let backup_map = match repo.get_backups() {
        Ok(backup_map) => backup_map,
        Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
            warn!("Some backups could not be read, ignoring them");
            backup_map
        },
        Err(err) => {
            error!("Failed to load backup files: {}", err);
            return Err(ErrorCode::LoadBackup)
        }
    };
    for (name, backup) in backup_map {
        if backup.host == hostname && backup.path == path {
            matching.push((name, backup));
        }
    }
    matching.sort_by_key(|&(_, ref b)| b.date);
    Ok(matching.pop())
}

fn print_backup(backup: &Backup) {
    if backup.modified {
        warn!("This backup has been modified");
    }
    println!("Date: {}", Local.timestamp(backup.date, 0).to_rfc2822());
    println!("Source: {}:{}", backup.host, backup.path);
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

pub fn format_inode_one_line(inode: &Inode) -> String {
    match inode.file_type {
        FileType::Directory => format!("{:25}\t{} entries", format!("{}/", inode.name), inode.children.as_ref().map(|c| c.len()).unwrap_or(0)),
        FileType::File => format!("{:25}\t{:>10}\t{}", inode.name, to_file_size(inode.size), Local.timestamp(inode.timestamp, 0).to_rfc2822()),
        FileType::Symlink => format!("{:25}\t -> {}", inode.name, inode.symlink_target.as_ref().map(|s| s as &str).unwrap_or("?")),
    }
}

fn print_inode(inode: &Inode) {
    println!("Name: {}", inode.name);
    println!("Type: {}", inode.file_type);
    println!("Size: {}", to_file_size(inode.size));
    println!("Permissions: {:3o}", inode.mode);
    println!("User: {}", inode.user);
    println!("Group: {}", inode.group);
    println!("Timestamp: {}", Local.timestamp(inode.timestamp, 0).to_rfc2822());
    if let Some(ref target) = inode.symlink_target {
        println!("Symlink target: {}", target);
    }
    println!("Cumulative size: {}", to_file_size(inode.cum_size));
    println!("Cumulative file count: {}", inode.cum_files);
    println!("Cumulative directory count: {}", inode.cum_dirs);
    if let Some(ref children) = inode.children {
        println!("Children:");
        for name in children.keys() {
            println!("  - {}", name);
        }
    }
}

fn print_backups(backup_map: &HashMap<String, Backup>) {
    let mut backups: Vec<_> = backup_map.into_iter().collect();
    backups.sort_by_key(|b| b.0);
    for (name, backup) in backups {
        println!("{:40}  {:>32}  {:7} files, {:6} dirs, {:>10}",
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

fn print_bundle(bundle: &StoredBundle) {
    println!("Bundle {}", bundle.info.id);
    println!("  - Mode: {:?}", bundle.info.mode);
    println!("  - Path: {:?}", bundle.path);
    println!("  - Hash method: {:?}", bundle.info.hash_method);
    let encryption = if let Some((_, ref key)) = bundle.info.encryption {
        to_hex(key)
    } else {
        "none".to_string()
    };
    println!("  - Encryption: {}", encryption);
    println!("  - Chunks: {}", bundle.info.chunk_count);
    println!("  - Size: {}", to_file_size(bundle.info.encoded_size as u64));
    println!("  - Data size: {}", to_file_size(bundle.info.raw_size as u64));
    let ratio = bundle.info.encoded_size as f32 / bundle.info.raw_size as f32;
    let compression = if let Some(ref c) = bundle.info.compression {
        c.to_string()
    } else {
        "none".to_string()
    };
    println!("  - Compression: {}, ratio: {:.1}%", compression, ratio * 100.0);
}

fn print_bundle_one_line(bundle: &BundleInfo) {
    println!("{}: {:8?}, {:5} chunks, {:8}", bundle.id, bundle.mode, bundle.chunk_count, to_file_size(bundle.encoded_size as u64))
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

fn print_analysis(analysis: &HashMap<u32, BundleAnalysis>) {
    let mut reclaim_space = [0; 11];
    let mut data_total = 0;
    for bundle in analysis.values() {
        data_total += bundle.info.encoded_size;
        #[allow(unknown_lints,needless_range_loop)]
        for i in 0..11 {
            if bundle.get_usage_ratio() <= i as f32 * 0.1 {
                reclaim_space[i] += bundle.get_unused_size();
            }
        }
    }
    println!("Total bundle size: {}", to_file_size(data_total as u64));
    let used = data_total - reclaim_space[10];
    println!("Space used: {}, {:.1} %", to_file_size(used as u64), used as f32 / data_total as f32 * 100.0);
    println!("Reclaimable space (depending on vacuum ratio)");
    #[allow(unknown_lints,needless_range_loop)]
    for i in 0..11 {
        println!("  - ratio={:3}: {:>10}, {:4.1} %", i*10, to_file_size(reclaim_space[i] as u64), reclaim_space[i] as f32 / data_total as f32 * 100.0);
    }
}


#[allow(unknown_lints,cyclomatic_complexity)]
pub fn run() -> Result<(), ErrorCode> {
    if let Err(err) = logger::init() {
        println!("Failed to initialize the logger: {}", err);
        return Err(ErrorCode::InitializeLogger)
    }
    match try!(args::parse()) {
        Arguments::Init{repo_path, bundle_size, chunker, compression, encryption, hash, remote_path} => {
            let mut repo = checked!(Repository::create(repo_path, Config {
                bundle_size: bundle_size,
                chunker: chunker,
                compression: compression,
                encryption: None,
                hash: hash
            }, remote_path), "create repository", ErrorCode::CreateRepository);
            if encryption {
                let (public, secret) = gen_keypair();
                info!("Created the following key pair");
                println!("public: {}", to_hex(&public[..]));
                println!("secret: {}", to_hex(&secret[..]));
                repo.set_encryption(Some(&public));
                checked!(repo.register_key(public, secret), "add key", ErrorCode::AddKey);
                checked!(repo.save_config(), "save config", ErrorCode::SaveConfig);
                warn!("Please store this key pair in a secure location before using the repository");
                println!();
            }
            print_config(&repo.config);
        },
        Arguments::Backup{repo_path, backup_name, src_path, full, reference, same_device, mut excludes, excludes_from, no_default_excludes, tar} => {
            let mut repo = try!(open_repository(&repo_path));
            let mut reference_backup = None;
            if !full && !tar {
                reference_backup = match reference {
                    Some(r) => {
                        let b = try!(get_backup(&repo, &r));
                        Some((r, b))
                    },
                    None => None
                };
                if reference_backup.is_none() {
                    reference_backup = try!(find_reference_backup(&repo, &src_path));
                }
                if let Some(&(ref name, _)) = reference_backup.as_ref() {
                    info!("Using backup {} as reference", name);
                } else {
                    info!("No reference backup found, doing a full scan instead");
                }
            }
            let reference_backup = reference_backup.map(|(_, backup)| backup);
            if !no_default_excludes && !tar {
                for line in BufReader::new(checked!(File::open(&repo.layout.excludes_path()), "open default excludes file", ErrorCode::LoadExcludes)).lines() {
                    excludes.push(checked!(line, "read default excludes file", ErrorCode::LoadExcludes));
                }
            }
            if let Some(excludes_from) = excludes_from {
                for line in BufReader::new(checked!(File::open(excludes_from), "open excludes file", ErrorCode::LoadExcludes)).lines() {
                    excludes.push(checked!(line, "read excludes file", ErrorCode::LoadExcludes));
                }
            }
            let mut excludes_parsed = Vec::with_capacity(excludes.len());
            for mut exclude in excludes {
                if exclude.starts_with('#') || exclude.is_empty() {
                    continue
                }
                exclude = regex::escape(&exclude).replace('?', ".").replace(r"\*\*", ".*").replace(r"\*", "[^/]*");
                excludes_parsed.push(if exclude.starts_with('/') {
                    format!(r"^{}($|/)", exclude)
                } else {
                    format!(r"/{}($|/)", exclude)
                });
            };
            let excludes = if excludes_parsed.is_empty() {
                None
            } else {
                Some(checked!(RegexSet::new(excludes_parsed), "parse exclude patterns", ErrorCode::InvalidExcludes))
            };
            let options = BackupOptions {
                same_device: same_device,
                excludes: excludes
            };
            let result = if tar {
                repo.import_tarfile(&src_path)
            } else {
                repo.create_backup_recursively(&src_path, reference_backup.as_ref(), &options)
            };
            info!("Backup finished");
            let backup = match result {
                Ok(backup) => backup,
                Err(RepositoryError::Backup(BackupError::FailedPaths(backup, _failed_paths))) => {
                    warn!("Some files are missing from the backup");
                    backup
                },
                Err(err) => {
                    error!("Backup failed: {}", err);
                    return Err(ErrorCode::BackupRun)
                }
            };
            checked!(repo.save_backup(&backup, &backup_name), "save backup file", ErrorCode::SaveBackup);
            print_backup(&backup);
        },
        Arguments::Restore{repo_path, backup_name, inode, dst_path, tar} => {
            let mut repo = try!(open_repository(&repo_path));
            let backup = try!(get_backup(&repo, &backup_name));
            let inode = if let Some(inode) = inode {
                checked!(repo.get_backup_inode(&backup, &inode), "load subpath inode", ErrorCode::LoadInode)
            } else {
                checked!(repo.get_inode(&backup.root), "load root inode", ErrorCode::LoadInode)
            };
            if tar {
                checked!(repo.export_tarfile(inode, &dst_path), "restore backup", ErrorCode::RestoreRun);
            } else {
                checked!(repo.restore_inode_tree(inode, &dst_path), "restore backup", ErrorCode::RestoreRun);
            }
            info!("Backup finished");
        },
        Arguments::Remove{repo_path, backup_name, inode} => {
            let mut repo = try!(open_repository(&repo_path));
            if let Some(inode) = inode {
                let mut backup = try!(get_backup(&repo, &backup_name));
                checked!(repo.remove_backup_path(&mut backup, inode), "remove backup subpath", ErrorCode::RemoveRun);
                checked!(repo.save_backup(&backup, &backup_name), "save backup file", ErrorCode::SaveBackup);
                info!("The backup subpath has been deleted, run vacuum to reclaim space");
            } else {
                checked!(repo.delete_backup(&backup_name), "delete backup", ErrorCode::RemoveRun);
                info!("The backup has been deleted, run vacuum to reclaim space");
            }
        },
        Arguments::Prune{repo_path, prefix, daily, weekly, monthly, yearly, force} => {
            let repo = try!(open_repository(&repo_path));
            if daily + weekly + monthly + yearly == 0 {
                error!("This would remove all those backups");
                return Err(ErrorCode::UnsafeArgs)
            }
            checked!(repo.prune_backups(&prefix, daily, weekly, monthly, yearly, force), "prune backups", ErrorCode::PruneRun);
            if !force {
                info!("Run with --force to actually execute this command");
            }
        },
        Arguments::Vacuum{repo_path, ratio, force} => {
            let mut repo = try!(open_repository(&repo_path));
            let info_before = repo.info();
            checked!(repo.vacuum(ratio, force), "vacuum", ErrorCode::VacuumRun);
            if !force {
                info!("Run with --force to actually execute this command");
            } else {
                let info_after = repo.info();
                info!("Reclaimed {}", to_file_size(info_before.encoded_data_size - info_after.encoded_data_size));
            }
        },
        Arguments::Check{repo_path, backup_name, inode, full} => {
            let mut repo = try!(open_repository(&repo_path));
            if let Some(backup_name) = backup_name {
                let backup = try!(get_backup(&repo, &backup_name));
                if let Some(inode) = inode {
                    let inode = checked!(repo.get_backup_inode(&backup, inode), "load subpath inode", ErrorCode::LoadInode);
                    checked!(repo.check_inode(&inode), "check inode", ErrorCode::CheckRun)
                } else {
                    checked!(repo.check_backup(&backup), "check backup", ErrorCode::CheckRun)
                }
            } else {
                checked!(repo.check(full), "check repository", ErrorCode::CheckRun)
            }
            info!("Integrity verified")
        },
        Arguments::List{repo_path, backup_name, inode} => {
            let mut repo = try!(open_repository(&repo_path));
            if let Some(backup_name) = backup_name {
                let backup = try!(get_backup(&repo, &backup_name));
                let inode = checked!(repo.get_backup_inode(&backup, inode.as_ref().map(|v| v as &str).unwrap_or("/")), "load subpath inode", ErrorCode::LoadInode);
                println!("{}", format_inode_one_line(&inode));
                if let Some(children) = inode.children {
                    for chunks in children.values() {
                        let inode = checked!(repo.get_inode(chunks), "load child inode", ErrorCode::LoadInode);
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
                        return Err(ErrorCode::LoadBackup)
                    }
                };
                print_backups(&backup_map);
            }
        },
        Arguments::Info{repo_path, backup_name, inode} => {
            let mut repo = try!(open_repository(&repo_path));
            if let Some(backup_name) = backup_name {
                let backup = try!(get_backup(&repo, &backup_name));
                if let Some(inode) = inode {
                    let inode = checked!(repo.get_backup_inode(&backup, inode), "load subpath inode", ErrorCode::LoadInode);
                    print_inode(&inode);
                } else {
                    print_backup(&backup);
                }
            } else {
                print_repoinfo(&repo.info());
            }
        },
        Arguments::Mount{repo_path, backup_name, inode, mount_point} => {
            let mut repo = try!(open_repository(&repo_path));
            let fs = if let Some(backup_name) = backup_name {
                let backup = try!(get_backup(&repo, &backup_name));
                if let Some(inode) = inode {
                    let inode = checked!(repo.get_backup_inode(&backup, inode), "load subpath inode", ErrorCode::LoadInode);
                    checked!(FuseFilesystem::from_inode(&mut repo, inode), "create fuse filesystem", ErrorCode::FuseMount)
                } else {
                    checked!(FuseFilesystem::from_backup(&mut repo, &backup), "create fuse filesystem", ErrorCode::FuseMount)
                }
            } else {
                checked!(FuseFilesystem::from_repository(&mut repo), "create fuse filesystem", ErrorCode::FuseMount)
            };
            info!("Mounting the filesystem...");
            info!("Please unmount the filesystem via 'fusermount -u {}' when done.", mount_point);
            checked!(fs.mount(&mount_point), "mount filesystem", ErrorCode::FuseMount);
        },
        Arguments::Analyze{repo_path} => {
            let mut repo = try!(open_repository(&repo_path));
            print_analysis(&checked!(repo.analyze_usage(), "analyze repository", ErrorCode::AnalyzeRun));
        },
        Arguments::RebuildIndex{repo_path} => {
            let mut repo = try!(open_repository(&repo_path));
            checked!(repo.rebuild_index(), "rebuild index", ErrorCode::RebuildIndexRun);
        },
        Arguments::BundleList{repo_path} => {
            let repo = try!(open_repository(&repo_path));
            for bundle in repo.list_bundles() {
                print_bundle_one_line(bundle);
            }
        },
        Arguments::BundleInfo{repo_path, bundle_id} => {
            let repo = try!(open_repository(&repo_path));
            if let Some(bundle) = repo.get_bundle(&bundle_id) {
                print_bundle(bundle);
            } else {
                error!("No such bundle");
                return Err(ErrorCode::LoadBundle)
            }
        },
        Arguments::Import{repo_path, remote_path, key_files} => {
            checked!(Repository::import(repo_path, remote_path, key_files), "import repository", ErrorCode::ImportRun);
            info!("Import finished");
        },
        Arguments::Versions{repo_path, path} => {
            let mut repo = try!(open_repository(&repo_path));
            let mut found = false;
            for (name, mut inode) in checked!(repo.find_versions(&path), "find versions", ErrorCode::VersionsRun) {
                inode.name = format!("{}::{}", name, &path);
                println!("{}", format_inode_one_line(&inode));
                found = true;
            }
            if !found {
                info!("No versions of that file were found.");
            }
        },
        Arguments::Diff{repo_path_old, backup_name_old, inode_old, repo_path_new, backup_name_new, inode_new} => {
            if repo_path_old != repo_path_new {
                error!("Can only run diff on same repository");
                return Err(ErrorCode::InvalidArgs)
            }
            let mut repo = try!(open_repository(&repo_path_old));
            let backup_old = try!(get_backup(&repo, &backup_name_old));
            let backup_new = try!(get_backup(&repo, &backup_name_new));
            let inode1 = checked!(repo.get_backup_inode(&backup_old, inode_old.unwrap_or_else(|| "/".to_string())), "load subpath inode", ErrorCode::LoadInode);
            let inode2 = checked!(repo.get_backup_inode(&backup_new, inode_new.unwrap_or_else(|| "/".to_string())), "load subpath inode", ErrorCode::LoadInode);
            let diffs = checked!(repo.find_differences(&inode1, &inode2), "find differences", ErrorCode::DiffRun);
            for diff in &diffs {
                println!("{} {:?}", match diff.0 {
                    DiffType::Add => "add",
                    DiffType::Mod => "mod",
                    DiffType::Del => "del"
                }, diff.1);
            }
            if diffs.is_empty() {
                info!("No differences found");
            }
        },
        Arguments::Config{repo_path, bundle_size, chunker, compression, encryption, hash} => {
            let mut repo = try!(open_repository(&repo_path));
            let mut changed = false;
            if let Some(bundle_size) = bundle_size {
                repo.config.bundle_size = bundle_size;
                changed = true;
            }
            if let Some(chunker) = chunker {
                warn!("Changing the chunker makes it impossible to use existing data for deduplication");
                repo.config.chunker = chunker;
                changed = true;
            }
            if let Some(compression) = compression {
                repo.config.compression = compression;
                changed = true;
            }
            if let Some(encryption) = encryption {
                repo.set_encryption(encryption.as_ref());
                changed = true;
            }
            if let Some(hash) = hash {
                warn!("Changing the hash makes it impossible to use existing data for deduplication");
                repo.config.hash = hash;
                changed = true;
            }
            if changed {
                checked!(repo.save_config(), "save config", ErrorCode::SaveConfig);
                info!("The configuration has been updated.");
            } else {
                print_config(&repo.config);
            }
        },
        Arguments::GenKey{file} => {
            let (public, secret) = gen_keypair();
            info!("Created the following key pair");
            println!("public: {}", to_hex(&public[..]));
            println!("secret: {}", to_hex(&secret[..]));
            if let Some(file) = file {
                checked!(Crypto::save_keypair_to_file(&public, &secret, file), "save key pair", ErrorCode::SaveKey);
            }
        },
        Arguments::AddKey{repo_path, set_default, file} => {
            let mut repo = try!(open_repository(&repo_path));
            let (public, secret) = if let Some(file) = file {
                checked!(Crypto::load_keypair_from_file(file), "load key pair", ErrorCode::LoadKey)
            } else {
                info!("Created the following key pair");
                let (public, secret) = gen_keypair();
                println!("public: {}", to_hex(&public[..]));
                println!("secret: {}", to_hex(&secret[..]));
                (public, secret)
            };
            checked!(repo.register_key(public, secret), "add key pair", ErrorCode::AddKey);
            if set_default {
                repo.set_encryption(Some(&public));
                checked!(repo.save_config(), "save config", ErrorCode::SaveConfig);
                warn!("Please store this key pair in a secure location before using the repository");
            }
        },
        Arguments::AlgoTest{bundle_size, chunker, compression, encrypt, hash, file} => {
            algotest::run(&file, bundle_size, chunker, compression, encrypt, hash);
        }
    }
    Ok(())
}
