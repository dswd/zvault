use clap::{Arg, App, SubCommand};

use docopt::Docopt;

use ::chunker::ChunkerType;
use ::util::{ChecksumType, Compression, HashMethod};

use std::process::exit;
use std::path::Path;


static USAGE: &'static str = "
Usage:
    zvault init [--bundle-size SIZE] [--chunker METHOD] [--chunk-size SIZE] [--compression COMPRESSION] <repo>
    zvault backup [--full] <backup> <path>
    zvault restore <backup> [<src>] <dst>
    zvault check [--full] <repo>
    zvault backups <repo>
    zvault info <backup>
    zvault list [--tree] <backup> <path>
    zvault stats <repo>
    zvault bundles <repo>
    zvault algotest <file>

Options:
    --tree                         Print the whole (sub-)tree from the backup
    --full                         Whether to verify the repository by loading all bundles
    --bundle-size SIZE             The target size of a full bundle in MiB [default: 25]
    --chunker METHOD               The chunking algorithm to use [default: fastcdc]
    --chunk-size SIZE              The target average chunk size in KiB [default: 8]
    --compression COMPRESSION      The compression to use [default: brotli/3]
";


#[derive(RustcDecodable, Debug)]
pub struct DocoptArgs {
    pub cmd_init: bool,
    pub cmd_backup: bool,
    pub cmd_restore: bool,
    pub cmd_check: bool,

    pub cmd_backups: bool,
    pub cmd_info: bool,
    pub cmd_list: bool,

    pub cmd_stats: bool,
    pub cmd_bundles: bool,

    pub cmd_algotest: bool,
    pub cmd_stat: bool,

    pub arg_file: Option<String>,
    pub arg_repo: Option<String>,
    pub arg_path: Option<String>,
    pub arg_src: Option<String>,
    pub arg_dst: Option<String>,
    pub arg_backup: Option<String>,

    pub flag_full: bool,
    pub flag_bundle_size: usize,
    pub flag_chunker: String,
    pub flag_chunk_size: usize,
    pub flag_compression: String,
    pub flag_tree: bool
}


pub enum Arguments {
    Init {
        repo_path: String,
        bundle_size: usize,
        chunker: ChunkerType,
        chunk_size: usize,
        compression: Compression
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
        file: String
    }
}


pub fn parse() -> DocoptArgs {
    Docopt::new(USAGE).and_then(|d| d.decode()).unwrap_or_else(|e| e.exit())
}

pub fn parse2() -> Arguments {
    let args = clap_app!(zvault =>
        (version: "0.1")
        (author: "Dennis Schwerdel <schwerdel@googlemail.com>")
        (about: "Deduplicating backup tool")
        (@subcommand init =>
            (about: "initializes a new repository")
            (@arg bundle_size: --bundle-size +takes_value "maximal bundle size")
            (@arg chunker: --chunker +takes_value "chunker algorithm")
            (@arg chunk_size: --chunk-size +takes_value "average chunk size")
            (@arg compression: --compression -c +takes_value "compression to use")
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
            (@arg bundle_size: --bundle-size +takes_value "maximal bundle size")
            (@arg chunker: --chunker +takes_value "chunker algorithm")
            (@arg chunk_size: --chunk-size +takes_value "average chunk size")
            (@arg compression: --compression -c +takes_value "compression to use")
            (@arg FILE: +required "the file to test the algorithms with")
        )
    ).get_matches();
    if let Some(args) = args.subcommand_matches("init") {
    }
    unimplemented!()
}
