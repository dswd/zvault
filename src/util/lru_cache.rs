
use std::hash::Hash;
use std::collections::HashMap;

pub struct LruCache<K, V> {
    items: HashMap<K, (V, u64)>,
    min_size: usize,
    max_size: usize,
    next: u64
}


impl<K: Eq+Hash, V> LruCache<K, V> {
    #[inline]
    pub fn new(min_size: usize, max_size: usize) -> Self {
        LruCache {
            items: HashMap::default(),
            min_size: min_size,
            max_size: max_size,
            next: 0
        }
    }

    #[inline]
    pub fn put(&mut self, key: K, value: V) {
        self.items.insert(key, (value, self.next));
        self.next += 1;
        if self.items.len() > self.max_size {
            self.shrink()
        }
    }

    #[inline]
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(&mut (ref item, ref mut n)) = self.items.get_mut(key) {
            *n = self.next;
            self.next += 1;
            Some(item)
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if let Some(&mut (ref mut item, ref mut n)) = self.items.get_mut(key) {
            *n = self.next;
            self.next += 1;
            Some(item)
        } else {
            None
        }
    }

    fn shrink(&mut self) {
        let mut tags: Vec<u64> = self.items.values().map(|&(_, n)| n).collect();
        tags.sort();
        let min = tags[tags.len()-self.min_size];
        let mut new = HashMap::with_capacity(self.min_size);
        new.extend(self.items.drain().filter(|&(_,(_, n))| n>=min));
        self.items = new;
    }
}
