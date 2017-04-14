use ::prelude::*;

use std::io::{self, BufReader, BufWriter, Read, Write};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::collections::HashMap;


static HEADER_STRING: [u8; 7] = *b"zvault\x03";
static HEADER_VERSION: u8 = 1;


quick_error!{
    #[derive(Debug)]
    pub enum BackupFileError {
        Read(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to read backup")
            display("Backup file error: failed to read backup file {:?}\n\tcaused by: {}", path, err)
        }
        Write(err: io::Error, path: PathBuf) {
            cause(err)
            description("Failed to write backup")
            display("Backup file error: failed to write backup file {:?}\n\tcaused by: {}", path, err)
        }
        Decode(err: msgpack::DecodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::DecodeError) -> (err, path.to_path_buf())
            description("Failed to decode backup")
            display("Backup file error: failed to decode backup of {:?}\n\tcaused by: {}", path, err)
        }
        Encode(err: msgpack::EncodeError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: msgpack::EncodeError) -> (err, path.to_path_buf())
            description("Failed to encode backup")
            display("Backup file error: failed to encode backup of {:?}\n\tcaused by: {}", path, err)
        }
        WrongHeader(path: PathBuf) {
            description("Wrong header")
            display("Backup file error: wrong header on backup {:?}", path)
        }
        UnsupportedVersion(path: PathBuf, version: u8) {
            description("Wrong version")
            display("Backup file error: unsupported version on backup {:?}: {}", path, version)
        }
        Decryption(err: EncryptionError, path: PathBuf) {
            cause(err)
            context(path: &'a Path, err: EncryptionError) -> (err, path.to_path_buf())
            description("Decryption failed")
            display("Backup file error: decryption failed on backup {:?}\n\tcaused by: {}", path, err)
        }
        Encryption(err: EncryptionError) {
            from()
            cause(err)
            description("Encryption failed")
            display("Backup file error: encryption failed\n\tcaused by: {}", err)
        }
        PartialBackupsList(partial: HashMap<String, Backup>, failed: Vec<PathBuf>) {
            description("Some backups could not be loaded")
            display("Backup file error: some backups could not be loaded: {:?}", failed)
        }
    }
}

#[derive(Default, Debug, Clone)]
struct BackupHeader {
    pub encryption: Option<Encryption>
}
serde_impl!(BackupHeader(u8) {
    encryption: Option<Encryption> => 0
});


#[derive(Default, Debug, Clone)]
pub struct Backup {
    pub root: ChunkList,
    pub total_data_size: u64, // Sum of all raw sizes of all entities
    pub changed_data_size: u64, // Sum of all raw sizes of all entities actively stored
    pub deduplicated_data_size: u64, // Sum of all raw sizes of all new bundles
    pub encoded_data_size: u64, // Sum al all encoded sizes of all new bundles
    pub bundle_count: usize,
    pub chunk_count: usize,
    pub avg_chunk_size: f32,
    pub timestamp: i64,
    pub duration: f32,
    pub file_count: usize,
    pub dir_count: usize,
    pub host: String,
    pub path: String,
    pub config: Config,
    pub modified: bool,
    pub user_names: HashMap<u32, String>,
    pub group_names: HashMap<u32, String>
}
serde_impl!(Backup(u8?) {
    root: ChunkList => 0,
    total_data_size: u64 => 1,
    changed_data_size: u64 => 2,
    deduplicated_data_size: u64 => 3,
    encoded_data_size: u64 => 4,
    bundle_count: usize => 5,
    chunk_count: usize => 6,
    avg_chunk_size: f32 => 7,
    timestamp: i64 => 8,
    duration: f32 => 9,
    file_count: usize => 10,
    dir_count: usize => 11,
    host: String => 12,
    path: String => 13,
    config: Config => 14,
    modified: bool => 15,
    user_names: HashMap<u32, String> => 16,
    group_names: HashMap<u32, String> => 17
});

impl Backup {
    pub fn read_from<P: AsRef<Path>>(crypto: &Crypto, path: P) -> Result<Self, BackupFileError> {
        let path = path.as_ref();
        let mut file = BufReader::new(try!(File::open(path).map_err(|err| BackupFileError::Read(err, path.to_path_buf()))));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header).map_err(|err| BackupFileError::Read(err, path.to_path_buf())));
        if header[..HEADER_STRING.len()] != HEADER_STRING {
            return Err(BackupFileError::WrongHeader(path.to_path_buf()))
        }
        let version = header[HEADER_STRING.len()];
        if version != HEADER_VERSION {
            return Err(BackupFileError::UnsupportedVersion(path.to_path_buf(), version))
        }
        let header: BackupHeader = try!(msgpack::decode_from_stream(&mut file).context(path));
        let mut data = Vec::new();
        try!(file.read_to_end(&mut data).map_err(|err| BackupFileError::Read(err, path.to_path_buf())));
        if let Some(ref encryption) = header.encryption {
            data = try!(crypto.decrypt(encryption, &data));
        }
        Ok(try!(msgpack::decode(&data).context(path)))
    }

    pub fn save_to<P: AsRef<Path>>(&self, crypto: &Crypto, encryption: Option<Encryption>, path: P) -> Result<(), BackupFileError> {
        let path = path.as_ref();
        let mut data = try!(msgpack::encode(self).context(path));
        if let Some(ref encryption) = encryption {
            data = try!(crypto.encrypt(encryption, &data));
        }
        let mut file = BufWriter::new(try!(File::create(path).map_err(|err| BackupFileError::Write(err, path.to_path_buf()))));
        try!(file.write_all(&HEADER_STRING).map_err(|err| BackupFileError::Write(err, path.to_path_buf())));
        try!(file.write_all(&[HEADER_VERSION]).map_err(|err| BackupFileError::Write(err, path.to_path_buf())));
        let header = BackupHeader { encryption: encryption };
        try!(msgpack::encode_to_stream(&header, &mut file).context(path));
        try!(file.write_all(&data).map_err(|err| BackupFileError::Write(err, path.to_path_buf())));
        Ok(())
    }

    pub fn get_all_from<P: AsRef<Path>>(crypto: &Crypto, path: P) -> Result<HashMap<String, Backup>, BackupFileError> {
        let mut backups = HashMap::new();
        let base_path = path.as_ref();
        let path = path.as_ref();
        if !path.exists() {
            debug!("Backup root folder does not exist");
            return Ok(backups);
        }
        let mut paths = vec![path.to_path_buf()];
        let mut failed_paths = vec![];
        while let Some(path) = paths.pop() {
            for entry in try!(fs::read_dir(&path).map_err(|e| BackupFileError::Read(e, path.clone()))) {
                let entry = try!(entry.map_err(|e| BackupFileError::Read(e, path.clone())));
                let path = entry.path();
                if path.is_dir() {
                    paths.push(path);
                } else {
                    let relpath = path.strip_prefix(&base_path).unwrap();
                    if relpath.extension() != Some("backup".as_ref()) {
                        continue
                    }
                    let name = relpath.with_file_name(relpath.file_stem().unwrap()).to_string_lossy().to_string();
                    if let Ok(backup) = Backup::read_from(crypto, &path) {
                        backups.insert(name, backup);
                    } else {
                        failed_paths.push(path.clone());
                    }
                }
            }
        }
        if failed_paths.is_empty() {
            Ok(backups)
        } else {
            Err(BackupFileError::PartialBackupsList(backups, failed_paths))
        }
    }
}
