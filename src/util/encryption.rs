use std::collections::HashMap;

quick_error!{
    #[derive(Debug)]
    pub enum EncryptionError {
        Operation(reason: &'static str) {
            description("Operation failed")
            display("Operation failed: {}", reason)
        }
    }
}


#[derive(Clone)]
pub enum EncryptionMethod {
    Dummy
}
serde_impl!(EncryptionMethod(u64) {
    Dummy => 0
});

pub type EncryptionKey = Vec<u8>;

pub type EncryptionKeyId = u64;

pub type Encryption = (EncryptionMethod, EncryptionKeyId);

#[derive(Clone)]
pub struct Crypto {
    keys: HashMap<EncryptionKeyId, EncryptionKey>
}

impl Crypto {
    #[inline]
    pub fn new() -> Self {
        Crypto { keys: Default::default() }
    }

    #[inline]
    pub fn register_key(&mut self, key: EncryptionKey, id: EncryptionKeyId) {
        self.keys.insert(id, key);
    }

    #[inline]
    pub fn contains_key(&mut self, id: EncryptionKeyId) -> bool {
        self.keys.contains_key(&id)
    }

    #[inline]
    pub fn encrypt(&self, _enc: Encryption, _data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        unimplemented!()
    }

    #[inline]
    pub fn decrypt(&self, _enc: Encryption, _data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        unimplemented!()
    }
}

impl Default for Crypto {
    #[inline]
    fn default() -> Self {
        Crypto::new()
    }
}
