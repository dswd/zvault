use ::prelude::*;

use std::io;
use std::path::PathBuf;

use super::backup_file::BackupFileError;
use super::backup::BackupError;
use super::bundle_map::BundleMapError;
use super::config::ConfigError;
use super::metadata::InodeError;


quick_error!{
    #[derive(Debug)]
    #[allow(unknown_lints,large_enum_variant)]
    pub enum RepositoryError {
        NoRemote {
            description("Remote storage not found")
            display("Repository error: The remote storage has not been found, may be it needs to be mounted?")
        }
        Index(err: IndexError) {
            from()
            cause(err)
            description("Index error")
            display("Repository error: index error\n\tcaused by: {}", err)
        }
        BundleDb(err: BundleDbError) {
            from()
            cause(err)
            description("Bundle error")
            display("Repository error: bundle db error\n\tcaused by: {}", err)
        }
        BundleWriter(err: BundleWriterError) {
            from()
            cause(err)
            description("Bundle write error")
            display("Repository error: failed to write to new bundle\n\tcaused by: {}", err)
        }
        BackupFile(err: BackupFileError) {
            from()
            cause(err)
            description("Backup file error")
            display("Repository error: backup file error\n\tcaused by: {}", err)
        }
        Chunker(err: ChunkerError) {
            from()
            cause(err)
            description("Chunker error")
            display("Repository error: failed to chunk data\n\tcaused by: {}", err)
        }
        Config(err: ConfigError) {
            from()
            cause(err)
            description("Configuration error")
            display("Repository error: configuration error\n\tcaused by: {}", err)
        }
        Inode(err: InodeError) {
            from()
            cause(err)
            description("Inode error")
            display("Repository error: inode error\n\tcaused by: {}", err)
        }
        LoadKeys(err: EncryptionError) {
            from()
            cause(err)
            description("Failed to load keys")
            display("Repository error: failed to load keys\n\tcaused by: {}", err)
        }
        BundleMap(err: BundleMapError) {
            from()
            cause(err)
            description("Bundle map error")
            display("Repository error: bundle map error\n\tcaused by: {}", err)
        }
        Integrity(err: IntegrityError) {
            from()
            cause(err)
            description("Integrity error")
            display("Repository error: integrity error\n\tcaused by: {}", err)
        }
        Dirty {
            description("Dirty repository")
            display("The repository is dirty, please run a check")
        }
        Backup(err: BackupError) {
            from()
            cause(err)
            description("Failed to create a backup")
            display("Repository error: failed to create backup\n\tcaused by: {}", err)
        }
        Lock(err: LockError) {
            from()
            cause(err)
            description("Failed to obtain lock")
            display("Repository error: failed to obtain lock\n\tcaused by: {}", err)
        }

        Io(err: io::Error) {
            from()
            cause(err)
            description("IO error")
            display("IO error: {}", err)
        }
        NoSuchFileInBackup(backup: Backup, path: PathBuf) {
            description("No such file in backup")
            display("The backup does not contain the file {:?}", path)
        }
    }
}
