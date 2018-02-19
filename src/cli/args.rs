use prelude::*;
use super::*;

use std::path::{Path, PathBuf};
use log::LogLevel;
use clap::{App, AppSettings, Arg, SubCommand};

#[allow(option_option)]
pub enum Arguments {
    Init {
        repo_path: PathBuf,
        bundle_size: usize,
        chunker: ChunkerType,
        compression: Option<Compression>,
        encryption: bool,
        hash: HashMethod,
        remote_path: String
    },
    Backup {
        repo_path: PathBuf,
        backup_name: String,
        src_path: String,
        full: bool,
        reference: Option<String>,
        same_device: bool,
        excludes: Vec<String>,
        excludes_from: Option<String>,
        no_default_excludes: bool,
        tar: bool
    },
    Restore {
        repo_path: PathBuf,
        backup_name: String,
        inode: Option<String>,
        dst_path: String,
        tar: bool
    },
    Remove {
        repo_path: PathBuf,
        backup_name: String,
        inode: Option<String>,
        force: bool
    },
    Prune {
        repo_path: PathBuf,
        prefix: String,
        daily: usize,
        weekly: usize,
        monthly: usize,
        yearly: usize,
        force: bool
    },
    Vacuum {
        repo_path: PathBuf,
        ratio: f32,
        force: bool,
        combine: bool
    },
    Check {
        repo_path: PathBuf,
        backup_name: Option<String>,
        inode: Option<String>,
        bundles: bool,
        bundle_data: bool,
        index: bool,
        repair: bool
    },
    List {
        repo_path: PathBuf,
        backup_name: Option<String>,
        inode: Option<String>
    },
    Info {
        repo_path: PathBuf,
        backup_name: Option<String>,
        inode: Option<String>
    },
    Copy {
        repo_path_src: PathBuf,
        backup_name_src: String,
        repo_path_dst: PathBuf,
        backup_name_dst: String
    },
    Mount {
        repo_path: PathBuf,
        backup_name: Option<String>,
        inode: Option<String>,
        mount_point: String
    },
    Versions { repo_path: PathBuf, path: String },
    Diff {
        repo_path_old: PathBuf,
        backup_name_old: String,
        inode_old: Option<String>,
        repo_path_new: PathBuf,
        backup_name_new: String,
        inode_new: Option<String>
    },
    Analyze { repo_path: PathBuf },
    BundleList { repo_path: PathBuf },
    BundleInfo {
        repo_path: PathBuf,
        bundle_id: BundleId
    },
    Import {
        repo_path: PathBuf,
        remote_path: String,
        key_files: Vec<String>
    },
    Config {
        repo_path: PathBuf,
        bundle_size: Option<usize>,
        chunker: Option<ChunkerType>,
        compression: Option<Option<Compression>>,
        encryption: Option<Option<PublicKey>>,
        hash: Option<HashMethod>
    },
    GenKey {
        file: Option<String>,
        password: Option<String>
    },
    AddKey {
        repo_path: PathBuf,
        file: Option<String>,
        password: Option<String>,
        set_default: bool
    },
    AlgoTest {
        file: String,
        bundle_size: usize,
        chunker: ChunkerType,
        compression: Option<Compression>,
        encrypt: bool,
        hash: HashMethod
    }
}


fn convert_repo_path(mut path_str: &str) -> PathBuf {
    if path_str.is_empty() {
        path_str = "default";
    }
    let path = Path::new(path_str);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        ZVAULT_FOLDER.join("repos").join(path)
    }
}

fn parse_repo_path(
    repo_path: &str,
    existing: bool,
    backup_restr: Option<bool>,
    path_restr: Option<bool>,
) -> Result<(PathBuf, Option<&str>, Option<&str>), String> {
    let mut parts = repo_path.splitn(3, "::");
    let repo = convert_repo_path(parts.next().unwrap_or(""));
    if existing && !repo.join("config.yaml").exists() {
        return Err("The specified repository does not exist".to_string());
    }
    if !existing && repo.exists() {
        return Err("The specified repository already exists".to_string());
    }
    let mut backup = parts.next();
    if let Some(val) = backup {
        if val.is_empty() {
            backup = None
        }
    }
    let mut path = parts.next();
    if let Some(val) = path {
        if val.is_empty() {
            path = None
        }
    }
    if let Some(restr) = backup_restr {
        if !restr && backup.is_some() {
            return Err("No backup may be given here".to_string());
        }
        if restr && backup.is_none() {
            return Err("A backup must be specified".to_string());
        }
    }
    if let Some(restr) = path_restr {
        if !restr && path.is_some() {
            return Err("No subpath may be given here".to_string());
        }
        if restr && path.is_none() {
            return Err("A subpath must be specified".to_string());
        }
    }
    Ok((repo, backup, path))
}

#[allow(unknown_lints, needless_pass_by_value)]
fn validate_repo_path(
    repo_path: String,
    existing: bool,
    backup_restr: Option<bool>,
    path_restr: Option<bool>,
) -> Result<(), String> {
    parse_repo_path(&repo_path, existing, backup_restr, path_restr).map(|_| ())
}

fn parse_num(num: &str) -> Result<u64, String> {
    if let Ok(num) = num.parse::<u64>() {
        Ok(num)
    } else {
        Err("Must be a number".to_string())
    }
}

#[allow(unknown_lints, needless_pass_by_value)]
fn validate_num(val: String) -> Result<(), String> {
    parse_num(&val).map(|_| ())
}

fn parse_chunker(val: &str) -> Result<ChunkerType, String> {
    if let Ok(chunker) = ChunkerType::from_string(val) {
        Ok(chunker)
    } else {
        Err("Invalid chunker method/size".to_string())
    }
}

#[allow(unknown_lints, needless_pass_by_value)]
fn validate_chunker(val: String) -> Result<(), String> {
    parse_chunker(&val).map(|_| ())
}

fn parse_compression(val: &str) -> Result<Option<Compression>, String> {
    if val == "none" {
        return Ok(None);
    }
    if let Ok(compression) = Compression::from_string(val) {
        Ok(Some(compression))
    } else {
        Err("Invalid compression method/level".to_string())
    }
}

#[allow(unknown_lints, needless_pass_by_value)]
fn validate_compression(val: String) -> Result<(), String> {
    parse_compression(&val).map(|_| ())
}

fn parse_public_key(val: &str) -> Result<Option<PublicKey>, String> {
    if val.to_lowercase() == "none" {
        return Ok(None);
    }
    let bytes = match parse_hex(val) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err("Invalid hexadecimal".to_string());
        }
    };
    if let Some(key) = PublicKey::from_slice(&bytes) {
        Ok(Some(key))
    } else {
        return Err("Invalid key".to_string());
    }
}

#[allow(unknown_lints, needless_pass_by_value)]
fn validate_public_key(val: String) -> Result<(), String> {
    parse_public_key(&val).map(|_| ())
}

fn parse_hash(val: &str) -> Result<HashMethod, String> {
    if let Ok(hash) = HashMethod::from(val) {
        Ok(hash)
    } else {
        Err("Invalid hash method".to_string())
    }
}

#[allow(unknown_lints, needless_pass_by_value)]
fn validate_hash(val: String) -> Result<(), String> {
    parse_hash(&val).map(|_| ())
}

fn parse_bundle_id(val: &str) -> Result<BundleId, ErrorCode> {
    if let Ok(hash) = Hash::from_string(val) {
        Ok(BundleId(hash))
    } else {
        error!("Invalid bundle id: {}", val);
        Err(ErrorCode::InvalidArgs)
    }
}

#[allow(unknown_lints, needless_pass_by_value)]
fn validate_existing_path(val: String) -> Result<(), String> {
    if !Path::new(&val).exists() {
        Err("Path does not exist".to_string())
    } else {
        Ok(())
    }
}

#[allow(unknown_lints, needless_pass_by_value)]
fn validate_existing_path_or_stdio(val: String) -> Result<(), String> {
    if val != "-" && !Path::new(&val).exists() {
        Err("Path does not exist".to_string())
    } else {
        Ok(())
    }
}


#[allow(unknown_lints, cyclomatic_complexity)]
pub fn parse() -> Result<(LogLevel, Arguments), ErrorCode> {
    let args = App::new("zvault").version(crate_version!()).author(crate_authors!(",\n")).about(crate_description!())
        .settings(&[AppSettings::VersionlessSubcommands, AppSettings::SubcommandRequiredElseHelp])
        .global_settings(&[AppSettings::AllowMissingPositional, AppSettings::UnifiedHelpMessage, AppSettings::ColoredHelp, AppSettings::ColorAuto])
        .arg(Arg::from_usage("-v --verbose 'Print more information'").global(true).multiple(true).max_values(3).takes_value(false))
        .arg(Arg::from_usage("-q --quiet 'Print less information'").global(true).conflicts_with("verbose"))
        .subcommand(SubCommand::with_name("init").about("Initialize a new repository")
            .arg(Arg::from_usage("[bundle_size] --bundle-size [SIZE] 'Set the target bundle size in MiB'")
                .default_value(DEFAULT_BUNDLE_SIZE_STR).validator(validate_num))
            .arg(Arg::from_usage("--chunker [CHUNKER] 'Set the chunker algorithm and target chunk size'")
                .default_value(DEFAULT_CHUNKER).validator(validate_chunker))
            .arg(Arg::from_usage("-c --compression [COMPRESSION] 'Set the compression method and level'")
                .default_value(DEFAULT_COMPRESSION).validator(validate_compression))
            .arg(Arg::from_usage("-e --encrypt 'Generate a keypair and enable encryption'"))
            .arg(Arg::from_usage("--hash [HASH] 'Set the hash method'")
                .default_value(DEFAULT_HASH).validator(validate_hash))
            .arg(Arg::from_usage("-r --remote <REMOTE> 'Set the path to the mounted remote storage'")
                .validator(validate_existing_path))
            .arg(Arg::from_usage("<REPO> 'The path for the new repository'")
                .validator(|val| validate_repo_path(val, false, Some(false), Some(false)))))
        .subcommand(SubCommand::with_name("backup").about("Create a new backup")
            .arg(Arg::from_usage("--full 'Create a full backup without using a reference'"))
            .arg(Arg::from_usage("[reference] --ref [REF] 'Base the new backup on this reference'")
                .conflicts_with("full"))
            .arg(Arg::from_usage("[cross_device] -x --xdev 'Allow to cross filesystem boundaries'"))
            .arg(Arg::from_usage("-e --exclude [PATTERN]... 'Exclude this path or file pattern'"))
            .arg(Arg::from_usage("[excludes_from] --excludes-from [FILE] 'Read the list of excludes from this file'"))
            .arg(Arg::from_usage("[no_default_excludes] --no-default-excludes 'Do not load the default excludes file'"))
            .arg(Arg::from_usage("--tar 'Read the source data from a tar file'")
                .conflicts_with_all(&["reference", "exclude", "excludes_from"]))
            .arg(Arg::from_usage("<SRC> 'Source path to backup'")
                .validator(validate_existing_path_or_stdio))
            .arg(Arg::from_usage("<BACKUP> 'Backup path, [repository]::backup'")
                .validator(|val| validate_repo_path(val, true, Some(true), Some(false)))))
        .subcommand(SubCommand::with_name("restore").about("Restore a backup or subtree")
            .arg(Arg::from_usage("--tar 'Restore in form of a tar file'"))
            .arg(Arg::from_usage("<BACKUP> 'The backup/subtree path, [repository]::backup[::subtree]'")
                .validator(|val| validate_repo_path(val, true, Some(true), None)))
            .arg(Arg::from_usage("<DST> 'Destination path for backup'")))
        .subcommand(SubCommand::with_name("remove").aliases(&["rm", "delete", "del"]).about("Remove a backup or a subtree")
            .arg(Arg::from_usage("-f --force 'Remove multiple backups in a backup folder'"))
            .arg(Arg::from_usage("<BACKUP> 'The backup/subtree path, [repository]::backup[::subtree]'")
                .validator(|val| validate_repo_path(val, true, Some(true), None))))
        .subcommand(SubCommand::with_name("prune").about("Remove backups based on age")
            .arg(Arg::from_usage("-p --prefix [PREFIX] 'Only consider backups starting with this prefix'"))
            .arg(Arg::from_usage("-d --daily [NUM] 'Keep this number of daily backups'")
                .default_value("0").validator(validate_num))
            .arg(Arg::from_usage("-w --weekly [NUM] 'Keep this number of weekly backups'")
                .default_value("0").validator(validate_num))
            .arg(Arg::from_usage("-m --monthly [NUM] 'Keep this number of monthly backups'")
                .default_value("0").validator(validate_num))
            .arg(Arg::from_usage("-y --yearly [NUM] 'Keep this number of yearly backups'")
                .default_value("0").validator(validate_num))
            .arg(Arg::from_usage("-f --force 'Actually run the prune instead of simulating it'"))
            .arg(Arg::from_usage("<REPO> 'Path of the repository'")
                .validator(|val| validate_repo_path(val, true, Some(false), Some(false)))))
        .subcommand(SubCommand::with_name("vacuum").about("Reclaim space by rewriting bundles")
            .arg(Arg::from_usage("-r --ratio [NUM] 'Ratio in % of unused space in a bundle to rewrite that bundle'")
                .default_value(DEFAULT_VACUUM_RATIO_STR).validator(validate_num))
            .arg(Arg::from_usage("--combine 'Combine small bundles into larger ones'"))
            .arg(Arg::from_usage("-f --force 'Actually run the vacuum instead of simulating it'"))
            .arg(Arg::from_usage("<REPO> 'Path of the repository'")
                .validator(|val| validate_repo_path(val, true, Some(false), Some(false)))))
        .subcommand(SubCommand::with_name("check").about("Check the repository, a backup or a backup subtree")
            .arg(Arg::from_usage("-b --bundles 'Check the bundles'"))
            .arg(Arg::from_usage("[bundle_data] --bundle-data 'Check bundle contents (slow)'").requires("bundles").alias("data"))
            .arg(Arg::from_usage("-i --index 'Check the chunk index'"))
            .arg(Arg::from_usage("-r --repair 'Try to repair errors'"))
            .arg(Arg::from_usage("<PATH> 'Path of the repository/backup/subtree, [repository][::backup[::subtree]]'")
                .validator(|val| validate_repo_path(val, true, None, None))))
        .subcommand(SubCommand::with_name("list").alias("ls").about("List backups or backup contents")
            .arg(Arg::from_usage("<PATH> 'Path of the repository/backup/subtree, [repository][::backup[::subtree]]'")
                .validator(|val| validate_repo_path(val, true, None, None))))
        .subcommand(SubCommand::with_name("mount").about("Mount the repository, a backup or a subtree")
            .arg(Arg::from_usage("<PATH> 'Path of the repository/backup/subtree, [repository][::backup[::subtree]]'")
                .validator(|val| validate_repo_path(val, true, None, None)))
            .arg(Arg::from_usage("<MOUNTPOINT> 'Existing mount point'")
                .validator(validate_existing_path)))
        .subcommand(SubCommand::with_name("bundlelist").about("List bundles in a repository")
            .arg(Arg::from_usage("<REPO> 'Path of the repository'")
                .validator(|val| validate_repo_path(val, true, Some(false), Some(false)))))
        .subcommand(SubCommand::with_name("bundleinfo").about("Display information on a bundle")
            .arg(Arg::from_usage("<REPO> 'Path of the repository'")
                .validator(|val| validate_repo_path(val, true, Some(false), Some(false))))
            .arg(Arg::from_usage("<BUNDLE> 'Id of the bundle'")))
        .subcommand(SubCommand::with_name("import").about("Reconstruct a repository from the remote storage")
            .arg(Arg::from_usage("-k --key [FILE]... 'Key file needed to read the bundles'"))
            .arg(Arg::from_usage("<REMOTE> 'Remote repository path'")
                .validator(validate_existing_path))
            .arg(Arg::from_usage("<REPO> 'The path for the new repository'")
                .validator(|val| validate_repo_path(val, false, Some(false), Some(false)))))
        .subcommand(SubCommand::with_name("info").about("Display information on a repository, a backup or a subtree")
            .arg(Arg::from_usage("<PATH> 'Path of the repository/backup/subtree, [repository][::backup[::subtree]]'")
                .validator(|val| validate_repo_path(val, true, None, None))))
        .subcommand(SubCommand::with_name("analyze").about("Analyze the used and reclaimable space of bundles")
            .arg(Arg::from_usage("<REPO> 'Path of the repository'")
                .validator(|val| validate_repo_path(val, true, Some(false), Some(false)))))
        .subcommand(SubCommand::with_name("versions").about("Find different versions of a file in all backups")
            .arg(Arg::from_usage("<REPO> 'Path of the repository'")
                .validator(|val| validate_repo_path(val, true, Some(false), Some(false))))
            .arg(Arg::from_usage("<PATH> 'Path of the file'")))
        .subcommand(SubCommand::with_name("diff").about("Display differences between two backup versions")
            .arg(Arg::from_usage("<OLD> 'Old version, [repository]::backup[::subpath]'")
                .validator(|val| validate_repo_path(val, true, Some(true), None)))
            .arg(Arg::from_usage("<NEW> 'New version, [repository]::backup[::subpath]'")
                .validator(|val| validate_repo_path(val, true, Some(true), None))))
        .subcommand(SubCommand::with_name("copy").alias("cp").about("Create a copy of a backup")
            .arg(Arg::from_usage("<SRC> 'Existing backup, [repository]::backup'")
                .validator(|val| validate_repo_path(val, true, Some(true), Some(false))))
            .arg(Arg::from_usage("<DST> 'Destination backup, [repository]::backup'")
                .validator(|val| validate_repo_path(val, true, Some(true), Some(false)))))
        .subcommand(SubCommand::with_name("config").about("Display or change the configuration")
            .arg(Arg::from_usage("[bundle_size] --bundle-size [SIZE] 'Set the target bundle size in MiB'")
                .validator(validate_num))
            .arg(Arg::from_usage("--chunker [CHUNKER] 'Set the chunker algorithm and target chunk size'")
                .validator(validate_chunker))
            .arg(Arg::from_usage("-c --compression [COMPRESSION] 'Set the compression method and level'")
                .validator(validate_compression))
            .arg(Arg::from_usage("-e --encryption [PUBLIC_KEY] 'The public key to use for encryption'")
                .validator(validate_public_key))
            .arg(Arg::from_usage("--hash [HASH] 'Set the hash method'")
                .validator(validate_hash))
            .arg(Arg::from_usage("<REPO> 'Path of the repository'")
                .validator(|val| validate_repo_path(val, true, Some(false), Some(false)))))
        .subcommand(SubCommand::with_name("genkey").about("Generate a new key pair")
            .arg(Arg::from_usage("-p --password [PASSWORD] 'Derive the key pair from the given password'"))
            .arg(Arg::from_usage("[FILE] 'Destination file for the keypair'")))
        .subcommand(SubCommand::with_name("addkey").about("Add a key pair to the repository")
            .arg(Arg::from_usage("-g --generate 'Generate a new key pair'")
                .conflicts_with("FILE"))
            .arg(Arg::from_usage("[set_default] --default -d 'Set the key pair as default'"))
            .arg(Arg::from_usage("-p --password [PASSWORD] 'Derive the key pair from the given password'")
                .requires("generate"))
            .arg(Arg::from_usage("[FILE] 'File containing the keypair'")
                .validator(validate_existing_path))
            .arg(Arg::from_usage("<REPO> 'Path of the repository'")
                .validator(|val| validate_repo_path(val, true, Some(false), Some(false)))))
        .subcommand(SubCommand::with_name("algotest").about("Test a specific algorithm combination")
            .arg(Arg::from_usage("[bundle_size] --bundle-size [SIZE] 'Set the target bundle size in MiB'")
                .default_value(DEFAULT_BUNDLE_SIZE_STR).validator(validate_num))
            .arg(Arg::from_usage("--chunker [CHUNKER] 'Set the chunker algorithm and target chunk size'")
                .default_value(DEFAULT_CHUNKER).validator(validate_chunker))
            .arg(Arg::from_usage("-c --compression [COMPRESSION] 'Set the compression method and level'")
                .default_value(DEFAULT_COMPRESSION).validator(validate_compression))
            .arg(Arg::from_usage("-e --encrypt 'Generate a keypair and enable encryption'"))
            .arg(Arg::from_usage("--hash [HASH] 'Set the hash method'")
                .default_value(DEFAULT_HASH).validator(validate_hash))
            .arg(Arg::from_usage("<FILE> 'File with test data'")
                .validator(validate_existing_path))).get_matches();
    let verbose_count = args.subcommand()
        .1
        .map(|m| m.occurrences_of("verbose"))
        .unwrap_or(0) + args.occurrences_of("verbose");
    let quiet_count = args.subcommand()
        .1
        .map(|m| m.occurrences_of("quiet"))
        .unwrap_or(0) + args.occurrences_of("quiet");
    let log_level = match 1 + verbose_count - quiet_count {
        0 => LogLevel::Warn,
        1 => LogLevel::Info,
        2 => LogLevel::Debug,
        _ => LogLevel::Trace,
    };
    let args = match args.subcommand() {
        ("init", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                false,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::Init {
                bundle_size: (parse_num(args.value_of("bundle_size").unwrap()).unwrap() *
                                  1024 * 1024) as usize,
                chunker: parse_chunker(args.value_of("chunker").unwrap()).unwrap(),
                compression: parse_compression(args.value_of("compression").unwrap()).unwrap(),
                encryption: args.is_present("encrypt"),
                hash: parse_hash(args.value_of("hash").unwrap()).unwrap(),
                repo_path: repository,
                remote_path: args.value_of("remote").unwrap().to_string()
            }
        }
        ("backup", Some(args)) => {
            let (repository, backup, _inode) = parse_repo_path(
                args.value_of("BACKUP").unwrap(),
                true,
                Some(true),
                Some(false)
            ).unwrap();
            Arguments::Backup {
                repo_path: repository,
                backup_name: backup.unwrap().to_string(),
                full: args.is_present("full"),
                same_device: !args.is_present("cross_device"),
                excludes: args.values_of("exclude")
                    .map(|v| v.map(|k| k.to_string()).collect())
                    .unwrap_or_else(|| vec![]),
                excludes_from: args.value_of("excludes_from").map(|v| v.to_string()),
                src_path: args.value_of("SRC").unwrap().to_string(),
                reference: args.value_of("reference").map(|v| v.to_string()),
                no_default_excludes: args.is_present("no_default_excludes"),
                tar: args.is_present("tar")
            }
        }
        ("restore", Some(args)) => {
            let (repository, backup, inode) =
                parse_repo_path(args.value_of("BACKUP").unwrap(), true, Some(true), None).unwrap();
            Arguments::Restore {
                repo_path: repository,
                backup_name: backup.unwrap().to_string(),
                inode: inode.map(|v| v.to_string()),
                dst_path: args.value_of("DST").unwrap().to_string(),
                tar: args.is_present("tar")
            }
        }
        ("remove", Some(args)) => {
            let (repository, backup, inode) =
                parse_repo_path(args.value_of("BACKUP").unwrap(), true, Some(true), None).unwrap();
            Arguments::Remove {
                repo_path: repository,
                backup_name: backup.unwrap().to_string(),
                inode: inode.map(|v| v.to_string()),
                force: args.is_present("force")
            }
        }
        ("prune", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                true,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::Prune {
                repo_path: repository,
                prefix: args.value_of("prefix").unwrap_or("").to_string(),
                force: args.is_present("force"),
                daily: parse_num(args.value_of("daily").unwrap()).unwrap() as usize,
                weekly: parse_num(args.value_of("weekly").unwrap()).unwrap() as usize,
                monthly: parse_num(args.value_of("monthly").unwrap()).unwrap() as usize,
                yearly: parse_num(args.value_of("yearly").unwrap()).unwrap() as usize
            }
        }
        ("vacuum", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                true,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::Vacuum {
                repo_path: repository,
                force: args.is_present("force"),
                combine: args.is_present("combine"),
                ratio: parse_num(args.value_of("ratio").unwrap()).unwrap() as f32 / 100.0
            }
        }
        ("check", Some(args)) => {
            let (repository, backup, inode) =
                parse_repo_path(args.value_of("PATH").unwrap(), true, None, None).unwrap();
            Arguments::Check {
                repo_path: repository,
                backup_name: backup.map(|v| v.to_string()),
                inode: inode.map(|v| v.to_string()),
                bundles: args.is_present("bundles"),
                bundle_data: args.is_present("bundle_data"),
                index: args.is_present("index"),
                repair: args.is_present("repair")
            }
        }
        ("list", Some(args)) => {
            let (repository, backup, inode) =
                parse_repo_path(args.value_of("PATH").unwrap(), true, None, None).unwrap();
            Arguments::List {
                repo_path: repository,
                backup_name: backup.map(|v| v.to_string()),
                inode: inode.map(|v| v.to_string())
            }
        }
        ("bundlelist", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                true,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::BundleList { repo_path: repository }
        }
        ("bundleinfo", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                true,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::BundleInfo {
                repo_path: repository,
                bundle_id: try!(parse_bundle_id(args.value_of("BUNDLE").unwrap()))
            }
        }
        ("info", Some(args)) => {
            let (repository, backup, inode) =
                parse_repo_path(args.value_of("PATH").unwrap(), true, None, None).unwrap();
            Arguments::Info {
                repo_path: repository,
                backup_name: backup.map(|v| v.to_string()),
                inode: inode.map(|v| v.to_string())
            }
        }
        ("copy", Some(args)) => {
            let (repository_src, backup_src, _inode) =
                parse_repo_path(args.value_of("SRC").unwrap(), true, Some(true), Some(false))
                    .unwrap();
            let (repository_dst, backup_dst, _inode) =
                parse_repo_path(args.value_of("DST").unwrap(), true, Some(true), Some(false))
                    .unwrap();
            Arguments::Copy {
                repo_path_src: repository_src,
                backup_name_src: backup_src.unwrap().to_string(),
                repo_path_dst: repository_dst,
                backup_name_dst: backup_dst.unwrap().to_string()
            }
        }
        ("mount", Some(args)) => {
            let (repository, backup, inode) =
                parse_repo_path(args.value_of("PATH").unwrap(), true, None, None).unwrap();
            Arguments::Mount {
                repo_path: repository,
                backup_name: backup.map(|v| v.to_string()),
                inode: inode.map(|v| v.to_string()),
                mount_point: args.value_of("MOUNTPOINT").unwrap().to_string()
            }
        }
        ("versions", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                true,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::Versions {
                repo_path: repository,
                path: args.value_of("PATH").unwrap().to_string()
            }
        }
        ("diff", Some(args)) => {
            let (repository_old, backup_old, inode_old) =
                parse_repo_path(args.value_of("OLD").unwrap(), true, Some(true), None).unwrap();
            let (repository_new, backup_new, inode_new) =
                parse_repo_path(args.value_of("NEW").unwrap(), true, Some(true), None).unwrap();
            Arguments::Diff {
                repo_path_old: repository_old,
                backup_name_old: backup_old.unwrap().to_string(),
                inode_old: inode_old.map(|v| v.to_string()),
                repo_path_new: repository_new,
                backup_name_new: backup_new.unwrap().to_string(),
                inode_new: inode_new.map(|v| v.to_string())
            }
        }
        ("analyze", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                true,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::Analyze { repo_path: repository }
        }
        ("import", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                false,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::Import {
                repo_path: repository,
                remote_path: args.value_of("REMOTE").unwrap().to_string(),
                key_files: args.values_of("key")
                    .map(|v| v.map(|k| k.to_string()).collect())
                    .unwrap_or_else(|| vec![])
            }
        }
        ("config", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                true,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::Config {
                bundle_size: args.value_of("bundle_size").map(|v| {
                    parse_num(v).unwrap() as usize * 1024 * 1024
                }),
                chunker: args.value_of("chunker").map(|v| parse_chunker(v).unwrap()),
                compression: args.value_of("compression").map(|v| {
                    parse_compression(v).unwrap()
                }),
                encryption: args.value_of("encryption").map(
                    |v| parse_public_key(v).unwrap()
                ),
                hash: args.value_of("hash").map(|v| parse_hash(v).unwrap()),
                repo_path: repository
            }
        }
        ("genkey", Some(args)) => {
            Arguments::GenKey {
                file: args.value_of("FILE").map(|v| v.to_string()),
                password: args.value_of("password").map(|v| v.to_string())
            }
        }
        ("addkey", Some(args)) => {
            let (repository, _backup, _inode) = parse_repo_path(
                args.value_of("REPO").unwrap(),
                true,
                Some(false),
                Some(false)
            ).unwrap();
            Arguments::AddKey {
                repo_path: repository,
                set_default: args.is_present("set_default"),
                password: args.value_of("password").map(|v| v.to_string()),
                file: args.value_of("FILE").map(|v| v.to_string())
            }
        }
        ("algotest", Some(args)) => {
            Arguments::AlgoTest {
                bundle_size: (parse_num(args.value_of("bundle_size").unwrap()).unwrap() *
                                  1024 * 1024) as usize,
                chunker: parse_chunker(args.value_of("chunker").unwrap()).unwrap(),
                compression: parse_compression(args.value_of("compression").unwrap()).unwrap(),
                encrypt: args.is_present("encrypt"),
                hash: parse_hash(args.value_of("hash").unwrap()).unwrap(),
                file: args.value_of("FILE").unwrap().to_string()
            }
        }
        _ => {
            error!("No subcommand given");
            return Err(ErrorCode::InvalidArgs);
        }
    };
    Ok((log_level, args))
}
