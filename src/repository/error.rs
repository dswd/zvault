use std::io;
use std::path::PathBuf;

use super::bundle_map::BundleMapError;
use super::config::ConfigError;
use super::integrity::RepositoryIntegrityError;
use ::index::IndexError;
use ::bundle::BundleError;
use ::chunker::ChunkerError;
use ::util::*;


quick_error!{
    #[derive(Debug)]
    pub enum RepositoryError {
        Io(err: io::Error) {
            from()
            cause(err)
            description("IO Error")
        }
        Config(err: ConfigError) {
            from()
            cause(err)
            description("Configuration error")
        }
        BundleMap(err: BundleMapError) {
            from()
            cause(err)
            description("Bundle map error")
        }
        Index(err: IndexError) {
            from()
            cause(err)
            description("Index error")
        }
        Bundle(err: BundleError) {
            from()
            cause(err)
            description("Bundle error")
        }
        Chunker(err: ChunkerError) {
            from()
            cause(err)
            description("Chunker error")
        }
        Decode(err: msgpack::DecodeError) {
            from()
            cause(err)
            description("Failed to decode metadata")
        }
        Encode(err: msgpack::EncodeError) {
            from()
            cause(err)
            description("Failed to encode metadata")
        }
        Integrity(err: RepositoryIntegrityError) {
            from()
            cause(err)
            description("Integrity error")
        }
        InvalidFileType(path: PathBuf) {
            description("Invalid file type")
            display("{:?} has an invalid file type", path)
        }
    }
}
