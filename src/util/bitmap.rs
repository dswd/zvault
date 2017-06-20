use std::ops::Deref;

#[derive(Clone)]
pub struct Bitmap {
    bytes: Vec<u8>
}

impl Bitmap {
    /// Creates a new bitmap
    pub fn new(len: usize) -> Self {
        let len = (len+7)/8;
        let mut bytes = Vec::with_capacity(len);
        bytes.resize(len, 0);
        Self { bytes: bytes }
    }

    /// Returns the number of bits in the bitmap
    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len() * 8
    }

    /// Returns whether the bitmap is empty, i.e. contains no bits
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


mod tests {
    #[allow(unused_imports)]
    use super::Bitmap;

    #[test]
    fn test_new() {
        Bitmap::new(1024);
    }

    #[test]
    fn test_len() {
        assert_eq!(Bitmap::new(1024).len(), 1024);
    }

    #[test]
    fn test_is_empty() {
        assert!(!Bitmap::new(1024).is_empty());
        assert!(Bitmap::new(0).is_empty());
    }

    #[test]
    fn test_set() {
        let mut bitmap = Bitmap::new(1024);
        assert!(!bitmap.get(5));
        assert!(!bitmap.get(154));
        bitmap.set(5);
        assert!(bitmap.get(5));
        assert!(!bitmap.get(154));
        bitmap.set(154);
        assert!(bitmap.get(5));
        assert!(bitmap.get(154));
    }

    #[test]
    fn test_unset() {
        let mut bitmap = Bitmap::new(1024);
        assert!(!bitmap.get(5));
        bitmap.set(5);
        assert!(bitmap.get(5));
        bitmap.unset(5);
        assert!(!bitmap.get(5));
        assert!(!bitmap.get(154));
        bitmap.unset(154);
        assert!(!bitmap.get(154));
    }

    #[test]
    fn test_flip() {
        let mut bitmap = Bitmap::new(1024);
        assert!(!bitmap.get(5));
        bitmap.flip(5);
        assert!(bitmap.get(5));
        bitmap.set(154);
        assert!(bitmap.get(154));
        bitmap.flip(154);
        assert!(!bitmap.get(154));
    }

    #[test]
    fn test_as_bytes() {
        let mut bitmap = Bitmap::new(16);
        assert_eq!(bitmap.as_bytes(), &[0, 0]);
        bitmap.set(0);
        assert_eq!(bitmap.as_bytes(), &[1, 0]);
        bitmap.set(8);
        bitmap.set(9);
        assert_eq!(bitmap.as_bytes(), &[1, 3]);
    }

    #[test]
    fn test_into_bytes() {
        let mut bitmap = Bitmap::new(16);
        bitmap.set(0);
        bitmap.set(8);
        bitmap.set(9);
        assert_eq!(bitmap.as_bytes(), &bitmap.clone().into_bytes() as &[u8]);
    }

    #[test]
    fn test_from_bytes() {
        assert_eq!(&[1, 3], Bitmap::from_bytes(vec![1, 3]).as_bytes());
    }

}
