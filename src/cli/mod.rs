mod args;
mod logger;
mod algotest;

use prelude::*;

use chrono::prelude::*;
use regex::{self, RegexSet};

use std::collections::HashMap;
use std::io::{BufReader, BufRead};
use std::fs::File;
use std::env;
use std::str;
use std::path::{Path, PathBuf};

use self::args::Arguments;


pub enum ErrorCode {
    UnsafeArgs,
    InvalidArgs,
    InitializeLogger,
    CreateRepository,
    LoadRepository,
    SaveBackup,
    LoadBackup,
    LoadInode,
    LoadBundle,
    NoSuchBackup,
    BackupAlreadyExists,
    AddKey,
    LoadKey,
    SaveKey,
    SaveConfig,
    LoadExcludes,
    InvalidExcludes,
    BackupRun,
    RestoreRun,
    RemoveRun,
    PruneRun,
    VacuumRun,
    CheckRun,
    AnalyzeRun,
    DiffRun,
    VersionsRun,
    ImportRun,
    FuseMount,
    DuplicatesRun
}
impl ErrorCode {
    pub fn code(&self) -> i32 {
        match *self {
            // Crazy stuff
            ErrorCode::InitializeLogger |
            ErrorCode::InvalidExcludes => -1,
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
            ErrorCode::DuplicatesRun => 27,
            //
            ErrorCode::NoSuchBackup => 25,
            ErrorCode::BackupAlreadyExists => 26,
        }
    }
}


pub const DEFAULT_CHUNKER: &str = "fastcdc/16";
pub const DEFAULT_HASH: &str = "blake2";
pub const DEFAULT_COMPRESSION: &str = "brotli/3";
pub const DEFAULT_BUNDLE_SIZE_STR: &str = "25";
pub const DEFAULT_VACUUM_RATIO_STR: &str = "0";
pub const DEFAULT_DUPLICATES_MIN_SIZE_STR: &str = "1b";
lazy_static! {
    pub static ref ZVAULT_FOLDER: PathBuf = {
        env::home_dir().unwrap().join(".zvault")
    };
}

macro_rules! checked {
    ($expr:expr, $msg:expr, $code:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                tr_error!("Failed to {}\n\tcaused by: {}", tr!($msg), err);
                return Err($code)
            }
        }
    };
}

fn open_repository(path: &Path, online: bool) -> Result<Repository, ErrorCode> {
    Ok(checked!(
        Repository::open(path, online),
        "load repository",
        ErrorCode::LoadRepository
    ))
}

fn get_backup(repo: &Repository, backup_name: &str) -> Result<Backup, ErrorCode> {
    if !repo.has_backup(backup_name) {
        tr_error!("A backup with that name does not exist");
        return Err(ErrorCode::NoSuchBackup);
    }
    Ok(checked!(
        repo.get_backup(backup_name),
        "load backup",
        ErrorCode::LoadBackup
    ))
}

fn get_inode(repo: &mut Repository, backup: &Backup, inode: Option<&String>) -> Result<Inode, ErrorCode> {
    Ok(if let Some(inode) = inode {
        checked!(
                    repo.get_backup_inode(backup, &inode),
                    "load subpath inode",
                    ErrorCode::LoadInode
                )
    } else {
        checked!(
                    repo.get_inode(&backup.root),
                    "load root inode",
                    ErrorCode::LoadInode
                )
    })
}

fn find_reference_backup(
    repo: &Repository,
    path: &str,
) -> Result<Option<(String, Backup)>, ErrorCode> {
    let mut matching = Vec::new();
    let hostname = match get_hostname() {
        Ok(hostname) => hostname,
        Err(_) => return Ok(None),
    };
    let backup_map = match repo.get_all_backups() {
        Ok(backup_map) => backup_map,
        Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map,
                                                                            _failed))) => {
            tr_warn!("Some backups could not be read, ignoring them");
            backup_map
        }
        Err(err) => {
            tr_error!("Failed to load backup files: {}", err);
            return Err(ErrorCode::LoadBackup);
        }
    };
    for (name, backup) in backup_map {
        if backup.host == hostname && backup.path == path {
            matching.push((name, backup));
        }
    }
    matching.sort_by_key(|&(_, ref b)| b.timestamp);
    Ok(matching.pop())
}

fn print_backup(backup: &Backup) {
    if backup.modified {
        tr_warn!("This backup has been modified");
    }
    tr_println!(
        "Date: {}",
        Local.timestamp(backup.timestamp, 0).to_rfc2822()
    );
    tr_println!("Source: {}:{}", backup.host, backup.path);
    tr_println!("Duration: {}", to_duration(backup.duration));
    tr_println!(
        "Entries: {} files, {} dirs",
        backup.file_count,
        backup.dir_count
    );
    tr_println!(
        "Total backup size: {}",
        to_file_size(backup.total_data_size)
    );
    tr_println!(
        "Modified data size: {}",
        to_file_size(backup.changed_data_size)
    );
    let dedup_ratio = backup.deduplicated_data_size as f32 / backup.changed_data_size as f32;
    tr_println!(
        "Deduplicated size: {}, {:.1}%",
        to_file_size(backup.deduplicated_data_size),
        (dedup_ratio - 1.0) * 100.0
    );
    let compress_ratio = backup.encoded_data_size as f32 / backup.deduplicated_data_size as f32;
    tr_println!(
        "Compressed size: {} in {} bundles, {:.1}%",
        to_file_size(backup.encoded_data_size),
        backup.bundle_count,
        (compress_ratio - 1.0) * 100.0
    );
    tr_println!(
        "Chunk count: {}, avg size: {}",
        backup.chunk_count,
        to_file_size(backup.avg_chunk_size as u64)
    );
}

pub fn format_inode_one_line(inode: &Inode) -> String {
    match inode.file_type {
        FileType::Directory => {
            format!(
                "{:25}\t{} entries",
                format!("{}/", inode.name),
                inode.children.as_ref().map(|c| c.len()).unwrap_or(0)
            )
        }
        FileType::File => {
            format!(
                "{:25}\t{:>10}\t{}",
                inode.name,
                to_file_size(inode.size),
                Local.timestamp(inode.timestamp, 0).to_rfc2822()
            )
        }
        FileType::Symlink => {
            format!(
                "{:25}\t -> {}",
                inode.name,
                inode.symlink_target.as_ref().map(|s| s as &str).unwrap_or(
                    "?"
                )
            )
        }
        FileType::BlockDevice | FileType::CharDevice => {
            let device = inode.device.unwrap_or((0, 0));
            format!(
                "{:25}\t{:12}\t{}:{}",
                inode.name,
                inode.file_type,
                device.0,
                device.1
            )
        }
        FileType::NamedPipe => format!("{:25}\t fifo", inode.name),
    }
}

fn print_inode(inode: &Inode) {
    tr_println!("Name: {}", inode.name);
    tr_println!("Type: {}", inode.file_type);
    tr_println!("Size: {}", to_file_size(inode.size));
    tr_println!("Permissions: {:3o}", inode.mode);
    tr_println!("User: {}", inode.user);
    tr_println!("Group: {}", inode.group);
    tr_println!(
        "Timestamp: {}",
        Local.timestamp(inode.timestamp, 0).to_rfc2822()
    );
    if let Some(ref target) = inode.symlink_target {
        tr_println!("Symlink target: {}", target);
    }
    tr_println!("Cumulative size: {}", to_file_size(inode.cum_size));
    tr_println!("Cumulative file count: {}", inode.cum_files);
    tr_println!("Cumulative directory count: {}", inode.cum_dirs);
    if let Some(ref children) = inode.children {
        tr_println!("Children:");
        for name in children.keys() {
            println!("  - {}", name);
        }
    }
    if !inode.xattrs.is_empty() {
        tr_println!("Extended attributes:");
        for (key, value) in &inode.xattrs {
            if let Ok(value) = str::from_utf8(value) {
                println!("  - {} = '{}'", key, value);
            } else {
                println!("  - {} = 0x{}", key, to_hex(value));
            }
        }
    }
}

fn print_backups(backup_map: &HashMap<String, Backup>) {
    let mut backups: Vec<_> = backup_map.into_iter().collect();
    backups.sort_by_key(|b| b.0);
    for (name, backup) in backups {
        println!(
            "{:40}  {:>32}  {:7} files, {:6} dirs, {:>10}",
            name,
            Local.timestamp(backup.timestamp, 0).to_rfc2822(),
            backup.file_count,
            backup.dir_count,
            to_file_size(backup.total_data_size)
        );
    }
}

fn print_repoinfo(info: &RepositoryInfo) {
    tr_println!("Bundles: {}", info.bundle_count);
    tr_println!("Total size: {}", to_file_size(info.encoded_data_size));
    tr_println!("Uncompressed size: {}", to_file_size(info.raw_data_size));
    tr_println!("Compression ratio: {:.1}%", (info.compression_ratio - 1.0) * 100.0);
    tr_println!("Chunk count: {}", info.chunk_count);
    tr_println!(
        "Average chunk size: {}",
        to_file_size(info.avg_chunk_size as u64)
    );
    let index_usage = info.index_entries as f32 / info.index_capacity as f32;
    tr_println!(
        "Index: {}, {:.0}% full",
        to_file_size(info.index_size as u64),
        index_usage * 100.0
    );
}

fn print_repostats(stats: &RepositoryStatistics) {
    tr_println!("Index\n=====");
    let index_usage = stats.index.count as f32 / stats.index.capacity as f32;
    tr_println!("Size: {}", to_file_size(stats.index.size as u64));
    tr_println!("Entries: {} / {}, {:.0}%", stats.index.count, stats.index.capacity, index_usage*100.0);
    let disp = &stats.index.displacement;
    tr_println!("Displacement:\n  - average: {:.1}\n  - stddev: {:.1}\n  - over {:.1}: {:.0}, {:.1}%\n  - maximum: {:.0}",
        disp.avg, disp.stddev, disp.avg + 2.0 * disp.stddev, disp.count_xl, disp.count_xl as f32 / disp.count as f32 * 100.0, disp.max);
    println!();
    tr_println!("Bundles\n=======");
    let tsize = (stats.bundles.raw_size.count as f32 * stats.bundles.encoded_size.avg) as u64;
    tr_println!("All bundles: {} in {} bundles", to_file_size(tsize), stats.bundles.raw_size.count);
    let rsize = &stats.bundles.raw_size;
    tr_println!("  - raw size: ø = {}, maximum: {}", to_file_size(rsize.avg as u64), to_file_size(rsize.max as u64));
    let esize = &stats.bundles.encoded_size;
    tr_println!("  - encoded size: ø = {}, maximum: {}", to_file_size(esize.avg as u64), to_file_size(esize.max as u64));
    let ccount = &stats.bundles.chunk_count;
    tr_println!("  - chunk count: ø = {:.1}, maximum: {:.0}", ccount.avg, ccount.max);
    let tsize = (stats.bundles.raw_size_meta.count as f32 * stats.bundles.encoded_size_meta.avg) as u64;
    tr_println!("Meta bundles: {} in {} bundles", to_file_size(tsize), stats.bundles.raw_size_meta.count);
    let rsize = &stats.bundles.raw_size_meta;
    tr_println!("  - raw size: ø = {}, maximum: {}", to_file_size(rsize.avg as u64), to_file_size(rsize.max as u64));
    let esize = &stats.bundles.encoded_size_meta;
    tr_println!("  - encoded size: ø = {}, maximum: {}", to_file_size(esize.avg as u64), to_file_size(esize.max as u64));
    let ccount = &stats.bundles.chunk_count_meta;
    tr_println!("  - chunk count: ø = {:.1}, maximum: {:.0}", ccount.avg, ccount.max);
    let tsize = (stats.bundles.raw_size_data.count as f32 * stats.bundles.encoded_size_data.avg) as u64;
    tr_println!("Data bundles: {} in {} bundles", to_file_size(tsize), stats.bundles.raw_size_data.count);
    let rsize = &stats.bundles.raw_size_data;
    tr_println!("  - raw size: ø = {}, maximum: {}", to_file_size(rsize.avg as u64), to_file_size(rsize.max as u64));
    let esize = &stats.bundles.encoded_size_data;
    tr_println!("  - encoded size: ø = {}, maximum: {}", to_file_size(esize.avg as u64), to_file_size(esize.max as u64));
    let ccount = &stats.bundles.chunk_count_data;
    tr_println!("  - chunk count: ø = {:.1}, maximum: {:.0}", ccount.avg, ccount.max);
    println!();
    tr_println!("Bundle methods\n==============");
    tr_println!("Hash:");
    for (hash, &count) in &stats.bundles.hash_methods {
        tr_println!("  - {}: {}, {:.1}%", hash.name(), count, count as f32 / stats.bundles.raw_size.count as f32 * 100.0);
    }
    tr_println!("Compression:");
    for (compr, &count) in &stats.bundles.compressions {
        let compr_name = if let Some(ref compr) = *compr {
            compr.to_string()
        } else {
            tr!("none").to_string()
        };
        tr_println!("  - {}: {}, {:.1}%", compr_name, count, count as f32 / stats.bundles.raw_size.count as f32 * 100.0);
    }
    tr_println!("Encryption:");
    for (encr, &count) in &stats.bundles.encryptions {
        let encr_name = if let Some(ref encr) = *encr {
            to_hex(&encr.1[..])
        } else {
            tr!("none").to_string()
        };
        tr_println!("  - {}: {}, {:.1}%", encr_name, count, count as f32 / stats.bundles.raw_size.count as f32 * 100.0);
    }
}

fn print_bundle(bundle: &StoredBundle) {
    tr_println!("Bundle {}", bundle.info.id);
    tr_println!("  - Mode: {:?}", bundle.info.mode);
    tr_println!("  - Path: {:?}", bundle.path);
    tr_println!(
        "  - Date: {}",
        Local.timestamp(bundle.info.timestamp, 0).to_rfc2822()
    );
    tr_println!("  - Hash method: {:?}", bundle.info.hash_method);
    let encryption = if let Some((_, ref key)) = bundle.info.encryption {
        to_hex(key)
    } else {
        "none".to_string()
    };
    tr_println!("  - Encryption: {}", encryption);
    tr_println!("  - Chunks: {}", bundle.info.chunk_count);
    tr_println!(
        "  - Size: {}",
        to_file_size(bundle.info.encoded_size as u64)
    );
    tr_println!(
        "  - Data size: {}",
        to_file_size(bundle.info.raw_size as u64)
    );
    let ratio = bundle.info.encoded_size as f32 / bundle.info.raw_size as f32;
    let compression = if let Some(ref c) = bundle.info.compression {
        c.to_string()
    } else {
        "none".to_string()
    };
    tr_println!(
        "  - Compression: {}, ratio: {:.1}%",
        compression,
        (ratio - 1.0) * 100.0
    );
}

fn print_bundle_one_line(bundle: &BundleInfo) {
    tr_println!(
        "{}: {:8?}, {:5} chunks, {:8}",
        bundle.id,
        bundle.mode,
        bundle.chunk_count,
        to_file_size(bundle.encoded_size as u64)
    )
}

fn print_config(config: &Config) {
    tr_println!("Bundle size: {}", to_file_size(config.bundle_size as u64));
    tr_println!("Chunker: {}", config.chunker.to_string());
    if let Some(ref compression) = config.compression {
        tr_println!("Compression: {}", compression.to_string());
    } else {
        tr_println!("Compression: none");
    }
    if let Some(ref encryption) = config.encryption {
        tr_println!("Encryption: {}", to_hex(&encryption.1[..]));
    } else {
        tr_println!("Encryption: none");
    }
    tr_println!("Hash method: {}", config.hash.name());
}

fn print_analysis(analysis: &HashMap<u32, BundleAnalysis>) {
    let mut reclaim_space = [0; 11];
    let mut rewrite_size = [0; 11];
    let mut data_total = 0;
    for bundle in analysis.values() {
        data_total += bundle.info.encoded_size;
        #[allow(unknown_lints, needless_range_loop)]
        for i in 0..11 {
            if bundle.get_usage_ratio() <= i as f32 * 0.1 {
                reclaim_space[i] += bundle.get_unused_size();
                rewrite_size[i] += bundle.get_used_size();
            }
        }
    }
    tr_println!("Total bundle size: {}", to_file_size(data_total as u64));
    let used = data_total - reclaim_space[10];
    tr_println!(
        "Space used: {}, {:.1} %",
        to_file_size(used as u64),
        used as f32 / data_total as f32 * 100.0
    );
    tr_println!("Reclaimable space (depending on vacuum ratio)");
    #[allow(unknown_lints, needless_range_loop)]
    for i in 0..11 {
        tr_println!(
            "  - ratio={:3}: {:>10}, {:4.1} %, rewriting {:>10}",
            i * 10,
            to_file_size(reclaim_space[i] as u64),
            reclaim_space[i] as f32 / data_total as f32 * 100.0,
            to_file_size(rewrite_size[i] as u64)
        );
    }
}

fn print_duplicates(dups: Vec<(Vec<PathBuf>, u64)>) {
    for (group, size) in dups {
        tr_println!("{} duplicates found, size: {}", group.len(), to_file_size(size));
        for dup in group {
            println!("  - {}", dup.to_string_lossy());
        }
        println!();
    }
}



#[allow(unknown_lints, cyclomatic_complexity)]
pub fn run() -> Result<(), ErrorCode> {
    let (log_level, args) = try!(args::parse());
    if let Err(err) = logger::init(log_level) {
        tr_println!("Failed to initialize the logger: {}", err);
        return Err(ErrorCode::InitializeLogger);
    }
    match args {
        Arguments::Init {
            repo_path,
            bundle_size,
            chunker,
            compression,
            encryption,
            hash,
            remote_path
        } => {
            if !Path::new(&remote_path).is_absolute() {
                tr_error!("The remote path of a repository must be absolute.");
                return Err(ErrorCode::InvalidArgs);
            }
            let mut repo = checked!(
                Repository::create(
                    repo_path,
                    &Config {
                        bundle_size,
                        chunker,
                        compression,
                        encryption: None,
                        hash
                    },
                    remote_path
                ),
                "create repository",
                ErrorCode::CreateRepository
            );
            if encryption {
                let (public, secret) = Crypto::gen_keypair();
                tr_info!("Created the following key pair");
                tr_println!("public: {}", to_hex(&public[..]));
                tr_println!("secret: {}", to_hex(&secret[..]));
                repo.set_encryption(Some(&public));
                checked!(
                    repo.register_key(public, secret),
                    "add key",
                    ErrorCode::AddKey
                );
                checked!(repo.save_config(), "save config", ErrorCode::SaveConfig);
                tr_warn!(
                    "Please store this key pair in a secure location before using the repository"
                );
                println!();
            }
            print_config(&repo.config);
        }
        Arguments::Backup {
            repo_path,
            backup_name,
            src_path,
            full,
            reference,
            same_device,
            mut excludes,
            excludes_from,
            no_default_excludes,
            tar
        } => {
            let mut repo = try!(open_repository(&repo_path, true));
            if repo.has_backup(&backup_name) {
                tr_error!("A backup with that name already exists");
                return Err(ErrorCode::BackupAlreadyExists);
            }
            if src_path == "-" && !tar {
                tr_error!("Reading from stdin requires --tar");
                return Err(ErrorCode::InvalidArgs);
            }
            let mut reference_backup = None;
            if !full && !tar {
                reference_backup = match reference {
                    Some(r) => {
                        let b = try!(get_backup(&repo, &r));
                        Some((r, b))
                    }
                    None => None,
                };
                if reference_backup.is_none() {
                    reference_backup = try!(find_reference_backup(&repo, &src_path));
                }
                if let Some(&(ref name, _)) = reference_backup.as_ref() {
                    tr_info!("Using backup {} as reference", name);
                } else {
                    tr_info!("No reference backup found, doing a full scan instead");
                }
            }
            let reference_backup = reference_backup.map(|(_, backup)| backup);
            if !no_default_excludes && !tar {
                for line in BufReader::new(checked!(
                    File::open(&repo.layout.excludes_path()),
                    "open default excludes file",
                    ErrorCode::LoadExcludes
                )).lines()
                {
                    excludes.push(checked!(
                        line,
                        "read default excludes file",
                        ErrorCode::LoadExcludes
                    ));
                }
            }
            if let Some(excludes_from) = excludes_from {
                for line in BufReader::new(checked!(
                    File::open(excludes_from),
                    "open excludes file",
                    ErrorCode::LoadExcludes
                )).lines()
                {
                    excludes.push(checked!(
                        line,
                        "read excludes file",
                        ErrorCode::LoadExcludes
                    ));
                }
            }
            let mut excludes_parsed = Vec::with_capacity(excludes.len());
            for mut exclude in excludes {
                if exclude.starts_with('#') || exclude.is_empty() {
                    continue;
                }
                exclude = regex::escape(&exclude)
                    .replace('?', ".")
                    .replace(r"\*\*", ".*")
                    .replace(r"\*", "[^/]*");
                excludes_parsed.push(if exclude.starts_with('/') {
                    format!(r"^{}($|/)", exclude)
                } else {
                    format!(r"/{}($|/)", exclude)
                });
            }
            let excludes = if excludes_parsed.is_empty() {
                None
            } else {
                Some(checked!(
                    RegexSet::new(excludes_parsed),
                    "parse exclude patterns",
                    ErrorCode::InvalidExcludes
                ))
            };
            let options = BackupOptions {
                same_device,
                excludes
            };
            let result = if tar {
                repo.import_tarfile(&src_path)
            } else {
                repo.create_backup_recursively(&src_path, reference_backup.as_ref(), &options)
            };
            let backup = match result {
                Ok(backup) => {
                    tr_info!("Backup finished");
                    backup
                }
                Err(RepositoryError::Backup(BackupError::FailedPaths(backup, _failed_paths))) => {
                    tr_warn!("Some files are missing from the backup");
                    backup
                }
                Err(err) => {
                    tr_error!("Backup failed: {}", err);
                    return Err(ErrorCode::BackupRun);
                }
            };
            checked!(
                repo.save_backup(&backup, &backup_name),
                "save backup file",
                ErrorCode::SaveBackup
            );
            print_backup(&backup);
        }
        Arguments::Restore {
            repo_path,
            backup_name,
            inode,
            dst_path,
            tar
        } => {
            let mut repo = try!(open_repository(&repo_path, true));
            let backup = try!(get_backup(&repo, &backup_name));
            let inode = try!(get_inode(&mut repo, &backup, inode.as_ref()));
            if tar {
                checked!(
                    repo.export_tarfile(&backup, inode, &dst_path),
                    "restore backup",
                    ErrorCode::RestoreRun
                );
            } else {
                checked!(
                    repo.restore_inode_tree(&backup, inode, &dst_path),
                    "restore backup",
                    ErrorCode::RestoreRun
                );
            }
            tr_info!("Restore finished");
        }
        Arguments::Copy {
            repo_path_src,
            backup_name_src,
            repo_path_dst,
            backup_name_dst
        } => {
            if repo_path_src != repo_path_dst {
                tr_error!("Can only run copy on same repository");
                return Err(ErrorCode::InvalidArgs);
            }
            let mut repo = try!(open_repository(&repo_path_src, false));
            if repo.has_backup(&backup_name_dst) {
                tr_error!("A backup with that name already exists");
                return Err(ErrorCode::BackupAlreadyExists);
            }
            let backup = try!(get_backup(&repo, &backup_name_src));
            checked!(
                repo.save_backup(&backup, &backup_name_dst),
                "save backup file",
                ErrorCode::SaveBackup
            );
        }
        Arguments::Remove {
            repo_path,
            backup_name,
            inode,
            force
        } => {
            let mut repo = try!(open_repository(&repo_path, true));
            if let Some(inode) = inode {
                let mut backup = try!(get_backup(&repo, &backup_name));
                checked!(
                    repo.remove_backup_path(&mut backup, inode),
                    "remove backup subpath",
                    ErrorCode::RemoveRun
                );
                checked!(
                    repo.save_backup(&backup, &backup_name),
                    "save backup file",
                    ErrorCode::SaveBackup
                );
                tr_info!("The backup subpath has been deleted, run vacuum to reclaim space");
            } else if repo.layout.backups_path().join(&backup_name).is_dir() {
                let backups = checked!(
                    repo.get_backups(&backup_name),
                    "retrieve backups",
                    ErrorCode::RemoveRun
                );
                if force {
                    for name in backups.keys() {
                        checked!(
                            repo.delete_backup(&format!("{}/{}", &backup_name, name)),
                            "delete backup",
                            ErrorCode::RemoveRun
                        );
                    }
                } else {
                    tr_error!("Denying to remove multiple backups (use --force):");
                    for name in backups.keys() {
                        println!("  - {}/{}", backup_name, name);
                    }
                }
            } else {
                checked!(
                    repo.delete_backup(&backup_name),
                    "delete backup",
                    ErrorCode::RemoveRun
                );
                tr_info!("The backup has been deleted, run vacuum to reclaim space");
            }
        }
        Arguments::Prune {
            repo_path,
            prefix,
            daily,
            weekly,
            monthly,
            yearly,
            force
        } => {
            let mut repo = try!(open_repository(&repo_path, true));
            if daily + weekly + monthly + yearly == 0 {
                tr_error!("This would remove all those backups");
                return Err(ErrorCode::UnsafeArgs);
            }
            checked!(
                repo.prune_backups(&prefix, daily, weekly, monthly, yearly, force),
                "prune backups",
                ErrorCode::PruneRun
            );
            if !force {
                tr_info!("Run with --force to actually execute this command");
            }
        }
        Arguments::Vacuum {
            repo_path,
            ratio,
            force,
            combine
        } => {
            let mut repo = try!(open_repository(&repo_path, true));
            let info_before = repo.info();
            checked!(
                repo.vacuum(ratio, combine, force),
                "vacuum",
                ErrorCode::VacuumRun
            );
            if !force {
                tr_info!("Run with --force to actually execute this command");
            } else {
                let info_after = repo.info();
                tr_info!(
                    "Reclaimed {}",
                    to_file_size(info_before.encoded_data_size - info_after.encoded_data_size)
                );
            }
        }
        Arguments::Check {
            repo_path,
            backup_name,
            inode,
            bundles,
            index,
            bundle_data,
            repair
        } => {
            let mut repo = try!(open_repository(&repo_path, true));
            checked!(
                repo.check_repository(repair),
                "check repository",
                ErrorCode::CheckRun
            );
            if bundles {
                checked!(
                    repo.check_bundles(bundle_data, repair),
                    "check bundles",
                    ErrorCode::CheckRun
                );
            }
            if index {
                checked!(repo.check_index(repair), "check index", ErrorCode::CheckRun);
            }
            if let Some(backup_name) = backup_name {
                let mut backup = try!(get_backup(&repo, &backup_name));
                if let Some(path) = inode {
                    checked!(
                        repo.check_backup_inode(&backup_name, &mut backup, Path::new(&path), repair),
                        "check inode",
                        ErrorCode::CheckRun
                    )
                } else {
                    checked!(
                        repo.check_backup(&backup_name, &mut backup, repair),
                        "check backup",
                        ErrorCode::CheckRun
                    )
                }
            } else {
                checked!(
                    repo.check_backups(repair),
                    "check repository",
                    ErrorCode::CheckRun
                )
            }
            repo.set_clean();
            tr_info!("Integrity verified")
        }
        Arguments::List {
            repo_path,
            backup_name,
            inode
        } => {
            let mut repo = try!(open_repository(&repo_path, false));
            let backup_map = if let Some(backup_name) = backup_name {
                if repo.layout.backups_path().join(&backup_name).is_dir() {
                    repo.get_backups(&backup_name)
                } else {
                    let backup = try!(get_backup(&repo, &backup_name));
                    let inode = checked!(
                        repo.get_backup_inode(
                            &backup,
                            inode.as_ref().map(|v| v as &str).unwrap_or("/")
                        ),
                        "load subpath inode",
                        ErrorCode::LoadInode
                    );
                    println!("{}", format_inode_one_line(&inode));
                    if let Some(children) = inode.children {
                        for chunks in children.values() {
                            let inode = checked!(
                                repo.get_inode(chunks),
                                "load child inode",
                                ErrorCode::LoadInode
                            );
                            println!("- {}", format_inode_one_line(&inode));
                        }
                    }
                    return Ok(());
                }
            } else {
                repo.get_all_backups()
            };
            let backup_map = match backup_map {
                Ok(backup_map) => backup_map,
                Err(RepositoryError::BackupFile(BackupFileError::PartialBackupsList(backup_map, _failed))) => {
                    tr_warn!("Some backups could not be read, ignoring them");
                    backup_map
                }
                Err(err) => {
                    tr_error!("Failed to load backup files: {}", err);
                    return Err(ErrorCode::LoadBackup);
                }
            };
            print_backups(&backup_map);
        }
        Arguments::Info {
            repo_path,
            backup_name,
            inode
        } => {
            let mut repo = try!(open_repository(&repo_path, false));
            if let Some(backup_name) = backup_name {
                let backup = try!(get_backup(&repo, &backup_name));
                if let Some(inode) = inode {
                    let inode = checked!(
                        repo.get_backup_inode(&backup, inode),
                        "load subpath inode",
                        ErrorCode::LoadInode
                    );
                    print_inode(&inode);
                } else {
                    print_backup(&backup);
                }
            } else {
                print_repoinfo(&repo.info());
            }
        }
        Arguments::Statistics {
            repo_path
        } => {
            let mut repo = try!(open_repository(&repo_path, false));
            print_repostats(&repo.statistics());
        }
        Arguments::Duplicates {
            repo_path,
            backup_name,
            inode,
            min_size
        } => {
            let mut repo = try!(open_repository(&repo_path, true));
            let backup = try!(get_backup(&repo, &backup_name));
            let inode = try!(get_inode(&mut repo, &backup, inode.as_ref()));
            let dups = checked!(
                repo.find_duplicates(&inode, min_size),
                "find duplicates",
                ErrorCode::DuplicatesRun
            );
            print_duplicates(dups);
        }
        Arguments::Mount {
            repo_path,
            backup_name,
            inode,
            mount_point
        } => {
            let mut repo = try!(open_repository(&repo_path, true));
            let fs = if let Some(backup_name) = backup_name {
                if repo.layout.backups_path().join(&backup_name).is_dir() {
                    checked!(
                        FuseFilesystem::from_repository(&mut repo, Some(&backup_name)),
                        "create fuse filesystem",
                        ErrorCode::FuseMount
                    )
                } else {
                    let backup = try!(get_backup(&repo, &backup_name));
                    if let Some(inode) = inode {
                        let inode = checked!(
                            repo.get_backup_inode(&backup, inode),
                            "load subpath inode",
                            ErrorCode::LoadInode
                        );
                        checked!(
                            FuseFilesystem::from_inode(&mut repo, backup, inode),
                            "create fuse filesystem",
                            ErrorCode::FuseMount
                        )
                    } else {
                        checked!(
                            FuseFilesystem::from_backup(&mut repo, backup),
                            "create fuse filesystem",
                            ErrorCode::FuseMount
                        )
                    }
                }
            } else {
                checked!(
                    FuseFilesystem::from_repository(&mut repo, None),
                    "create fuse filesystem",
                    ErrorCode::FuseMount
                )
            };
            tr_info!("Mounting the filesystem...");
            tr_info!(
                "Please unmount the filesystem via 'fusermount -u {}' when done.",
                mount_point
            );
            checked!(
                fs.mount(&mount_point),
                "mount filesystem",
                ErrorCode::FuseMount
            );
        }
        Arguments::Analyze { repo_path } => {
            let mut repo = try!(open_repository(&repo_path, true));
            print_analysis(&checked!(
                repo.analyze_usage(),
                "analyze repository",
                ErrorCode::AnalyzeRun
            ));
        }
        Arguments::BundleList { repo_path } => {
            let repo = try!(open_repository(&repo_path, true));
            for bundle in repo.list_bundles() {
                print_bundle_one_line(bundle);
            }
        }
        Arguments::BundleInfo {
            repo_path,
            bundle_id
        } => {
            let repo = try!(open_repository(&repo_path, true));
            if let Some(bundle) = repo.get_bundle(&bundle_id) {
                print_bundle(bundle);
            } else {
                tr_error!("No such bundle");
                return Err(ErrorCode::LoadBundle);
            }
        }
        Arguments::Import {
            repo_path,
            remote_path,
            key_files
        } => {
            checked!(
                Repository::import(repo_path, remote_path, key_files),
                "import repository",
                ErrorCode::ImportRun
            );
            tr_info!("Import finished");
        }
        Arguments::Versions { repo_path, path } => {
            let mut repo = try!(open_repository(&repo_path, true));
            let mut found = false;
            for (name, mut inode) in
                checked!(
                    repo.find_versions(&path),
                    "find versions",
                    ErrorCode::VersionsRun
                )
            {
                inode.name = format!("{}::{}", name, &path);
                println!("{}", format_inode_one_line(&inode));
                found = true;
            }
            if !found {
                tr_info!("No versions of that file were found.");
            }
        }
        Arguments::Diff {
            repo_path_old,
            backup_name_old,
            inode_old,
            repo_path_new,
            backup_name_new,
            inode_new
        } => {
            if repo_path_old != repo_path_new {
                tr_error!("Can only run diff on same repository");
                return Err(ErrorCode::InvalidArgs);
            }
            let mut repo = try!(open_repository(&repo_path_old, true));
            let backup_old = try!(get_backup(&repo, &backup_name_old));
            let backup_new = try!(get_backup(&repo, &backup_name_new));
            let inode1 =
                checked!(
                    repo.get_backup_inode(&backup_old, inode_old.unwrap_or_else(|| "/".to_string())),
                    "load subpath inode",
                    ErrorCode::LoadInode
                );
            let inode2 =
                checked!(
                    repo.get_backup_inode(&backup_new, inode_new.unwrap_or_else(|| "/".to_string())),
                    "load subpath inode",
                    ErrorCode::LoadInode
                );
            let diffs = checked!(
                repo.find_differences(&inode1, &inode2),
                "find differences",
                ErrorCode::DiffRun
            );
            for diff in &diffs {
                println!(
                    "{} {:?}",
                    match diff.0 {
                        DiffType::Add => "add",
                        DiffType::Mod => "mod",
                        DiffType::Del => "del",
                    },
                    diff.1
                );
            }
            if diffs.is_empty() {
                tr_info!("No differences found");
            }
        }
        Arguments::Config {
            repo_path,
            bundle_size,
            chunker,
            compression,
            encryption,
            hash
        } => {
            let mut repo = try!(open_repository(&repo_path, false));
            let mut changed = false;
            if let Some(bundle_size) = bundle_size {
                repo.config.bundle_size = bundle_size;
                changed = true;
            }
            if let Some(chunker) = chunker {
                tr_warn!(
                    "Changing the chunker makes it impossible to use existing data for deduplication"
                );
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
                tr_warn!(
                    "Changing the hash makes it impossible to use existing data for deduplication"
                );
                repo.config.hash = hash;
                changed = true;
            }
            if changed {
                checked!(repo.save_config(), "save config", ErrorCode::SaveConfig);
                tr_info!("The configuration has been updated.");
            } else {
                print_config(&repo.config);
            }
        }
        Arguments::GenKey { file, password } => {
            let (public, secret) = match password {
                None => Crypto::gen_keypair(),
                Some(ref password) => Crypto::keypair_from_password(password),
            };
            tr_info!("Created the following key pair");
            tr_println!("public: {}", to_hex(&public[..]));
            tr_println!("secret: {}", to_hex(&secret[..]));
            if let Some(file) = file {
                checked!(
                    Crypto::save_keypair_to_file(&public, &secret, file),
                    "save key pair",
                    ErrorCode::SaveKey
                );
            }
        }
        Arguments::AddKey {
            repo_path,
            set_default,
            password,
            file
        } => {
            let mut repo = try!(open_repository(&repo_path, false));
            let (public, secret) = if let Some(file) = file {
                checked!(
                    Crypto::load_keypair_from_file(file),
                    "load key pair",
                    ErrorCode::LoadKey
                )
            } else {
                tr_info!("Created the following key pair");
                let (public, secret) = match password {
                    None => Crypto::gen_keypair(),
                    Some(ref password) => Crypto::keypair_from_password(password),
                };
                tr_println!("public: {}", to_hex(&public[..]));
                tr_println!("secret: {}", to_hex(&secret[..]));
                (public, secret)
            };
            checked!(
                repo.register_key(public, secret),
                "add key pair",
                ErrorCode::AddKey
            );
            if set_default {
                repo.set_encryption(Some(&public));
                checked!(repo.save_config(), "save config", ErrorCode::SaveConfig);
                tr_warn!(
                    "Please store this key pair in a secure location before using the repository"
                );
            }
        }
        Arguments::AlgoTest {
            bundle_size,
            chunker,
            compression,
            encrypt,
            hash,
            file
        } => {
            algotest::run(&file, bundle_size, chunker, compression, encrypt, hash);
        }
    }
    Ok(())
}
