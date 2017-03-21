use ::prelude::*;

use std::process::exit;


pub enum Arguments {
    Init {
        repo_path: String,
        bundle_size: usize,
        chunker: ChunkerType,
        compression: Option<Compression>,
        encryption: bool,
        hash: HashMethod
    },
    Backup {
        repo_path: String,
        backup_name: String,
        src_path: String,
        full: bool,
        reference: Option<String>
    },
    Restore {
        repo_path: String,
        backup_name: String,
        inode: Option<String>,
        dst_path: String
    },
    Remove {
        repo_path: String,
        backup_name: String,
        inode: Option<String>
    },
    Prune {
        repo_path: String,
        prefix: String,
        daily: Option<usize>,
        weekly: Option<usize>,
        monthly: Option<usize>,
        yearly: Option<usize>,
        force: bool
    },
    Vacuum {
        repo_path: String,
        ratio: f32,
        force: bool
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
    Import {
        repo_path: String,
        remote_path: String
    },
    Configure {
        repo_path: String,
        bundle_size: Option<usize>,
        chunker: Option<ChunkerType>,
        compression: Option<Option<Compression>>,
        encryption: Option<Option<PublicKey>>,
        hash: Option<HashMethod>
    },
    GenKey {
    },
    AddKey {
        repo_path: String,
        key_pair: Option<(PublicKey, SecretKey)>,
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

fn parse_float(num: &str, name: &str) -> f64 {
    if let Ok(num) = num.parse::<f64>() {
        num
    } else {
        error!("{} must be a floating-point number, was '{}'", name, num);
        exit(1);
    }
}


fn parse_chunker(val: &str) -> ChunkerType {
    if let Ok(chunker) = ChunkerType::from_string(val) {
        chunker
    } else {
        error!("Invalid chunker method/size: {}", val);
        exit(1);
    }
}

fn parse_compression(val: &str) -> Option<Compression> {
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

fn parse_public_key(val: &str) -> PublicKey {
    let bytes = match parse_hex(val) {
        Ok(bytes) => bytes,
        Err(_) => {
            error!("Invalid key: {}", val);
            exit(1);
        }
    };
    if let Some(key) = PublicKey::from_slice(&bytes) {
        key
    } else {
        error!("Invalid key: {}", val);
        exit(1);
    }
}

fn parse_secret_key(val: &str) -> SecretKey {
    let bytes = match parse_hex(val) {
        Ok(bytes) => bytes,
        Err(_) => {
            error!("Invalid key: {}", val);
            exit(1);
        }
    };
    if let Some(key) = SecretKey::from_slice(&bytes) {
        key
    } else {
        error!("Invalid key: {}", val);
        exit(1);
    }
}

fn parse_hash(val: &str) -> HashMethod {
    if let Ok(hash) = HashMethod::from(val) {
        hash
    } else {
        error!("Invalid hash method: {}", val);
        exit(1);
    }
}


pub fn parse() -> Arguments {
    let args = clap_app!(zvault =>
        (version: crate_version!())
        (author: crate_authors!(",\n"))
        (about: crate_description!())
        (@setting SubcommandRequiredElseHelp)
        (@setting GlobalVersion)
        (@setting VersionlessSubcommands)
        (@setting UnifiedHelpMessage)
        (@subcommand init =>
            (about: "initializes a new repository")
            (@arg bundle_size: --bundlesize +takes_value "maximal bundle size in MiB [default: 25]")
            (@arg chunker: --chunker +takes_value "chunker algorithm [default: fastcdc/8]")
            (@arg compression: --compression -c +takes_value "compression to use [default: brotli/3]")
            (@arg encryption: --encryption -e "generate a keypair and enable encryption")
            (@arg hash: --hash +takes_value "hash method to use [default: blake2]")
            (@arg REPO: +required "path of the repository")
        )
        (@subcommand backup =>
            (about: "creates a new backup")
            (@arg full: --full "create a full backup")
            (@arg reference: --ref +takes_value "the reference backup to use for partial backup")
            (@arg SRC: +required "source path to backup")
            (@arg BACKUP: +required "repository::backup path")
        )
        (@subcommand restore =>
            (about: "restores a backup (or subpath)")
            (@arg BACKUP: +required "repository::backup[::subpath] path")
            (@arg DST: +required "destination path for backup")
        )
        (@subcommand remove =>
            (about: "removes a backup or a subpath")
            (@arg BACKUP: +required "repository::backup[::subpath] path")
        )
        (@subcommand prune =>
            (about: "removes backups based on age")
            (@arg prefix: --prefix +takes_value "only consider backups starting with this prefix")
            (@arg daily: --daily +takes_value "keep this number of daily backups")
            (@arg weekly: --weekly +takes_value "keep this number of weekly backups")
            (@arg monthly: --monthly +takes_value "keep this number of monthly backups")
            (@arg yearly: --yearly +takes_value  "keep this number of yearly backups")
            (@arg force: --force -f "actually run the prunce instead of simulating it")
            (@arg REPO: +required "path of the repository")
        )
        (@subcommand vacuum =>
            (about: "saves space by combining and recompressing bundles")
            (@arg ratio: --ratio -r +takes_value "ratio of unused chunks in a bundle to rewrite that bundle")
            (@arg force: --force -f "actually run the vacuum instead of simulating it")
            (@arg REPO: +required "path of the repository")
        )
        (@subcommand check =>
            (about: "checks the repository, a backup or a backup subpath")
            (@arg full: --full "also check file contents")
            (@arg PATH: +required "repository[::backup] path")
        )
        (@subcommand list =>
            (about: "lists backups or backup contents")
            (@arg PATH: +required "repository[::backup[::subpath]] path")
        )
        (@subcommand listbundles =>
            (about: "lists bundles in a repository")
            (@arg REPO: +required "path of the repository")
        )
        (@subcommand import =>
            (about: "reconstruct a repository from the remote files")
            (@arg REMOTE: +required "remote repository path")
            (@arg REPO: +required "path of the local repository to create")
        )
        (@subcommand info =>
            (about: "displays information on a repository, a backup or a path in a backup")
            (@arg PATH: +required "repository[::backup[::subpath]] path")
        )
        (@subcommand configure =>
            (about: "changes the configuration")
            (@arg REPO: +required "path of the repository")
            (@arg bundle_size: --bundlesize +takes_value "maximal bundle size in MiB [default: 25]")
            (@arg chunker: --chunker +takes_value "chunker algorithm [default: fastcdc/8]")
            (@arg compression: --compression -c +takes_value "compression to use [default: brotli/3]")
            (@arg encryption: --encryption -e +takes_value "the public key to use for encryption")
            (@arg hash: --hash +takes_value "hash method to use [default: blake2]")
        )
        (@subcommand genkey =>
            (about: "generates a new key pair")
        )
        (@subcommand addkey =>
            (about: "adds a key to the respository")
            (@arg REPO: +required "path of the repository")
            (@arg generate: --generate "generate a new key")
            (@arg set_default: --default "set this key as default")
            (@arg PUBLIC: +takes_value "the public key")
            (@arg SECRET: +takes_value "the secret key")
        )
        (@subcommand algotest =>
            (about: "test a specific algorithm combination")
            (@arg bundle_size: --bundlesize +takes_value "maximal bundle size in MiB [default: 25]")
            (@arg chunker: --chunker +takes_value "chunker algorithm [default: fastcdc/8]")
            (@arg compression: --compression -c +takes_value "compression to use [default: brotli/3]")
            (@arg encrypt: --encrypt -e "enable encryption")
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
            chunker: parse_chunker(args.value_of("chunker").unwrap_or("fastcdc/8")),
            compression: parse_compression(args.value_of("compression").unwrap_or("brotli/3")),
            encryption: args.is_present("encryption"),
            hash: parse_hash(args.value_of("hash").unwrap_or("blake2")),
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
            src_path: args.value_of("SRC").unwrap().to_string(),
            reference: args.value_of("reference").map(|v| v.to_string())
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
    if let Some(args) = args.subcommand_matches("remove") {
        let (repository, backup, inode) = split_repo_path(args.value_of("BACKUP").unwrap());
        if backup.is_none() {
            println!("A backup must be specified");
            exit(1);
        }
        return Arguments::Remove {
            repo_path: repository.to_string(),
            backup_name: backup.unwrap().to_string(),
            inode: inode.map(|v| v.to_string())
        }
    }
    if let Some(args) = args.subcommand_matches("prune") {
        let (repository, backup, inode) = split_repo_path(args.value_of("REPO").unwrap());
        if backup.is_some() || inode.is_some() {
            println!("No backups or subpaths may be given here");
            exit(1);
        }
        return Arguments::Prune {
            repo_path: repository.to_string(),
            prefix: args.value_of("prefix").unwrap_or("").to_string(),
            force: args.is_present("force"),
            daily: args.value_of("daily").map(|v| parse_num(v, "daily backups") as usize),
            weekly: args.value_of("weekly").map(|v| parse_num(v, "weekly backups") as usize),
            monthly: args.value_of("monthly").map(|v| parse_num(v, "monthly backups") as usize),
            yearly: args.value_of("yearly").map(|v| parse_num(v, "yearly backups") as usize),
        }
    }
    if let Some(args) = args.subcommand_matches("vacuum") {
        let (repository, backup, inode) = split_repo_path(args.value_of("REPO").unwrap());
        if backup.is_some() || inode.is_some() {
            println!("No backups or subpaths may be given here");
            exit(1);
        }
        return Arguments::Vacuum {
            repo_path: repository.to_string(),
            force: args.is_present("force"),
            ratio: parse_float(args.value_of("ratio").unwrap_or("0.5"), "ratio") as f32
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
        let (repository, backup, inode) = split_repo_path(args.value_of("REPO").unwrap());
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
    if let Some(args) = args.subcommand_matches("import") {
        let (repository, backup, inode) = split_repo_path(args.value_of("REPO").unwrap());
        if backup.is_some() || inode.is_some() {
            println!("No backups or subpaths may be given here");
            exit(1);
        }
        return Arguments::Import {
            repo_path: repository.to_string(),
            remote_path: args.value_of("REMOTE").unwrap().to_string()
        }
    }
    if let Some(args) = args.subcommand_matches("configure") {
        let (repository, backup, inode) = split_repo_path(args.value_of("REPO").unwrap());
        if backup.is_some() || inode.is_some() {
            println!("No backups or subpaths may be given here");
            exit(1);
        }
        return Arguments::Configure {
            bundle_size: args.value_of("bundle_size").map(|v| (parse_num(v, "Bundle size") * 1024 * 1024) as usize),
            chunker: args.value_of("chunker").map(|v| parse_chunker(v)),
            compression: args.value_of("compression").map(|v| parse_compression(v)),
            encryption: args.value_of("encryption").map(|v| {
                if v == "none" {
                    None
                } else {
                    Some(parse_public_key(v))
                }
            }),
            hash: args.value_of("hash").map(|v| parse_hash(v)),
            repo_path: repository.to_string(),
        }
    }
    if let Some(_args) = args.subcommand_matches("genkey") {
        return Arguments::GenKey {}
    }
    if let Some(args) = args.subcommand_matches("addkey") {
        let (repository, backup, inode) = split_repo_path(args.value_of("REPO").unwrap());
        if backup.is_some() || inode.is_some() {
            println!("No backups or subpaths may be given here");
            exit(1);
        }
        let generate = args.is_present("generate");
        if !generate && (!args.is_present("PUBLIC") || !args.is_present("SECRET")) {
            println!("Without --generate, a public and secret key must be given");
            exit(1);
        }
        if generate && (args.is_present("PUBLIC") || args.is_present("SECRET")) {
            println!("With --generate, no public or secret key may be given");
            exit(1);
        }
        let key_pair = if generate {
            None
        } else {
            Some((parse_public_key(args.value_of("PUBLIC").unwrap()), parse_secret_key(args.value_of("SECRET").unwrap())))
        };
        return Arguments::AddKey {
            repo_path: repository.to_string(),
            set_default: args.is_present("set_default"),
            key_pair: key_pair
        }
    }
    if let Some(args) = args.subcommand_matches("algotest") {
        return Arguments::AlgoTest {
            bundle_size: (parse_num(args.value_of("bundle_size").unwrap_or("25"), "Bundle size") * 1024 * 1024) as usize,
            chunker: parse_chunker(args.value_of("chunker").unwrap_or("fastcdc/8")),
            compression: parse_compression(args.value_of("compression").unwrap_or("brotli/3")),
            encrypt: args.is_present("encrypt"),
            hash: parse_hash(args.value_of("hash").unwrap_or("blake2")),
            file: args.value_of("FILE").unwrap().to_string(),
        }
    }
    error!("No subcommand given");
    exit(1);
}
