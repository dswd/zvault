use serde::bytes::ByteBuf;
use crypto::sha3;
use crypto::digest::Digest;


#[derive(Clone, Debug, Copy)]
#[allow(non_camel_case_types)]
pub enum ChecksumType {
    Sha3_256
}
serde_impl!(ChecksumType(u64) {
    Sha3_256 => 1
});

impl ChecksumType {
    #[inline]
    pub fn from(name: &str) -> Result<Self, &'static str> {
        match name {
            "sha3-256" => Ok(ChecksumType::Sha3_256),
            _ => Err("Unsupported checksum type")
        }
    }

    #[inline]
    pub fn name(&self) -> &'static str {
        match *self {
            ChecksumType::Sha3_256 => "sha3-256",
        }
    }
}


pub type Checksum = (ChecksumType, ByteBuf);

#[allow(non_camel_case_types, unknown_lints, large_enum_variant)]
pub enum ChecksumCreator {
    Sha3_256(sha3::Sha3)
}

impl ChecksumCreator {
    #[inline]
    pub fn new(type_: ChecksumType) -> Self {
        match type_ {
            ChecksumType::Sha3_256 => ChecksumCreator::Sha3_256(sha3::Sha3::sha3_256())
        }
    }

    #[inline]
    pub fn update(&mut self, data: &[u8]) {
        match *self {
            ChecksumCreator::Sha3_256(ref mut state) => state.input(data)
        }
    }

    #[inline]
    pub fn finish(self) -> Checksum {
        match self {
            ChecksumCreator::Sha3_256(mut state) => {
                let mut buf = Vec::with_capacity(state.output_bytes());
                buf.resize(state.output_bytes(), 0);
                state.result(&mut buf);
                (ChecksumType::Sha3_256, buf.into())
            }
        }
    }
}
