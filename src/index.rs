use super::util::Hash;

use std::path::Path;
use std::fs::{File, OpenOptions};
use std::mem;
use std::io;
use std::slice;
use std::os::unix::io::AsRawFd;

use mmap::{MemoryMap, MapOption, MapError};

const MAGIC: [u8; 7] = *b"zcindex";
const VERSION: u8 = 1;
pub const MAX_USAGE: f64 = 0.9;
pub const MIN_USAGE: f64 = 0.25;
pub const INITIAL_SIZE: usize = 1024;

#[repr(packed)]
pub struct Header {
    magic: [u8; 7],
    version: u8,
    entries: u64,
    capacity: u64,
}

#[repr(packed)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Location {
    pub bundle: u32,
    pub chunk: u32
}
impl Location {
    pub fn new(bundle: u32, chunk: u32) -> Self {
        Location{ bundle: bundle, chunk: chunk }
    }
}


#[repr(packed)]
#[derive(Clone)]
pub struct Entry {
    pub key: Hash,
    pub data: Location
}

impl Entry {
    #[inline]
    fn is_used(&self) -> bool {
        self.key.low != 0 || self.key.high != 0
    }

    fn clear(&mut self) {
        self.key.low = 0;
        self.key.high = 0;
    }
}

pub struct Index {
    capacity: usize,
    entries: usize,
    max_entries: usize,
    min_entries: usize,
    fd: File,
    mmap: MemoryMap,
    data: &'static mut [Entry]
}

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    MapError(MapError),
    NoHeader,
    MagicError,
    VersionError,
}

#[derive(Debug)]
enum LocateResult {
    Found(usize), // Found the key at this position
    Hole(usize), // Found a hole at this position while searching for a key
    Steal(usize) // Found a spot to steal at this position while searching for a key
}

impl Index {
    pub fn new(path: &Path, create: bool) -> Result<Index, Error> {
        let fd = try!(OpenOptions::new().read(true).write(true).create(create).open(path).map_err(|e| { Error::IOError(e) }));
        if create {
            try!(Index::resize_fd(&fd, INITIAL_SIZE));
        }
        let mmap = try!(Index::map_fd(&fd));
        if mmap.len() < mem::size_of::<Header>() {
            return Err(Error::NoHeader);
        }
        let data = Index::mmap_as_slice(&mmap, INITIAL_SIZE as usize);
        let mut index = Index{capacity: 0, max_entries: 0, min_entries: 0, entries: 0, fd: fd, mmap: mmap, data: data};
        {
            let capacity;
            let entries;
            {
                let header = index.header();
                if create {
                    header.magic = MAGIC;
                    header.version = VERSION;
                    header.entries = 0;
                    header.capacity = INITIAL_SIZE as u64;
                } else {
                    if header.magic != MAGIC {
                        return Err(Error::MagicError);
                    }
                    if header.version != VERSION {
                        return Err(Error::VersionError);
                    }
                }
                capacity = header.capacity;
                entries = header.entries;
            }
            index.data = Index::mmap_as_slice(&index.mmap, capacity as usize);
            index.set_capacity(capacity as usize);
            index.entries = entries as usize;
        }
        debug_assert!(index.is_consistent(), "Inconsistent after creation");
        Ok(index)
    }

    #[inline]
    pub fn open(path: &Path) -> Result<Index, Error> {
        Index::new(path, false)
    }

    #[inline]
    pub fn create(path: &Path) -> Result<Index, Error> {
        Index::new(path, true)
    }

    #[inline]
    fn map_fd(fd: &File) -> Result<MemoryMap, Error> {
        MemoryMap::new(
            try!(fd.metadata().map_err(Error::IOError)).len() as usize,
            &[MapOption::MapReadable,
            MapOption::MapWritable,
            MapOption::MapFd(fd.as_raw_fd()),
            MapOption::MapNonStandardFlags(0x0001) //libc::consts::os::posix88::MAP_SHARED
        ]).map_err(|e| { Error::MapError(e) })
    }

    #[inline]
    fn resize_fd(fd: &File, capacity: usize) -> Result<(), Error> {
        fd.set_len((mem::size_of::<Header>() + capacity * mem::size_of::<Entry>()) as u64).map_err( Error::IOError)
    }

    #[inline]
    fn mmap_as_slice(mmap: &MemoryMap, len: usize) -> &'static mut [Entry] {
        if mmap.len() < mem::size_of::<Header>() + len * mem::size_of::<Entry>() {
            panic!("Memory map too small");
        }
        let ptr = unsafe { mmap.data().offset(mem::size_of::<Header>() as isize) as *mut Entry };
        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }

    #[inline]
    fn header(&mut self) -> &mut Header {
        if self.mmap.len() < mem::size_of::<Header>() {
            panic!("Failed to read beyond end");
        }
        unsafe { &mut *(self.mmap.data() as *mut Header) }
    }

    #[inline]
    fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        self.min_entries = (capacity as f64 * MIN_USAGE) as usize;
        self.max_entries = (capacity as f64 * MAX_USAGE) as usize;
    }

    fn reinsert(&mut self, start: usize, end: usize) -> Result<(), Error> {
        for pos in start..end {
            let key;
            let data;
            {
                let entry = &mut self.data[pos];
                if !entry.is_used() {
                    continue;
                }
                key = entry.key;
                data = entry.data;
                entry.clear();
            }
            self.entries -= 1;
            try!(self.set(&key, &data));
        }
        Ok(())
    }

    fn shrink(&mut self) -> Result<bool, Error> {
        if self.entries >= self.min_entries || self.capacity <= INITIAL_SIZE {
            return Ok(false)
        }
        let old_capacity = self.capacity;
        let new_capacity = self.capacity / 2;
        self.set_capacity(new_capacity);
        try!(self.reinsert(new_capacity, old_capacity));
        try!(Index::resize_fd(&self.fd, new_capacity));
        self.mmap = try!(Index::map_fd(&self.fd));
        self.data = Index::mmap_as_slice(&self.mmap, new_capacity);
        assert_eq!(self.data.len(), self.capacity);
        Ok(true)
    }

    fn extend(&mut self) -> Result<bool, Error> {
        if self.entries <= self.max_entries {
            return Ok(false)
        }
        let new_capacity = 2 * self.capacity;
        try!(Index::resize_fd(&self.fd, new_capacity));
        self.mmap = try!(Index::map_fd(&self.fd));
        self.data = Index::mmap_as_slice(&self.mmap, new_capacity);
        self.set_capacity(new_capacity);
        assert_eq!(self.data.len(), self.capacity);
        try!(self.reinsert(0, new_capacity));
        Ok(true)
    }

    #[allow(dead_code)]
    pub fn is_consistent(&self) -> bool {
        let mut entries = 0;
        for pos in 0..self.capacity {
            let entry = &self.data[pos];
            if !entry.is_used() {
                continue;
            }
            entries += 1;
            match self.locate(&entry.key) {
                LocateResult::Found(p) if p == pos => true,
                found => {
                    println!("Inconsistency found: Key {:?} should be at {} but is at {:?}", entry.key, pos, found);
                    return false
                }
            };
        }
        if entries != self.entries {
            println!("Inconsistency found: Index contains {} entries, should contain {}", entries, self.entries);
            return false
        }
        true
    }

    pub fn check(&self) -> Result<(), &'static str> {
        //TODO: proper errors instead of string
        if self.is_consistent() {
            Ok(())
        } else {
            Err("Inconsistent")
        }
    }

    #[inline]
    fn increase_count(&mut self) -> Result<(), Error> {
        self.entries += 1;
        try!(self.extend());
        self.write_header();
        Ok(())
    }

    #[inline]
    fn decrease_count(&mut self) -> Result<(), Error> {
        self.entries -= 1;
        try!(self.shrink());
        self.write_header();
        Ok(())
    }

    #[inline]
    fn write_header(&mut self) {
        let entries = self.entries;
        let capacity = self.capacity;
        let header = self.header();
        header.entries = entries as u64;
        header.capacity = capacity as u64;
    }

    /// Finds the position for this key
    /// If the key is in the table, it will be the position of the key,
    /// otherwise it will be the position where this key should be inserted
    fn locate(&self, key: &Hash) -> LocateResult {
        let mut pos = key.hash() as usize % self.capacity;
        let mut dist = 0;
        loop {
            let entry = &self.data[pos];
            if !entry.is_used() {
                return LocateResult::Hole(pos);
            }
            if entry.key == *key {
                return LocateResult::Found(pos);
            }
            let odist = (pos + self.capacity - entry.key.hash() as usize % self.capacity) % self.capacity;
            if dist > odist {
                return LocateResult::Steal(pos);
            }
            pos = (pos + 1) % self.capacity ;
            dist += 1;
        }
    }

    /// Shifts all following entries towards the left if they can get closer to their ideal position.
    /// The entry at the given position will be lost.
    fn backshift(&mut self, start: usize) {
        let mut pos = start;
        let mut last_pos;
        loop {
            last_pos = pos;
            pos = (pos + 1) % self.capacity;
            {
                let entry = &self.data[pos];
                if !entry.is_used() {
                    // we found a hole, stop shifting here
                    break;
                }
                if entry.key.hash() as usize % self.capacity == pos {
                    // we found an entry at the right position, stop shifting here
                    break;
                }
            }
            self.data[last_pos] = self.data[pos].clone();
        }
        self.data[last_pos].clear();
    }

    /// Adds the key, data pair into the table.
    /// If the key existed in the table before, it is overwritten and false is returned.
    /// Otherwise it will be added to the table and true is returned.
    pub fn set(&mut self, key: &Hash, data: &Location) -> Result<bool, Error> {
        match self.locate(key) {
            LocateResult::Found(pos) => {
                self.data[pos].data = *data;
                Ok(false)
            },
            LocateResult::Hole(pos) => {
                {
                    let entry = &mut self.data[pos];
                    entry.key = *key;
                    entry.data = *data;
                }
                try!(self.increase_count());
                Ok(true)
            },
            LocateResult::Steal(pos) => {
                let mut stolen_key;
                let mut stolen_data;
                let mut cur_pos = pos;
                {
                    let entry = &mut self.data[pos];
                    stolen_key = entry.key;
                    stolen_data = entry.data;
                    entry.key = *key;
                    entry.data = *data;
                }
                loop {
                    cur_pos = (cur_pos + 1) % self.capacity;
                    let entry = &mut self.data[cur_pos];
                    if entry.is_used() {
                        mem::swap(&mut stolen_key, &mut entry.key);
                        mem::swap(&mut stolen_data, &mut entry.data);
                    } else {
                        entry.key = stolen_key;
                        entry.data = stolen_data;
                        break;
                    }
                }
                try!(self.increase_count());
                Ok(true)
            }
        }
    }

    #[inline]
    pub fn contains(&self, key: &Hash) -> bool {
        debug_assert!(self.is_consistent(), "Inconsistent before get");
        match self.locate(key) {
            LocateResult::Found(_) => true,
            _ => false
        }
    }

    #[inline]
    pub fn get(&self, key: &Hash) -> Option<Location> {
        debug_assert!(self.is_consistent(), "Inconsistent before get");
        match self.locate(key) {
            LocateResult::Found(pos) => Some(self.data[pos].data),
            _ => None
        }
    }

    #[inline]
    pub fn modify<F>(&mut self, key: &Hash, mut f: F) -> bool where F: FnMut(&mut Location) {
        debug_assert!(self.is_consistent(), "Inconsistent before get");
        match self.locate(key) {
            LocateResult::Found(pos) => {
                f(&mut self.data[pos].data);
                true
            },
            _ => false
        }
    }

    #[inline]
    pub fn delete(&mut self, key: &Hash) -> Result<bool, Error> {
        match self.locate(key) {
            LocateResult::Found(pos) => {
                self.backshift(pos);
                try!(self.decrease_count());
                Ok(true)
            },
            _ => Ok(false)
        }
    }

    pub fn filter<F>(&mut self, mut f: F) -> Result<usize, Error> where F: FnMut(&Hash, &Location) -> bool {
        //TODO: is it faster to walk in reverse direction?
        let mut deleted = 0;
        let mut pos = 0;
        while pos < self.capacity {
            {
                let entry = &mut self.data[pos];
                if !entry.is_used() || f(&entry.key, &entry.data) {
                    pos += 1;
                    continue;
                }
            }
            self.backshift(pos);
            deleted += 1;
        }
        self.entries -= deleted;
        while try!(self.shrink()) {}
        self.write_header();
        Ok(deleted)
    }

    #[inline]
    pub fn walk<F>(&self, mut f: F) where F: FnMut(&Hash, &Location) {
        for pos in 0..self.capacity {
            let entry = &self.data[pos];
            if entry.is_used() {
                f(&entry.key, &entry.data);
            }
        }
    }

    #[inline]
    pub fn walk_mut<F>(&mut self, mut f: F) where F: FnMut(&Hash, &mut Location) {
        for pos in 0..self.capacity {
            let entry = &mut self.data[pos];
            if entry.is_used() {
                f(&entry.key, &mut entry.data);
            }
        }
    }

    #[inline]
    pub fn next_entry(&self, index: usize) -> Option<usize> {
        let mut i = index;
        while i < self.capacity && !self.data[i].is_used() {
            i += 1;
        }
        if i == self.capacity {
            None
        } else {
            Some(i)
        }
    }

    #[inline]
    pub fn get_entry(&self, index: usize) -> Option<&Entry> {
        let entry = &self.data[index];
        if entry.is_used() {
            Some(entry)
        } else {
            None
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}
