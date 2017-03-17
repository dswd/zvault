use ::chunker::ChunkerType;
use ::util::{Compression, HashMethod, ChecksumType};

use std::process::exit;


pub enum Arguments {
    Init {
        repo_path: String,
        bundle_size: usize,
        chunker: ChunkerType,
        compression: Option<Compression>,
        hash: HashMethod
    },
    Backup {
        repo_path: String,
        backup_name: String,
        src_path: String,
        full: bool
    },
    Restore {
        repo_path: String,
        backup_name: String,
        inode: Option<String>,
        dst_path: String
    },
    Check {
        repo_path: String,
        backup_name: Option<String>,
        inode: Option<String>,
        full: bool
    },
    List {
        repo_path: String,
        backup_name: Option<String>,
        inode: Option<String>
    },
    Info {
        repo_path: String,
        backup_name: Option<String>,
        inode: Option<String>
    },
    ListBundles {
        repo_path: String
    },
    AlgoTest {
        file: String,
        bundle_size: usize,
        chunker: ChunkerType,
        compression: Option<Compression>,
        hash: HashMethod
    }
}


pub fn split_repo_path(repo_path: &str) -> (&str, Option<&str>, Option<&str>) {
    let mut parts = repo_path.splitn(3, "::");
    let repo = parts.next().unwrap();
    let backup = parts.next();
    let inode = parts.next();
    (repo, backup, inode)
}

fn parse_num(num: &str, name: &str) -> u64 {
    if let Ok(num) = num.parse::<u64>() {
        num
    } else {
        error!("{} must be a number, was '{}'", name, num);
        exit(1);
    }
}

fn parse_chunker(val: Option<&str>) -> ChunkerType {
    if let Ok(chunker) = ChunkerType::from_string(val.unwrap_or("fastcdc/8")) {
        chunker
    } else {
        error!("Invalid chunker method/size: {}", val.unwrap());
        exit(1);
    }
}

fn parse_compression(val: Option<&str>) -> Option<Compression> {
    let val = val.unwrap_or("brotli/3");
    if val == "none" {
        return None
    }
    if let Ok(compression) = Compression::from_string(val) {
        Some(compression)
    } else {
        error!("Invalid compression method/level: {}", val);
        exit(1);
    }
}

#[allow(dead_code)]
fn parse_checksum(val: Option<&str>) -> ChecksumType {
    if let Ok(checksum) = ChecksumType::from(val.unwrap_or("blake2")) {
        checksum
    } else {
        error!("Invalid checksum method: {}", val.unwrap());
        exit(1);
    }
}

fn parse_hash(val: Option<&str>) -> HashMethod {
    if let Ok(hash) = HashMethod::from(val.unwrap_or("blake2")) {
        hash
    } else {
        error!("Invalid hash method: {}", val.unwrap());
        exit(1);
    }
}


pub fn parse() -> Arguments {
    let args = clap_app!(zvault =>
        (version: env!("CARGO_PKG_VERSION"))
        (author: "Dennis Schwerdel <schwerdel@googlemail.com>")
        (about: "Deduplicating backup tool")
        (@setting SubcommandRequiredElseHelp)
        (@setting GlobalVersion)
        (@setting VersionlessSubcommands)
        (@setting UnifiedHelpMessage)
        (@subcommand init =>
            (about: "initializes a new repository")
            (@arg bundle_size: --bundle-size +takes_value "maximal bundle size in MiB [default: 25]")
            (@arg chunker: --chunker +takes_value "chunker algorithm [default: fastcdc/8]")
            (@arg compression: --compression -c +takes_value "compression to use [default: brotli/3]")
            (@arg hash: --hash +takes_value "hash method to use [default: blake2]")
            (@arg REPO: +required "path of the repository")
        )
        (@subcommand backup =>
            (about: "creates a new backup")
            (@arg full: --full "create a full backup")
            (@arg BACKUP: +required "repository::backup path")
            (@arg SRC: +required "source path to backup")
        )
        (@subcommand restore =>
            (about: "restores a backup")
            (@arg BACKUP: +required "repository::backup[::subpath] path")
            (@arg DST: +required "destination path for backup")
        )
        (@subcommand check =>
            (about: "checks the repository")
            (@arg full: --full "also check file contents")
            (@arg PATH: +required "repository[::backup] path")
        )
        (@subcommand list =>
            (about: "lists backups or backup contents")
            (@arg PATH: +required "repository[::backup[::subpath]] path")
        )
        (@subcommand listbundles =>
            (about: "lists bundles in a repository")
            (@arg PATH: +required "repository path")
        )
        (@subcommand info =>
            (about: "displays information on a repository, a backup or a path in a backup")
            (@arg PATH: +required "repository[::backup[::subpath]] path")
        )
        (@subcommand algotest =>
            (about: "test a specific algorithm combination")
            (@arg bundle_size: --bundle-size +takes_value "maximal bundle size in MiB [default: 25]")
            (@arg chunker: --chunker +takes_value "chunker algorithm [default: fastcdc/8]")
            (@arg compression: --compression -c +takes_value "compression to use [default: brotli/3]")
            (@arg hash: --hash +takes_value "hash method to use [default: blake2]")
            (@arg FILE: +required "the file to test the algorithms with")
        )
    ).get_matches();
    if let Some(args) = args.subcommand_matches("init") {
        let (repository, backup, inode) = split_repo_path(args.value_of("REPO").unwrap());
        if backup.is_some() || inode.is_some() {
            println!("No backups or subpaths may be given here");
            exit(1);
        }
        return Arguments::Init {
            bundle_size: (parse_num(args.value_of("bundle_size").unwrap_or("25"), "Bundle size") * 1024 * 1024) as usize,
            chunker: parse_chunker(args.value_of("chunker")),
            compression: parse_compression(args.value_of("compression")),
            hash: parse_hash(args.value_of("hash")),
            repo_path: repository.to_string(),
        }
    }
    if let Some(args) = args.subcommand_matches("backup") {
        let (repository, backup, inode) = split_repo_path(args.value_of("BACKUP").unwrap());
        if backup.is_none() {
            println!("A backup must be specified");
            exit(1);
        }
        if inode.is_some() {
            println!("No subpaths may be given here");
            exit(1);
        }
        return Arguments::Backup {
            repo_path: repository.to_string(),
            backup_name: backup.unwrap().to_string(),
            full: args.is_present("full"),
            src_path: args.value_of("SRC").unwrap().to_string()
        }
    }
    if let Some(args) = args.subcommand_matches("restore") {
        let (repository, backup, inode) = split_repo_path(args.value_of("BACKUP").unwrap());
        if backup.is_none() {
            println!("A backup must be specified");
            exit(1);
        }
        return Arguments::Restore {
            repo_path: repository.to_string(),
            backup_name: backup.unwrap().to_string(),
            inode: inode.map(|v| v.to_string()),
            dst_path: args.value_of("DST").unwrap().to_string()
        }
    }
    if let Some(args) = args.subcommand_matches("check") {
        let (repository, backup, inode) = split_repo_path(args.value_of("PATH").unwrap());
        return Arguments::Check {
            repo_path: repository.to_string(),
            backup_name: backup.map(|v| v.to_string()),
            inode: inode.map(|v| v.to_string()),
            full: args.is_present("full")
        }
    }
    if let Some(args) = args.subcommand_matches("list") {
        let (repository, backup, inode) = split_repo_path(args.value_of("PATH").unwrap());
        return Arguments::List {
            repo_path: repository.to_string(),
            backup_name: backup.map(|v| v.to_string()),
            inode: inode.map(|v| v.to_string())
        }
    }
    if let Some(args) = args.subcommand_matches("listbundles") {
        let (repository, backup, inode) = split_repo_path(args.value_of("PATH").unwrap());
        if backup.is_some() || inode.is_some() {
            println!("No backups or subpaths may be given here");
            exit(1);
        }
        return Arguments::ListBundles {
            repo_path: repository.to_string(),
        }
    }
    if let Some(args) = args.subcommand_matches("info") {
        let (repository, backup, inode) = split_repo_path(args.value_of("PATH").unwrap());
        return Arguments::Info {
            repo_path: repository.to_string(),
            backup_name: backup.map(|v| v.to_string()),
            inode: inode.map(|v| v.to_string())
        }
    }
    if let Some(args) = args.subcommand_matches("algotest") {
        return Arguments::AlgoTest {
            bundle_size: (parse_num(args.value_of("bundle_size").unwrap_or("25"), "Bundle size") * 1024 * 1024) as usize,
            chunker: parse_chunker(args.value_of("chunker")),
            compression: parse_compression(args.value_of("compression")),
            hash: parse_hash(args.value_of("hash")),
            file: args.value_of("FILE").unwrap().to_string(),
        }
    }
    error!("No subcommand given");
    exit(1);
}
