use std::io;
use std::path::PathBuf;

use rmp_serde::decode::Error as MsgpackDecode;
use rmp_serde::encode::Error as MsgpackEncode;

use super::bundle::BundleId;

quick_error!{
    #[derive(Debug)]
    pub enum BundleError {
        List(err: io::Error) {
            cause(err)
            description("Failed to list bundles")
        }
        Read(err: io::Error, path: PathBuf, reason: &'static str) {
            cause(err)
            description("Failed to read bundle")
            display("Failed to read bundle {:?}: {}", path, reason)
        }
        Decode(err: MsgpackDecode, path: PathBuf) {
            cause(err)
            description("Failed to decode bundle header")
        }
        Write(err: io::Error, path: PathBuf, reason: &'static str) {
            cause(err)
            description("Failed to write bundle")
            display("Failed to write bundle {:?}: {}", path, reason)
        }
        Encode(err: MsgpackEncode, path: PathBuf) {
            cause(err)
            description("Failed to encode bundle header")
        }
        Format(path: PathBuf, reason: &'static str) {
            description("Failed to decode bundle")
            display("Failed to decode bundle {:?}: {}", path, reason)
        }
        Integrity(bundle: BundleId, reason: &'static str) {
            description("Bundle has an integrity error")
            display("Bundle {:?} has an integrity error: {}", bundle, reason)
        }
        Remove(err: io::Error, bundle: BundleId) {
            cause(err)
            description("Failed to remove bundle")
            display("Failed to remove bundle {}", bundle)
        }
        Custom {
            from(&'static str)
            description("Custom error")
        }
    }
}

quick_error!{
    #[derive(Debug)]
    pub enum ChunkerError {
        Read(err: io::Error) {
            from(err)
            cause(err)
            description("Failed to read")
        }
        Write(err: io::Error) {
            from(err)
            cause(err)
            description("Failed to write")
        }
        Custom {
            from(&'static str)
            description("Custom error")
        }
    }
}
