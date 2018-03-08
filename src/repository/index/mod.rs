use std::path::Path;
use std::fs::{File, OpenOptions};
use std::mem;
use std::ptr;
use std::io;
use std::slice;
use std::os::unix::io::AsRawFd;

use mmap::{MemoryMap, MapOption, MapError};

use ::prelude::*;

pub const MAX_USAGE: f64 = 0.9;
pub const MIN_USAGE: f64 = 0.35;
pub const INITIAL_SIZE: usize = 1024;


quick_error!{
    #[derive(Debug)]
    pub enum IndexError {
        Io(err: io::Error) {
            from()
            cause(err)
            description(tr!("Failed to open index file"))
            display("{}", tr_format!("Index error: failed to open the index file\n\tcaused by: {}", err))
        }
        Mmap(err: MapError) {
            from()
            cause(err)
            description(tr!("Failed to memory-map the index file"))
            display("{}", tr_format!("Index error: failed to memory-map the index file\n\tcaused by: {}", err))
        }
        WrongMagic {
            description(tr!("Wrong header"))
            display("{}", tr!("Index error: file has the wrong magic header"))
        }
        UnsupportedVersion(version: u8) {
            description(tr!("Unsupported version"))
            display("{}", tr_format!("Index error: index file has unsupported version: {}", version))
        }
        WrongPosition(should: usize, is: LocateResult) {
            description(tr!("Key at wrong position"))
            display("{}", tr_format!("Index error: key has wrong position, expected at: {}, but is at: {:?}", should, is))
        }
        WrongEntryCount(header: usize, actual: usize) {
            description(tr!("Wrong entry count"))
            display("{}", tr_format!("Index error: index has wrong entry count, expected {}, but is {}", header, actual))
        }
    }
}


#[repr(packed)]
pub struct Header {
    magic: [u8; 7],
    version: u8,
    entries: u64,
    capacity: u64,
}


pub trait Key: Eq + Copy + Default {
    fn hash(&self) -> u64;
    fn is_used(&self) -> bool;
    fn clear(&mut self);
}


pub trait Value: Copy + Default {}


#[repr(packed)]
#[derive(Default)]
pub struct Entry<K, V> {
    key: K,
    data: V
}

impl<K: Key, V> Entry<K, V> {
    #[inline]
    fn is_used(&self) -> bool {
        unsafe { self.key.is_used() }
    }

    #[inline]
    fn clear(&mut self) {
        unsafe { self.key.clear() }
    }

    #[inline]
    fn get(&self) -> (&K, &V) {
        unsafe { (&self.key, &self.data) }
    }

    #[inline]
    fn get_mut(&mut self) -> (&K, &mut V) {
        unsafe { (&self.key, &mut self.data) }
    }

    #[inline]
    fn get_key(&self) -> &K {
        unsafe { &self.key }
    }

    #[inline]
    fn get_mut_key(&mut self) -> &mut K {
        unsafe { &mut self.key }
    }

    #[inline]
    fn get_data(&self) -> &V {
        unsafe { &self.data }
    }

    #[inline]
    fn get_mut_data(&mut self) -> &mut V {
        unsafe { &mut self.data }
    }
}


#[derive(Debug)]
pub enum LocateResult {
    Found(usize), // Found the key at this position
    Hole(usize), // Found a hole at this position while searching for a key
    Steal(usize) // Found a spot to steal at this position while searching for a key
}


pub struct Iter<'a, K: 'static, V: 'static> (&'a [Entry<K, V>]);

impl<'a, K: Key, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((first, rest)) = self.0.split_first() {
            self.0 = rest;
            if first.is_used() {
                return Some(first.get())
            }
        }
        None
    }
}

#[allow(dead_code)]
pub struct IterMut<'a, K: 'static, V: 'static> (&'a mut [Entry<K, V>]);

impl<'a, K: Key, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let slice = mem::replace(&mut self.0, &mut []);
            match slice.split_first_mut() {
                None => return None,
                Some((first, rest)) => {
                    self.0 = rest;
                    if first.is_used() {
                        return Some(first.get_mut())
                    }
                }
            }
        }
    }
}


/// This method is unsafe as it potentially creates references to uninitialized memory
unsafe fn mmap_as_ref<K, V>(mmap: &MemoryMap, len: usize) -> (&'static mut Header, &'static mut [Entry<K, V>]) {
    if mmap.len() < mem::size_of::<Header>() + len * mem::size_of::<Entry<K, V>>() {
        tr_panic!("Memory map too small");
    }
    let header = &mut *(mmap.data() as *mut Header);
    let ptr = mmap.data().offset(mem::size_of::<Header>() as isize) as *mut Entry<K, V>;
    let data = slice::from_raw_parts_mut(ptr, len);
    (header, data)
}

pub struct Index<K: 'static, V: 'static> {
    capacity: usize,
    mask: usize,
    entries: usize,
    max_entries: usize,
    min_entries: usize,
    fd: File,
    mmap: MemoryMap,
    header: &'static mut Header,
    data: &'static mut [Entry<K, V>]
}

impl<K: Key, V: Value> Index<K, V> {
    pub fn new(path: &Path, create: bool, magic: &[u8; 7], version: u8) -> Result<Self, IndexError> {
        let fd = try!(OpenOptions::new().read(true).write(true).create(create).open(path));
        if create {
            try!(Self::resize_fd(&fd, INITIAL_SIZE));
        }
        let mmap = try!(Self::map_fd(&fd));
        if mmap.len() < mem::size_of::<Header>() {
            return Err(IndexError::WrongMagic);
        }
        let (header, data) = unsafe { mmap_as_ref::<K, V>(&mmap, INITIAL_SIZE as usize) };
        if create {
            // This is safe, nothing in header is Drop
            header.magic = magic.to_owned();
            header.version = version;
            header.entries = 0;
            header.capacity = INITIAL_SIZE as u64;
            // Initialize data without dropping the uninitialized data in it
            for d in data {
                unsafe { ptr::write(d, Entry::default()) }
            }
        }
        if header.magic != *magic {
            return Err(IndexError::WrongMagic);
        }
        if header.version != version {
            return Err(IndexError::UnsupportedVersion(header.version));
        }
        let (header, data) = unsafe { mmap_as_ref(&mmap, header.capacity as usize) };
        let index = Index{
            capacity: header.capacity as usize,
            mask: header.capacity as usize -1,
            max_entries: (header.capacity as f64 * MAX_USAGE) as usize,
            min_entries: (header.capacity as f64 * MIN_USAGE) as usize,
            entries: header.entries as usize,
            fd,
            mmap,
            data,
            header
        };
        debug_assert!(index.check().is_ok(), tr!("Inconsistent after creation"));
        Ok(index)
    }

    /// This method is unsafe as there is no way to guarantee that the contents of the file are
    /// valid objects.
    #[inline]
    pub unsafe fn open<P: AsRef<Path>>(path: P, magic: &[u8; 7], version: u8) -> Result<Self, IndexError> {
        Index::new(path.as_ref(), false, magic, version)
    }

    #[inline]
    pub fn create<P: AsRef<Path>>(path: P, magic: &[u8; 7], version: u8) -> Result<Self, IndexError> {
        Index::new(path.as_ref(), true, magic, version)
    }

    #[inline]
    fn map_fd(fd: &File) -> Result<MemoryMap, IndexError> {
        MemoryMap::new(
            try!(fd.metadata().map_err(IndexError::Io)).len() as usize,
            &[MapOption::MapReadable,
            MapOption::MapWritable,
            MapOption::MapFd(fd.as_raw_fd()),
            MapOption::MapNonStandardFlags(0x0001) //libc::consts::os::posix88::MAP_SHARED
        ]).map_err(IndexError::Mmap)
    }

    #[inline]
    fn resize_fd(fd: &File, capacity: usize) -> Result<(), IndexError> {
        fd.set_len((mem::size_of::<Header>() + capacity * mem::size_of::<Entry<K, V>>()) as u64).map_err(IndexError::Io)
    }

    #[inline]
    fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        debug_assert_eq!(capacity.count_ones(), 1);
        self.mask = capacity -1;
        self.min_entries = (capacity as f64 * MIN_USAGE) as usize;
        self.max_entries = (capacity as f64 * MAX_USAGE) as usize;
    }

    #[allow(redundant_field_names)]
    fn reinsert(&mut self, start: usize, end: usize) -> Result<(), IndexError> {
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

    fn shrink(&mut self) -> Result<bool, IndexError> {
        if self.entries >= self.min_entries || self.capacity <= INITIAL_SIZE {
            return Ok(false)
        }
        let old_capacity = self.capacity;
        let new_capacity = self.capacity / 2;
        self.set_capacity(new_capacity);
        try!(self.reinsert(new_capacity, old_capacity));
        try!(Self::resize_fd(&self.fd, new_capacity));
        self.mmap = try!(Self::map_fd(&self.fd));
        let (header, data) = unsafe { mmap_as_ref(&self.mmap, new_capacity) };
        self.header = header;
        self.data = data;
        assert_eq!(self.data.len(), self.capacity);
        Ok(true)
    }

    fn extend(&mut self) -> Result<bool, IndexError> {
        if self.entries <= self.max_entries {
            return Ok(false)
        }
        let new_capacity = 2 * self.capacity;
        try!(Self::resize_fd(&self.fd, new_capacity));
        self.mmap = try!(Self::map_fd(&self.fd));
        let (header, data) = unsafe { mmap_as_ref(&self.mmap, new_capacity) };
        // Initialize upper half of data without dropping the uninitialized data in it
        for d in &mut data[self.capacity..] {
            unsafe { ptr::write(d, Entry::default()) }
        }
        self.header = header;
        self.data = data;
        self.set_capacity(new_capacity);
        assert_eq!(self.data.len(), self.capacity);
        try!(self.reinsert(0, new_capacity));
        Ok(true)
    }

    pub fn check(&self) -> Result<(), IndexError> {
        let mut entries = 0;
        for pos in 0..self.capacity {
            let entry = &self.data[pos];
            if !entry.is_used() {
                continue;
            }
            entries += 1;
            match self.locate(entry.get_key()) {
                LocateResult::Found(p) if p == pos => true,
                found => return Err(IndexError::WrongPosition(pos, found))
            };
        }
        if entries != self.entries {
            return Err(IndexError::WrongEntryCount(self.entries, entries));
        }
        Ok(())
    }

    #[inline]
    fn increase_count(&mut self) -> Result<(), IndexError> {
        self.entries += 1;
        try!(self.extend());
        self.write_header();
        Ok(())
    }

    #[inline]
    fn decrease_count(&mut self) -> Result<(), IndexError> {
        self.entries -= 1;
        try!(self.shrink());
        self.write_header();
        Ok(())
    }

    #[inline]
    fn write_header(&mut self) {
        self.header.entries = self.entries as u64;
        self.header.capacity = self.capacity as u64;
    }

    #[inline]
    fn get_displacement(&self, entry: &Entry<K, V>, pos: usize) -> usize {
        (pos + self.capacity - (entry.get_key().hash() as usize & self.mask)) & self.mask
    }

    /// Finds the position for this key
    /// If the key is in the table, it will be the position of the key,
    /// otherwise it will be the position where this key should be inserted
    fn locate(&self, key: &K) -> LocateResult {
        let mut pos = key.hash() as usize & self.mask;
        let mut dist = 0;
        loop {
            let entry = &self.data[pos];
            if !entry.is_used() {
                return LocateResult::Hole(pos);
            }
            if entry.get_key() == key {
                return LocateResult::Found(pos);
            }
            let odist = self.get_displacement(entry, pos);
            if dist > odist {
                return LocateResult::Steal(pos);
            }
            pos = (pos + 1) & self.mask;
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
            pos = (pos + 1) & self.mask;
            {
                let entry = &self.data[pos];
                if !entry.is_used() {
                    // we found a hole, stop shifting here
                    break;
                }
                if (entry.get_key().hash() as usize & self.mask) == pos {
                    // we found an entry at the right position, stop shifting here
                    break;
                }
            }
            self.data.swap(last_pos, pos);
        }
        self.data[last_pos].clear();
    }

    /// Adds the key, data pair into the table.
    /// If the key existed the old data is returned.
    pub fn set(&mut self, key: &K, data: &V) -> Result<Option<V>, IndexError> {
        match self.locate(key) {
            LocateResult::Found(pos) => {
                let mut old = *data;
                mem::swap(&mut old, self.data[pos].get_mut_data());
                Ok(Some(old))
            },
            LocateResult::Hole(pos) => {
                {
                    let entry = &mut self.data[pos];
                    entry.key = *key;
                    entry.data = *data;
                }
                try!(self.increase_count());
                Ok(None)
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
                    cur_pos = (cur_pos + 1) & self.mask;
                    let entry = &mut self.data[cur_pos];
                    if entry.is_used() {
                        mem::swap(&mut stolen_key, entry.get_mut_key());
                        mem::swap(&mut stolen_data, entry.get_mut_data());
                    } else {
                        entry.key = stolen_key;
                        entry.data = stolen_data;
                        break;
                    }
                }
                try!(self.increase_count());
                Ok(None)
            }
        }
    }

    #[inline]
    pub fn contains(&self, key: &K) -> bool {
        debug_assert!(self.check().is_ok(), tr!("Inconsistent before get"));
        match self.locate(key) {
            LocateResult::Found(_) => true,
            _ => false
        }
    }

    #[inline]
    pub fn pos(&self, key: &K) -> Option<usize> {
        debug_assert!(self.check().is_ok(), tr!("Inconsistent before get"));
        match self.locate(key) {
            LocateResult::Found(pos) => Some(pos),
            _ => None
        }
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<V> {
        debug_assert!(self.check().is_ok(), tr!("Inconsistent before get"));
        match self.locate(key) {
            LocateResult::Found(pos) => Some(self.data[pos].data),
            _ => None
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn modify<F>(&mut self, key: &K, mut f: F) -> bool where F: FnMut(&mut V) {
        debug_assert!(self.check().is_ok(), tr!("Inconsistent before get"));
        match self.locate(key) {
            LocateResult::Found(pos) => {
                f(self.data[pos].get_mut_data());
                true
            },
            _ => false
        }
    }

    #[inline]
    pub fn delete(&mut self, key: &K) -> Result<bool, IndexError> {
        match self.locate(key) {
            LocateResult::Found(pos) => {
                self.backshift(pos);
                try!(self.decrease_count());
                Ok(true)
            },
            _ => Ok(false)
        }
    }

    pub fn filter<F>(&mut self, mut f: F) -> Result<usize, IndexError> where F: FnMut(&K, &V) -> bool {
        //TODO: is it faster to walk in reverse direction?
        let mut deleted = 0;
        let mut pos = 0;
        while pos < self.capacity {
            {
                let entry = &mut self.data[pos];
                if !entry.is_used() || f(entry.get_key(), entry.get_data()) {
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
    pub fn iter(&self) -> Iter<K, V> {
        Iter(self.data)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut(self.data)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.mmap.len()
    }

    #[inline]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn clear(&mut self) {
        for entry in &mut self.data[..] {
            entry.clear();
        }
        self.entries = 0;
    }

    #[allow(dead_code)]
    pub fn statistics(&self) -> IndexStatistics {
        IndexStatistics {
            count: self.entries,
            capacity: self.capacity,
            size: self.size(),
            displacement: ValueStats::from_iter(|| self.data.iter().enumerate().filter(
                |&(_, entry)| entry.is_used()).map(
                |(index, entry)| self.get_displacement(entry, index) as f32))
        }
    }

}


#[derive(Debug)]
pub struct IndexStatistics {
    pub count: usize,
    pub capacity: usize,
    pub size: usize,
    pub displacement: ValueStats
}