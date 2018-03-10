use prelude::*;

use std::io;
use std::path::PathBuf;

use super::bundle_map::BundleMapError;
use super::config::ConfigError;


quick_error!{
    #[derive(Debug)]
    #[allow(unknown_lints,large_enum_variant)]
    pub enum RepositoryError {
        NoRemote {
            description(tr!("Remote storage not found"))
            display("{}", tr_format!("Repository error: The remote storage has not been found, may be it needs to be mounted?"))
        }
        Index(err: IndexError) {
            from()
            cause(err)
            description(tr!("Index error"))
            display("{}", tr_format!("Repository error: index error\n\tcaused by: {}", err))
        }
        BundleDb(err: BundleDbError) {
            from()
            cause(err)
            description(tr!("Bundle error"))
            display("{}", tr_format!("Repository error: bundle db error\n\tcaused by: {}", err))
        }
        BundleWriter(err: BundleWriterError) {
            from()
            cause(err)
            description(tr!("Bundle write error"))
            display("{}", tr_format!("Repository error: failed to write to new bundle\n\tcaused by: {}", err))
        }
        BackupFile(err: BackupFileError) {
            from()
            cause(err)
            description(tr!("Backup file error"))
            display("{}", tr_format!("Repository error: backup file error\n\tcaused by: {}", err))
        }
        Chunker(err: ChunkerError) {
            from()
            cause(err)
            description(tr!("Chunker error"))
            display("{}", tr_format!("Repository error: failed to chunk data\n\tcaused by: {}", err))
        }
        Config(err: ConfigError) {
            from()
            cause(err)
            description(tr!("Configuration error"))
            display("{}", tr_format!("Repository error: configuration error\n\tcaused by: {}", err))
        }
        Inode(err: InodeError) {
            from()
            cause(err)
            description(tr!("Inode error"))
            display("{}", tr_format!("Repository error: inode error\n\tcaused by: {}", err))
        }
        LoadKeys(err: EncryptionError) {
            from()
            cause(err)
            description(tr!("Failed to load keys"))
            display("{}", tr_format!("Repository error: failed to load keys\n\tcaused by: {}", err))
        }
        BundleMap(err: BundleMapError) {
            from()
            cause(err)
            description(tr!("Bundle map error"))
            display("{}", tr_format!("Repository error: bundle map error\n\tcaused by: {}", err))
        }
        InodeIntegrity(err: InodeIntegrityError) {
            from()
            cause(err)
            description(tr!("Integrity error"))
            display("{}", tr_format!("Repository error: integrity error\n\tcaused by: {}", err))
        }
        Integrity(err: IntegrityError) {
            from()
            cause(err)
            description(tr!("Integrity error"))
            display("{}", tr_format!("Repository error: integrity error\n\tcaused by: {}", err))
        }
        Dirty {
            description(tr!("Dirty repository"))
            display("{}", tr_format!("The repository is dirty, please run a check"))
        }
        Backup(err: BackupError) {
            from()
            cause(err)
            description(tr!("Failed to create a backup"))
            display("{}", tr_format!("Repository error: failed to create backup\n\tcaused by: {}", err))
        }
        Lock(err: LockError) {
            from()
            cause(err)
            description(tr!("Failed to obtain lock"))
            display("{}", tr_format!("Repository error: failed to obtain lock\n\tcaused by: {}", err))
        }

        Io(err: io::Error) {
            from()
            cause(err)
            description(tr!("IO error"))
            display("{}", tr_format!("IO error: {}", err))
        }
        NoSuchFileInBackup(backup: BackupFile, path: PathBuf) {
            description(tr!("No such file in backup"))
            display("{}", tr_format!("The backup does not contain the file {:?}", path))
        }
    }
}
