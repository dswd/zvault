use prelude::*;

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write, Read};


pub static CACHE_FILE_STRING: [u8; 7] = *b"zvault\x04";
pub static CACHE_FILE_VERSION: u8 = 1;


quick_error!{
    #[derive(Debug)]
    pub enum BundleCacheError {
        Read(err: io::Error) {
            cause(err)
            description(tr!("Failed to read bundle cache"))
            display("{}", tr_format!("Bundle cache error: failed to read bundle cache\n\tcaused by: {}", err))
        }
        Write(err: io::Error) {
            cause(err)
            description(tr!("Failed to write bundle cache"))
            display("{}", tr_format!("Bundle cache error: failed to write bundle cache\n\tcaused by: {}", err))
        }
        WrongHeader {
            description(tr!("Wrong header"))
            display("{}", tr_format!("Bundle cache error: wrong header on bundle cache"))
        }
        UnsupportedVersion(version: u8) {
            description(tr!("Wrong version"))
            display("{}", tr_format!("Bundle cache error: unsupported version: {}", version))
        }
        Decode(err: msgpack::DecodeError) {
            from()
            cause(err)
            description(tr!("Failed to decode bundle cache"))
            display("{}", tr_format!("Bundle cache error: failed to decode bundle cache\n\tcaused by: {}", err))
        }
        Encode(err: msgpack::EncodeError) {
            from()
            cause(err)
            description(tr!("Failed to encode bundle cache"))
            display("{}", tr_format!("Bundle cache error: failed to encode bundle cache\n\tcaused by: {}", err))
        }
    }
}


#[derive(Clone, Default)]
pub struct StoredBundle {
    pub info: BundleInfo,
    pub path: PathBuf
}
serde_impl!(StoredBundle(u64) {
    info: BundleInfo => 0,
    path: PathBuf => 1
});

impl StoredBundle {
    #[inline]
    pub fn id(&self) -> BundleId {
        self.info.id.clone()
    }

    pub fn copy_to<P: AsRef<Path>>(
        &self,
        base_path: &Path,
        path: P,
    ) -> Result<Self, BundleDbError> {
        let src_path = base_path.join(&self.path);
        let dst_path = path.as_ref();
        try!(fs::copy(&src_path, dst_path).context(dst_path));
        let mut bundle = self.clone();
        bundle.path = dst_path.strip_prefix(base_path).unwrap().to_path_buf();
        Ok(bundle)
    }

    pub fn move_to<P: AsRef<Path>>(
        &mut self,
        base_path: &Path,
        path: P,
    ) -> Result<(), BundleDbError> {
        let src_path = base_path.join(&self.path);
        let dst_path = path.as_ref();
        if fs::rename(&src_path, dst_path).is_err() {
            try!(fs::copy(&src_path, dst_path).context(dst_path));
            try!(fs::remove_file(&src_path).context(&src_path as &Path));
        }
        self.path = dst_path.strip_prefix(base_path).unwrap().to_path_buf();
        Ok(())
    }

    pub fn read_list_from<P: AsRef<Path>>(path: P) -> Result<Vec<Self>, BundleCacheError> {
        let path = path.as_ref();
        let mut file = BufReader::new(try!(File::open(path).map_err(BundleCacheError::Read)));
        let mut header = [0u8; 8];
        try!(file.read_exact(&mut header).map_err(BundleCacheError::Read));
        if header[..CACHE_FILE_STRING.len()] != CACHE_FILE_STRING {
            return Err(BundleCacheError::WrongHeader);
        }
        let version = header[CACHE_FILE_STRING.len()];
        if version != CACHE_FILE_VERSION {
            return Err(BundleCacheError::UnsupportedVersion(version));
        }
        Ok(try!(msgpack::decode_from_stream(&mut file)))
    }

    pub fn save_list_to<P: AsRef<Path>>(list: &[Self], path: P) -> Result<(), BundleCacheError> {
        let path = path.as_ref();
        let mut file = BufWriter::new(try!(File::create(path).map_err(BundleCacheError::Write)));
        try!(file.write_all(&CACHE_FILE_STRING).map_err(
            BundleCacheError::Write
        ));
        try!(file.write_all(&[CACHE_FILE_VERSION]).map_err(
            BundleCacheError::Write
        ));
        try!(msgpack::encode_to_stream(&list, &mut file));
        Ok(())
    }
}
