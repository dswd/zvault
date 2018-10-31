use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io;
use std::fs::{self, File};
use std::sync::{RwLock, Once, ONCE_INIT};

use serde_yaml;
use serde_bytes::ByteBuf;

use sodiumoxide;
use sodiumoxide::crypto::sealedbox;
use sodiumoxide::crypto::box_;
use sodiumoxide::crypto::box_::curve25519xsalsa20poly1305::{keypair_from_seed, Seed};
use sodiumoxide::crypto::pwhash;
pub use sodiumoxide::crypto::box_::{SecretKey, PublicKey};

use util::*;


static INIT: Once = ONCE_INIT;

fn sodium_init() {
    INIT.call_once(|| if sodiumoxide::init().is_err() {
        tr_panic!("Failed to initialize sodiumoxide");
    });
}

quick_error!{
    #[derive(Debug)]
    pub enum EncryptionError {
        InvalidKey {
            description(tr!("Invalid key"))
        }
        MissingKey(key: PublicKey) {
            description(tr!("Missing key"))
            display("{}", tr_format!("Missing key: {}", to_hex(&key[..])))
        }
        Operation(reason: &'static str) {
            description(tr!("Operation failed"))
            display("{}", tr_format!("Operation failed: {}", reason))
        }
        Io(err: io::Error) {
            from()
            cause(err)
            description(tr!("IO error"))
            display("{}", tr_format!("IO error: {}", err))
        }
        Yaml(err: serde_yaml::Error) {
            from()
            cause(err)
            description(tr!("Yaml format error"))
            display("{}", tr_format!("Yaml format error: {}", err))
        }
    }
}


#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[allow(clippy::non_camel_case_types)]
pub enum EncryptionMethod {
    Sodium
}
serde_impl!(EncryptionMethod(u64) {
    Sodium => 0
});

impl EncryptionMethod {
    pub fn from_string(val: &str) -> Result<Self, &'static str> {
        match val {
            "sodium" => Ok(EncryptionMethod::Sodium),
            _ => Err(tr!("Unsupported encryption method")),
        }
    }

    pub fn to_string(&self) -> String {
        match *self {
            EncryptionMethod::Sodium => "sodium".to_string(),
        }
    }
}


pub type Encryption = (EncryptionMethod, ByteBuf);


pub struct KeyfileYaml {
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
        try!(serde_yaml::to_writer(&mut f, &self));
        Ok(())
    }
}


pub struct Crypto {
    path: Option<PathBuf>,
    keys: RwLock<HashMap<PublicKey, SecretKey>>
}

impl Crypto {
    #[inline]
    pub fn dummy() -> Self {
        sodium_init();
        Crypto {
            path: None,
            keys: RwLock::new(HashMap::new())
        }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, EncryptionError> {
        sodium_init();
        let path = path.as_ref().to_owned();
        let mut keys: HashMap<PublicKey, SecretKey> = HashMap::default();
        for entry in try!(fs::read_dir(&path)) {
            let entry = try!(entry);
            let keyfile = try!(KeyfileYaml::load(entry.path()));
            let public = try!(parse_hex(&keyfile.public).map_err(
                |_| EncryptionError::InvalidKey
            ));
            let public = try!(PublicKey::from_slice(&public).ok_or(
                EncryptionError::InvalidKey
            ));
            let secret = try!(parse_hex(&keyfile.secret).map_err(
                |_| EncryptionError::InvalidKey
            ));
            let secret = try!(SecretKey::from_slice(&secret).ok_or(
                EncryptionError::InvalidKey
            ));
            keys.insert(public, secret);
        }
        Ok(Crypto {
            path: Some(path),
            keys: RwLock::new(keys)
        })
    }

    #[inline]
    pub fn add_secret_key(&self, public: PublicKey, secret: SecretKey) {
        self.keys.write().expect("Lock poisoned").insert(public, secret);
    }

    #[inline]
    pub fn register_keyfile<P: AsRef<Path>>(&self, path: P) -> Result<(), EncryptionError> {
        let (public, secret) = try!(Self::load_keypair_from_file(path));
        self.register_secret_key(public, secret)
    }

    #[inline]
    pub fn load_keypair_from_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<(PublicKey, SecretKey), EncryptionError> {
        Self::load_keypair_from_file_data(&try!(KeyfileYaml::load(path)))
    }

    pub fn load_keypair_from_file_data(
        keyfile: &KeyfileYaml,
    ) -> Result<(PublicKey, SecretKey), EncryptionError> {
        let public = try!(parse_hex(&keyfile.public).map_err(
            |_| EncryptionError::InvalidKey
        ));
        let public = try!(PublicKey::from_slice(&public).ok_or(
            EncryptionError::InvalidKey
        ));
        let secret = try!(parse_hex(&keyfile.secret).map_err(
            |_| EncryptionError::InvalidKey
        ));
        let secret = try!(SecretKey::from_slice(&secret).ok_or(
            EncryptionError::InvalidKey
        ));
        Ok((public, secret))
    }

    #[inline]
    pub fn save_keypair_to_file_data(public: &PublicKey, secret: &SecretKey) -> KeyfileYaml {
        KeyfileYaml {
            public: to_hex(&public[..]),
            secret: to_hex(&secret[..])
        }
    }

    #[inline]
    pub fn save_keypair_to_file<P: AsRef<Path>>(
        public: &PublicKey,
        secret: &SecretKey,
        path: P,
    ) -> Result<(), EncryptionError> {
        Self::save_keypair_to_file_data(public, secret).save(path)
    }

    #[inline]
    pub fn register_secret_key(
        &self,
        public: PublicKey,
        secret: SecretKey,
    ) -> Result<(), EncryptionError> {
        if let Some(ref path) = self.path {
            let path = path.join(to_hex(&public[..]) + ".yaml");
            try!(Self::save_keypair_to_file(&public, &secret, path));
        }
        self.keys.write().expect("Lock poisoned").insert(public, secret);
        Ok(())
    }

    #[inline]
    pub fn contains_secret_key(&self, public: &PublicKey) -> bool {
        self.keys.read().expect("Lock poisoned").contains_key(public)
    }

    fn get_secret_key(&self, public: &PublicKey) -> Result<SecretKey, EncryptionError> {
        self.keys.read().expect("Lock poisoned").get(public).cloned().ok_or_else(
            || EncryptionError::MissingKey(*public)
        )
    }

    #[inline]
    pub fn encrypt(&self, enc: &Encryption, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let &(ref method, ref public) = enc;
        let public = try!(PublicKey::from_slice(public).ok_or(
            EncryptionError::InvalidKey
        ));
        match *method {
            EncryptionMethod::Sodium => Ok(sealedbox::seal(data, &public)),
        }
    }

    #[inline]
    pub fn decrypt(&self, enc: &Encryption, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let &(ref method, ref public) = enc;
        let public = try!(PublicKey::from_slice(public).ok_or(
            EncryptionError::InvalidKey
        ));
        let secret = try!(self.get_secret_key(&public));
        match *method {
            EncryptionMethod::Sodium => {
                sealedbox::open(data, &public, &secret).map_err(|_| {
                    EncryptionError::Operation(tr!("Decryption failed"))
                })
            }
        }
    }

    #[inline]
    pub fn gen_keypair() -> (PublicKey, SecretKey) {
        sodium_init();
        box_::gen_keypair()
    }

    pub fn keypair_from_password(password: &str) -> (PublicKey, SecretKey) {
        let salt = pwhash::Salt::from_slice(b"the_great_zvault_password_salt_1").unwrap();
        let mut key = [0u8; pwhash::HASHEDPASSWORDBYTES];
        let key = pwhash::derive_key(
            &mut key,
            password.as_bytes(),
            &salt,
            pwhash::OPSLIMIT_INTERACTIVE,
            pwhash::MEMLIMIT_INTERACTIVE
        ).unwrap();
        let seed = if let Some(seed) = Seed::from_slice(&key[key.len()-32..]) {
            seed
        } else {
            tr_panic!("Seed failed");
        };
        keypair_from_seed(&seed)
    }
}



mod tests {

    #[allow(unused_imports)]
    use super::*;


    #[test]
    fn test_gen_keypair() {
        let key1 = Crypto::gen_keypair();
        let key2 = Crypto::gen_keypair();
        assert!(key1.0 != key2.0);
    }

    #[test]
    fn test_keypair_from_password() {
        let key1 = Crypto::keypair_from_password("foo");
        let key2 = Crypto::keypair_from_password("foo");
        assert_eq!(key1.0, key2.0);
        let key3 = Crypto::keypair_from_password("bar");
        assert!(key1.0 != key3.0);
    }

    #[test]
    fn test_add_keypair() {
        let mut crypto = Crypto::dummy();
        let (pk, sk) = Crypto::gen_keypair();
        assert!(!crypto.contains_secret_key(&pk));
        crypto.add_secret_key(pk, sk);
        assert!(crypto.contains_secret_key(&pk));
    }

    #[test]
    fn test_save_load_keyfile() {
        let (pk, sk) = Crypto::gen_keypair();
        let data = Crypto::save_keypair_to_file_data(&pk, &sk);
        let res = Crypto::load_keypair_from_file_data(&data);
        assert!(res.is_ok());
        let (pk2, sk2) = res.unwrap();
        assert_eq!(pk, pk2);
        assert_eq!(sk, sk2);
    }

    #[test]
    fn test_encrypt_decrpyt() {
        let mut crypto = Crypto::dummy();
        let (pk, sk) = Crypto::gen_keypair();
        crypto.add_secret_key(pk, sk);
        let encryption = (EncryptionMethod::Sodium, ByteBuf::from(&pk[..]));
        let cleartext = b"test123";
        let result = crypto.encrypt(&encryption, cleartext);
        assert!(result.is_ok());
        let ciphertext = result.unwrap();
        assert!(&ciphertext != cleartext);
        let result = crypto.decrypt(&encryption, &ciphertext);
        assert!(result.is_ok());
        let unciphered = result.unwrap();
        assert_eq!(&cleartext[..] as &[u8], &unciphered as &[u8]);
    }

    #[test]
    fn test_wrong_key() {
        let mut crypto = Crypto::dummy();
        let (pk, sk) = Crypto::gen_keypair();
        crypto.add_secret_key(pk, sk.clone());
        let encryption = (EncryptionMethod::Sodium, ByteBuf::from(&pk[..]));
        let cleartext = b"test123";
        let result = crypto.encrypt(&encryption, cleartext);
        assert!(result.is_ok());
        let ciphertext = result.unwrap();
        assert!(&ciphertext != cleartext);
        let mut crypto2 = Crypto::dummy();
        let mut sk2 = sk[..].to_vec();
        sk2[4] ^= 53;
        assert!(&sk[..] as &[u8] != &sk2[..] as &[u8]);
        crypto2.add_secret_key(pk, SecretKey::from_slice(&sk2).unwrap());
        let result = crypto2.decrypt(&encryption, &ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_modified_ciphertext() {
        let mut crypto = Crypto::dummy();
        let (pk, sk) = Crypto::gen_keypair();
        crypto.add_secret_key(pk, sk.clone());
        let encryption = (EncryptionMethod::Sodium, ByteBuf::from(&pk[..]));
        let cleartext = b"test123";
        let result = crypto.encrypt(&encryption, cleartext);
        assert!(result.is_ok());
        let mut ciphertext = result.unwrap();
        assert!(&ciphertext != cleartext);
        ciphertext[4] ^= 53;
        let result = crypto.decrypt(&encryption, &ciphertext);
        assert!(result.is_err());

    }

}



#[cfg(feature = "bench")]
mod benches {

    #[allow(unused_imports)]
    use super::*;

    use test::Bencher;


    #[allow(dead_code, clippy::needless_range_loop)]
    fn test_data(n: usize) -> Vec<u8> {
        let mut input = vec![0; n];
        for i in 0..input.len() {
            input[i] = (i * i * i) as u8;
        }
        input
    }

    #[bench]
    fn bench_key_generate(b: &mut Bencher) {
        b.iter(|| Crypto::gen_keypair());
    }

    #[bench]
    fn bench_encrypt(b: &mut Bencher) {
        let mut crypto = Crypto::dummy();
        let (pk, sk) = Crypto::gen_keypair();
        crypto.add_secret_key(pk, sk.clone());
        let encryption = (EncryptionMethod::Sodium, ByteBuf::from(&pk[..]));
        let input = test_data(512 * 1024);
        b.iter(|| crypto.encrypt(&encryption, &input));
        b.bytes = input.len() as u64;
    }

    #[bench]
    fn bench_decrypt(b: &mut Bencher) {
        let mut crypto = Crypto::dummy();
        let (pk, sk) = Crypto::gen_keypair();
        crypto.add_secret_key(pk, sk.clone());
        let encryption = (EncryptionMethod::Sodium, ByteBuf::from(&pk[..]));
        let input = test_data(512 * 1024);
        let output = crypto.encrypt(&encryption, &input).unwrap();
        b.iter(|| crypto.decrypt(&encryption, &output));
        b.bytes = input.len() as u64;
    }

}
