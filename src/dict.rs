// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! An associative array mapping keys to values.

use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash, RandomState};

use crate::prng::Prng;

/// A key-value pair held in a bucket.
struct Entry<K, V> {
    /// The key.
    key: K,

    /// The value associated with the key.
    value: V,
}

/// An in-progress incremental rehash.
struct Rehash<K, V> {
    /// The target buckets being filled, twice the size of the main buckets.
    buckets: Vec<Vec<Entry<K, V>>>,

    /// Index of the next main bucket to migrate.
    index: usize,
}

/// A mapping from keys to values.
pub struct Dict<K, V> {
    /// The hash buckets, each is a chain of entries whose keys collide.
    buckets: Vec<Vec<Entry<K, V>>>,

    /// Present only while incrementally resizing.
    rehash: Option<Rehash<K, V>>,

    /// The number of entries stored across all buckets.
    length: usize,

    /// The fixed hasher seed, so a key hashes the same way for the dictionary's life.
    random_state: RandomState,
}

impl<K: Hash + Eq, V> Dict<K, V> {
    /// Creates an empty dictionary with at least `capacity` buckets, rounded up
    /// to a power of two.
    pub fn new(capacity: usize) -> Self {
        let size = capacity.max(4).next_power_of_two();
        Self {
            buckets: Self::empty_buckets(size),
            rehash: None,
            length: 0,
            random_state: RandomState::new(),
        }
    }

    /// Inserts `value` at `key`, returning the previous value if the key was
    /// already present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.rehash.is_some() {
            self.rehash_step();
        }
        // Only start rehash if the load factor has crossed 1.
        else if self.length > self.buckets.len() {
            self.start_rehash();
        }

        // Overwrite in the main buckets if the key is there.
        let main_index = Self::bucket_index(&self.random_state, &key, self.buckets.len());
        if let Some(existing) = self.buckets[main_index].iter_mut().find(|e| e.key == key) {
            return Some(std::mem::replace(&mut existing.value, value));
        }

        // If rehashing, the key belongs to the target buckets.
        if let Some(rehash) = self.rehash.as_mut() {
            let index = Self::bucket_index(&self.random_state, &key, rehash.buckets.len());
            if let Some(existing) = rehash.buckets[index].iter_mut().find(|e| e.key == key) {
                return Some(std::mem::replace(&mut existing.value, value));
            }
            rehash.buckets[index].push(Entry { key, value });
        }
        // Not rehashing, put the new entry in the main buckets.
        else {
            self.buckets[main_index].push(Entry { key, value });
        }

        self.length += 1;
        None
    }

    /// Returns a reference to the value at `key`, or `None` if it is absent.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        // Try finding in the main buckets.
        let index = Self::bucket_index(&self.random_state, key, self.buckets.len());
        if let Some(value) = self.buckets[index]
            .iter()
            .find(|e| e.key.borrow() == key)
            .map(|e| &e.value)
        {
            return Some(value);
        }

        // Try finding in the target buckets.
        if let Some(rehash) = &self.rehash {
            let index = Self::bucket_index(&self.random_state, key, rehash.buckets.len());
            return rehash.buckets[index]
                .iter()
                .find(|e| e.key.borrow() == key)
                .map(|e| &e.value);
        }

        None
    }

    /// Removes `key` and returns its value, or `None` if it is absent.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.rehash.is_some() {
            self.rehash_step();
        }

        // Try removing from the main buckets.
        let index = Self::bucket_index(&self.random_state, key, self.buckets.len());
        if let Some(pos) = self.buckets[index]
            .iter()
            .position(|e| e.key.borrow() == key)
        {
            self.length -= 1;
            // We don't use bucket.remove() since we don't care about the order
            // of elements in the bucket and hence prefer a O(1) operation over
            // O(N).
            return Some(self.buckets[index].swap_remove(pos).value);
        }

        // Try removing from the target buckets.
        if let Some(rehash) = self.rehash.as_mut() {
            let index = Self::bucket_index(&self.random_state, key, rehash.buckets.len());
            if let Some(pos) = rehash.buckets[index]
                .iter()
                .position(|e| e.key.borrow() == key)
            {
                self.length -= 1;
                // Same reason for swap_remove as above.
                return Some(rehash.buckets[index].swap_remove(pos).value);
            }
        }

        None
    }

    /// Returns up to `count` keys chosen at random.
    pub fn random_keys(&self, prng: &mut Prng, count: usize) -> Vec<&K> {
        if self.length == 0 {
            return Vec::new();
        }

        let mut keys = Self::sample_keys(&self.buckets, prng, count);
        if let Some(rehash) = &self.rehash {
            let remaining = count.saturating_sub(keys.len());
            keys.extend(Self::sample_keys(&rehash.buckets, prng, remaining));
        }
        keys
    }

    /// Returns the number of entries stored.
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns whether the dictionary is empty.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns whether `key` is present.
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(key).is_some()
    }

    /// Begins rehashing into twice as many buckets, migrating entries one
    /// bucket at a time as later operations run.
    fn start_rehash(&mut self) {
        let new_size = self.buckets.len() * 2;
        self.rehash = Some(Rehash {
            buckets: Self::empty_buckets(new_size),
            index: 0,
        });
    }

    /// Migrates one bucket from the main buckets into the target buckets,
    /// promoting the target to be the main buckets once every bucket has moved.
    /// A no-op when no rehash is in progress.
    fn rehash_step(&mut self) {
        let Some(rehash) = self.rehash.as_mut() else {
            return;
        };
        let entries = std::mem::take(&mut self.buckets[rehash.index]);

        for entry in entries {
            let index = Self::bucket_index(&self.random_state, &entry.key, rehash.buckets.len());
            rehash.buckets[index].push(entry);
        }
        rehash.index += 1;

        if rehash.index == self.buckets.len() {
            self.buckets = self.rehash.take().unwrap().buckets;
        }
    }

    /// Returns `size` empty buckets.
    fn empty_buckets(size: usize) -> Vec<Vec<Entry<K, V>>> {
        (0..size).map(|_| Vec::new()).collect()
    }

    /// Returns the bucket index for `key` among `num_buckets` buckets.
    fn bucket_index<Q>(random_state: &RandomState, key: &Q, num_buckets: usize) -> usize
    where
        Q: Hash + ?Sized,
    {
        (random_state.hash_one(key) as usize) & (num_buckets - 1)
    }

    /// Samples up to `count` keys from `buckets`, scanning buckets in circular
    /// order from a random starting bucket.
    fn sample_keys<'a>(
        buckets: &'a [Vec<Entry<K, V>>],
        prng: &mut Prng,
        count: usize,
    ) -> Vec<&'a K> {
        let num_buckets = buckets.len();
        let start = prng.next_rand() as usize & (num_buckets - 1);
        // Visit every bucket exactly once, in circular order from the random start.
        (start..start + num_buckets)
            .flat_map(|i| buckets[i % num_buckets].iter())
            .map(|e| &e.key)
            .take(count)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_new_returns_none_and_increments_len() {
        let mut dict = Dict::new(4);

        assert_eq!(dict.insert(1, 1), None);
        assert_eq!(dict.len(), 1);
    }

    #[test]
    fn insert_overwrite_returns_old_and_keeps_len() {
        let mut dict = Dict::new(4);
        dict.insert(1, 1);

        assert_eq!(dict.insert(1, 2), Some(1));
        assert_eq!(dict.len(), 1);
    }

    #[test]
    fn get_returns_value_for_present_key() {
        let mut dict = Dict::new(4);
        dict.insert(1, 1);

        assert_eq!(dict.get(&1), Some(&1));
    }

    #[test]
    fn get_returns_none_for_absent_key() {
        let mut dict = Dict::new(4);
        dict.insert(1, 1);

        assert_eq!(dict.get(&2), None);
    }

    #[test]
    fn remove_returns_value_and_decrements() {
        let mut dict = Dict::new(4);
        dict.insert(1, 1);

        assert_eq!(dict.remove(&1), Some(1));
        assert_eq!(dict.len(), 0);
    }

    #[test]
    fn remove_returns_none_for_absent_key() {
        let mut dict = Dict::new(4);
        dict.insert(1, 1);

        assert_eq!(dict.remove(&2), None);
        assert_eq!(dict.len(), 1);
    }

    #[test]
    fn collisions_are_handled() {
        let mut dict = Dict::new(4);
        for i in 0..10 {
            dict.insert(i, i);
        }

        assert_eq!(dict.len(), 10);
        for i in 0..10 {
            assert_eq!(dict.get(&i), Some(&i));
        }

        for i in 0..10 {
            dict.remove(&i);
        }

        assert_eq!(dict.len(), 0);
        assert_eq!(dict.get(&1), None);
    }

    #[test]
    fn grows_and_preserves_entries() {
        let mut dict = Dict::new(4);
        for i in 0..20 {
            dict.insert(i, i);
        }

        assert!(dict.buckets.len() > 4);
        for i in 0..20 {
            assert_eq!(dict.get(&i), Some(&i));
        }
    }

    #[test]
    fn reads_during_active_rehash() {
        let mut dict = Dict::new(4);

        let mut n = 0;
        while dict.rehash.is_none() {
            dict.insert(n, n);
            n += 1;
        }
        assert!(dict.rehash.is_some());

        for i in 0..n {
            assert_eq!(dict.get(&i), Some(&i));
        }
    }

    #[test]
    fn removes_during_active_rehash() {
        let mut dict = Dict::new(4);

        let mut n = 0;
        while dict.rehash.is_none() {
            dict.insert(n, n);
            n += 1;
        }
        assert!(dict.rehash.is_some());

        for i in 0..n {
            assert_eq!(dict.remove(&i), Some(i));
        }
        assert_eq!(dict.len(), 0);
    }

    #[test]
    fn random_keys_returns_existing_keys() {
        let mut dict = Dict::new(16);
        let mut prng = Prng::new(0);
        for i in 0..10 {
            dict.insert(i, i);
        }

        let keys = dict.random_keys(&mut prng, 5);
        assert_eq!(keys.len(), 5);
        for key in keys {
            assert!(dict.get(key).is_some());
        }
    }

    #[test]
    fn random_keys_returns_all_when_count_exceeds_len() {
        let mut dict = Dict::new(4);
        let mut prng = Prng::new(0);
        dict.insert(1, 1);
        dict.insert(2, 2);

        let keys = dict.random_keys(&mut prng, 10);
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn random_keys_samples_both_tables_during_rehash() {
        let mut dict = Dict::new(4);
        let mut prng = Prng::new(0);

        let mut n = 0;
        while dict.rehash.is_none() {
            dict.insert(n, n);
            n += 1;
        }
        assert!(dict.rehash.is_some());

        // Asking for every key must pull from both tables, sampling only the
        // main buckets would return fewer than n.
        let keys = dict.random_keys(&mut prng, n as usize);
        assert_eq!(keys.len(), n as usize);
        for key in keys {
            assert!(dict.get(key).is_some());
        }
    }
}
