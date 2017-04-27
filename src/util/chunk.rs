use std::io::{self, Write, Read, Cursor};
use std::ops::{Deref, DerefMut};

use serde::{self, Serialize, Deserialize};
use serde_bytes::{Bytes, ByteBuf};
use serde::de::Error;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use super::Hash;

pub type Chunk = (Hash, u32);

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct ChunkList(Vec<Chunk>);

impl ChunkList {
    #[inline]
    pub fn new() -> Self {
        ChunkList(Vec::new())
    }

    #[inline]
    pub fn with_capacity(num: usize) -> Self {
        ChunkList(Vec::with_capacity(num))
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn push(&mut self, chunk: Chunk) {
        self.0.push(chunk)
    }

    pub fn write_to(&self, dst: &mut Write) -> Result<(), io::Error> {
        for chunk in &self.0 {
            try!(chunk.0.write_to(dst));
            try!(dst.write_u32::<LittleEndian>(chunk.1));
        }
        Ok(())
    }

    pub fn read_n_from(n: usize, src: &mut Read) -> Result<Self, io::Error> {
        let mut chunks = Vec::with_capacity(n);
        for _ in 0..n {
            let hash = try!(Hash::read_from(src));
            let len = try!(src.read_u32::<LittleEndian>());
            chunks.push((hash, len));
        }
        Ok(ChunkList(chunks))
    }

    #[inline]
    pub fn read_from(src: &[u8]) -> Self {
        if src.len() % 20 != 0 {
            warn!("Reading truncated chunk list");
        }
        ChunkList::read_n_from(src.len()/20, &mut Cursor::new(src)).unwrap()
    }

    #[inline]
    pub fn encoded_size(&self) -> usize {
        self.0.len() * 20
    }

    #[inline]
    pub fn into_inner(self) -> Vec<Chunk> {
        self.0
    }
}

impl Default for ChunkList {
    #[inline]
    fn default() -> Self {
        ChunkList(Vec::new())
    }
}

impl From<Vec<Chunk>> for ChunkList {
    fn from(val: Vec<Chunk>) -> Self {
        ChunkList(val)
    }
}

impl Into<Vec<Chunk>> for ChunkList {
    fn into(self) -> Vec<Chunk> {
        self.0
    }
}

impl Deref for ChunkList {
    type Target = [Chunk];
    fn deref(&self) -> &[Chunk] {
        &self.0
    }
}

impl DerefMut for ChunkList {
    fn deref_mut(&mut self) -> &mut [Chunk] {
        &mut self.0
    }
}

impl Serialize for ChunkList {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
        let mut buf = Vec::with_capacity(self.encoded_size());
        self.write_to(&mut buf).unwrap();
        Bytes::from(&buf as &[u8]).serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for ChunkList {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::Deserializer<'a> {
        let data: Vec<u8> = try!(ByteBuf::deserialize(deserializer)).into();
        if data.len() % 20 != 0 {
            return Err(D::Error::custom("Invalid chunk list length"));
        }
        Ok(ChunkList::read_n_from(data.len()/20, &mut Cursor::new(data)).unwrap())
    }
}
