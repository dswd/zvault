use serde::bytes::ByteBuf;

use blake2::blake2b::Blake2b;

#[derive(Clone, Debug, Copy)]
#[allow(non_camel_case_types)]
pub enum ChecksumType {
    Blake2_256
}
serde_impl!(ChecksumType(u64) {
    Blake2_256 => 1
});

impl ChecksumType {
    #[inline]
    pub fn from(name: &str) -> Result<Self, &'static str> {
        match name {
            "blake2_256" => Ok(ChecksumType::Blake2_256),
            _ => Err("Unsupported checksum type")
        }
    }

    #[inline]
    pub fn name(&self) -> &'static str {
        match *self {
            ChecksumType::Blake2_256 => "blake2_256",
        }
    }
}


pub type Checksum = (ChecksumType, ByteBuf);

#[allow(non_camel_case_types, unknown_lints, large_enum_variant)]
pub enum ChecksumCreator {
    Blake2_256(Blake2b)
}

impl ChecksumCreator {
    #[inline]
    pub fn new(type_: ChecksumType) -> Self {
        match type_ {
            ChecksumType::Blake2_256 => ChecksumCreator::Blake2_256(Blake2b::new(32))
        }
    }

    #[inline]
    pub fn update(&mut self, data: &[u8]) {
        match *self {
            ChecksumCreator::Blake2_256(ref mut state) => state.update(data)
        }
    }

    #[inline]
    pub fn finish(self) -> Checksum {
        match self {
            ChecksumCreator::Blake2_256(state) => {
                let mut buf = Vec::with_capacity(32);
                buf.extend_from_slice(state.finalize().as_bytes());
                (ChecksumType::Blake2_256, buf.into())
            }
        }
    }
}
