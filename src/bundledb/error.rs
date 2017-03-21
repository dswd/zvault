use std::path::{Path, PathBuf};
use std::io;

use util::*;
use super::*;

quick_error!{
    #[derive(Debug)]
    pub enum BundleError {
        List(err: io::Error) {
            cause(err)
            description("Failed to list bundles")
            display("Failed to list bundles: {}", err)
        }
        Io(err: io::Error, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: io::Error) -> (err, path.to_path_buf())
            description("Failed to read/write bundle")
            display("Failed to read/write bundle {:?}: {}", path, err)
        }
        Decode(err: msgpack::DecodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::DecodeError) -> (err, path.to_path_buf())
            description("Failed to decode bundle header")
            display("Failed to decode bundle header of {:?}: {}", path, err)
        }
        Encode(err: msgpack::EncodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::EncodeError) -> (err, path.to_path_buf())
            description("Failed to encode bundle header")
            display("Failed to encode bundle header of {:?}: {}", path, err)
        }
        WrongHeader(path: PathBuf) {
            description("Wrong header")
            display("Wrong header on bundle {:?}", path)
        }
        WrongVersion(path: PathBuf, version: u8) {
            description("Wrong version")
            display("Wrong version on bundle {:?}: {}", path, version)
        }
        Integrity(bundle: BundleId, reason: &'static str) {
            description("Bundle has an integrity error")
            display("Bundle {:?} has an integrity error: {}", bundle, reason)
        }
        NoSuchBundle(bundle: BundleId) {
            description("No such bundle")
            display("No such bundle: {:?}", bundle)
        }
        NoSuchChunk(bundle: BundleId, id: usize) {
            description("Bundle has no such chunk")
            display("Bundle {:?} has no chunk with that id: {}", bundle, id)
        }
        Decompression(err: CompressionError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: CompressionError) -> (err, path.to_path_buf())
            description("Decompression failed")
            display("Decompression failed on bundle {:?}: {}", path, err)
        }
        Compression(err: CompressionError) {
            from()
            cause(err)
            description("Compression failed")
            display("Compression failed: {}", err)
        }
        Decryption(err: EncryptionError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: EncryptionError) -> (err, path.to_path_buf())
            description("Decryption failed")
            display("Decryption failed on bundle {:?}: {}", path, err)
        }
        Encryption(err: EncryptionError) {
            from()
            cause(err)
            description("Encryption failed")
            display("Encryption failed: {}", err)
        }
        Remove(err: io::Error, bundle: BundleId) {
            cause(err)
            description("Failed to remove bundle")
            display("Failed to remove bundle {}", bundle)
        }
    }
}
