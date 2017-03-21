use std::io;
use std::path::PathBuf;

use super::backup::{Backup, BackupError};
use super::bundle_map::BundleMapError;
use super::config::ConfigError;
use super::integrity::RepositoryIntegrityError;
use ::index::IndexError;
use ::bundledb::BundleError;
use ::chunker::ChunkerError;
use ::util::*;


quick_error!{
    #[derive(Debug)]
    pub enum RepositoryError {
        Io(err: io::Error) {
            from()
            cause(err)
            description("IO error")
            display("IO error: {}", err)
        }
        Config(err: ConfigError) {
            from()
            cause(err)
            description("Configuration error")
            display("Configuration error: {}", err)
        }
        BundleMap(err: BundleMapError) {
            from()
            cause(err)
            description("Bundle map error")
            display("Bundle map error: {}", err)
        }
        Index(err: IndexError) {
            from()
            cause(err)
            description("Index error")
            display("Index error: {}", err)
        }
        Bundle(err: BundleError) {
            from()
            cause(err)
            description("Bundle error")
            display("Bundle error: {}", err)
        }
        Backup(err: BackupError) {
            from()
            cause(err)
            description("Backup error")
            display("Backup error: {}", err)
        }
        Chunker(err: ChunkerError) {
            from()
            cause(err)
            description("Chunker error")
            display("Chunker error: {}", err)
        }
        Decode(err: msgpack::DecodeError) {
            from()
            cause(err)
            description("Failed to decode metadata")
            display("Failed to decode metadata: {}", err)
        }
        Encode(err: msgpack::EncodeError) {
            from()
            cause(err)
            description("Failed to encode metadata")
            display("Failed to encode metadata: {}", err)
        }
        Integrity(err: RepositoryIntegrityError) {
            from()
            cause(err)
            description("Integrity error")
            display("Integrity error: {}", err)
        }
        Encryption(err: EncryptionError) {
            from()
            cause(err)
            description("Failed to load keys")
            display("Failed to load keys: {}", err)
        }
        InvalidFileType(path: PathBuf) {
            description("Invalid file type")
            display("{:?} has an invalid file type", path)
        }
        NoSuchFileInBackup(backup: Backup, path: PathBuf) {
            description("No such file in backup")
            display("The backup does not contain the file {:?}", path)
        }
        UnsafeVacuum {
            description("Not all backups can be read, refusing to run vacuum")
        }
    }
}
