use std::ops::Deref;

pub struct Bitmap {
    bytes: Vec<u8>
}

impl Bitmap {
    pub fn new(len: usize) -> Self {
        let len = (len+7)/8;
        let mut bytes = Vec::with_capacity(len);
        bytes.resize(len, 0);
        Self { bytes: bytes }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len() * 8
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn convert_index(&self, index: usize) -> (usize, u8) {
        (index/8, 1u8<<(index%8))
    }

    #[inline]
    pub fn set(&mut self, index: usize) {
        let (byte, mask) = self.convert_index(index);
        self.bytes[byte] |= mask
    }

    #[inline]
    pub fn unset(&mut self, index: usize) {
        let (byte, mask) = self.convert_index(index);
        self.bytes[byte] &= !mask
    }

    #[inline]
    pub fn flip(&mut self, index: usize) {
        let (byte, mask) = self.convert_index(index);
        self.bytes[byte] ^= mask
    }

    #[inline]
    pub fn get(&self, index: usize) -> bool {
        let (byte, mask) = self.convert_index(index);
        self.bytes[byte] & mask != 0
    }

    #[inline]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[inline]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes: bytes }
    }
}

impl Deref for Bitmap {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}
