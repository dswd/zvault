use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io;
use std::fs::{self, File};

use serde_yaml;
use serde::bytes::ByteBuf;

use sodiumoxide::crypto::sealedbox;
pub use sodiumoxide::crypto::box_::{SecretKey, PublicKey, gen_keypair};

use ::util::*;


quick_error!{
    #[derive(Debug)]
    pub enum EncryptionError {
        InvalidKey {
            description("Invalid key")
        }
        MissingKey(key: PublicKey) {
            description("Missing key")
            display("Missing key: {}", to_hex(&key[..]))
        }
        Operation(reason: &'static str) {
            description("Operation failed")
            display("Operation failed: {}", reason)
        }
        Io(err: io::Error) {
            from()
            cause(err)
            description("IO error")
            display("IO error: {}", err)
        }
        Yaml(err: serde_yaml::Error) {
            from()
            cause(err)
            description("Yaml format error")
            display("Yaml format error: {}", err)
        }
    }
}


#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[allow(unknown_lints,non_camel_case_types)]
pub enum EncryptionMethod {
    Sodium,
}
serde_impl!(EncryptionMethod(u64) {
    Sodium => 0
});

impl EncryptionMethod {
    pub fn from_string(val: &str) -> Result<Self, &'static str> {
        match val {
            "sodium" => Ok(EncryptionMethod::Sodium),
            _ => Err("Unsupported encryption method")
        }
    }

    pub fn to_string(&self) -> String {
        match *self {
            EncryptionMethod::Sodium => "sodium".to_string()
        }
    }
}


pub type Encryption = (EncryptionMethod, ByteBuf);


struct KeyfileYaml {
    public: String,
    secret: String
}
impl Default for KeyfileYaml {
    fn default() -> Self {
        KeyfileYaml {
            public: "".to_string(),
            secret: "".to_string()
        }
    }
}
serde_impl!(KeyfileYaml(String) {
    public: String => "public",
    secret: String => "secret"
});

impl KeyfileYaml {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, EncryptionError> {
        let f = try!(File::open(path));
        Ok(try!(serde_yaml::from_reader(f)))
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), EncryptionError> {
        let mut f = try!(File::create(path));
        Ok(try!(serde_yaml::to_writer(&mut f, &self)))
    }
}


pub struct Crypto {
    path: PathBuf,
    keys: HashMap<PublicKey, SecretKey>
}

impl Crypto {
    #[inline]
    pub fn dummy() -> Self {
        Crypto { path: PathBuf::new(), keys: HashMap::new() }
    }

    #[inline]
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, EncryptionError> {
        let path = path.as_ref().to_owned();
        let mut keys: HashMap<PublicKey, SecretKey> = HashMap::default();
        for entry in try!(fs::read_dir(&path)) {
            let entry = try!(entry);
            let keyfile = try!(KeyfileYaml::load(entry.path()));
            let public = try!(parse_hex(&keyfile.public).map_err(|_| EncryptionError::InvalidKey));
            let public = try!(PublicKey::from_slice(&public).ok_or(EncryptionError::InvalidKey));
            let secret = try!(parse_hex(&keyfile.secret).map_err(|_| EncryptionError::InvalidKey));
            let secret = try!(SecretKey::from_slice(&secret).ok_or(EncryptionError::InvalidKey));
            keys.insert(public, secret);
        }
        Ok(Crypto { path: path, keys: keys })
    }

    #[inline]
    pub fn add_secret_key(&mut self, public: PublicKey, secret: SecretKey) {
        self.keys.insert(public, secret);
    }

    #[inline]
    pub fn register_keyfile<P: AsRef<Path>>(&mut self, path: P) -> Result<(), EncryptionError> {
        let (public, secret) = try!(Self::load_keypair_from_file(path));
        self.register_secret_key(public, secret)
    }

    #[inline]
    pub fn load_keypair_from_file<P: AsRef<Path>>(path: P) -> Result<(PublicKey, SecretKey), EncryptionError> {
        let keyfile = try!(KeyfileYaml::load(path));
        let public = try!(parse_hex(&keyfile.public).map_err(|_| EncryptionError::InvalidKey));
        let public = try!(PublicKey::from_slice(&public).ok_or(EncryptionError::InvalidKey));
        let secret = try!(parse_hex(&keyfile.secret).map_err(|_| EncryptionError::InvalidKey));
        let secret = try!(SecretKey::from_slice(&secret).ok_or(EncryptionError::InvalidKey));
        Ok((public, secret))
    }

    #[inline]
    pub fn save_keypair_to_file<P: AsRef<Path>>(public: &PublicKey, secret: &SecretKey, path: P) -> Result<(), EncryptionError> {
        KeyfileYaml { public: to_hex(&public[..]), secret: to_hex(&secret[..]) }.save(path)
    }

    #[inline]
    pub fn register_secret_key(&mut self, public: PublicKey, secret: SecretKey) -> Result<(), EncryptionError> {
        let path = self.path.join(to_hex(&public[..]) + ".yaml");
        try!(Self::save_keypair_to_file(&public, &secret, path));
        self.keys.insert(public, secret);
        Ok(())
    }

    #[inline]
    pub fn contains_secret_key(&mut self, public: &PublicKey) -> bool {
        self.keys.contains_key(public)
    }

    fn get_secret_key(&self, public: &PublicKey) -> Result<&SecretKey, EncryptionError> {
        self.keys.get(public).ok_or_else(|| EncryptionError::MissingKey(*public))
    }

    #[inline]
    pub fn encrypt(&self, enc: &Encryption, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let &(ref method, ref public) = enc;
        let public = try!(PublicKey::from_slice(public).ok_or(EncryptionError::InvalidKey));
        match *method {
            EncryptionMethod::Sodium => {
                Ok(sealedbox::seal(data, &public))
            }
        }
    }

    #[inline]
    pub fn decrypt(&self, enc: &Encryption, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let &(ref method, ref public) = enc;
        let public = try!(PublicKey::from_slice(public).ok_or(EncryptionError::InvalidKey));
        let secret = try!(self.get_secret_key(&public));
        match *method {
            EncryptionMethod::Sodium => {
                sealedbox::open(data, &public, secret).map_err(|_| EncryptionError::Operation("Decryption failed"))
            }
        }
    }
}
