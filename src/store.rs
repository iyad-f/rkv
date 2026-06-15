// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The key-value store and the expiry deadlines that drive key expiration.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::dict::Dict;
use crate::prng::Prng;

/// The expiry state of a key.
pub enum Expiry {
    /// The key does not exist.
    Missing,

    /// The key exists but has no expiry.
    Never,

    /// The key expires at this deadline, in milliseconds since the Unix epoch.
    At(i64),
}

/// A key-value store that tracks an optional expiry deadline per key.
pub struct Store {
    /// The stored values.
    data: Dict<Vec<u8>, Vec<u8>>,

    /// Absolute deadlines, in milliseconds since the Unix epoch, for the keys
    /// that have an expiry. A key without an expiry is absent.
    deadlines: Dict<Vec<u8>, i64>,
}

impl Store {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self {
            data: Dict::new(16),
            deadlines: Dict::new(16),
        }
    }

    /// Returns the number of keys stored, including any expired but not yet reaped.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns the value at `key`, or `None` if it is missing or has expired.
    pub fn get(&mut self, key: &[u8]) -> Option<&Vec<u8>> {
        self.remove_if_expired(key);
        self.data.get(key)
    }

    /// Reports whether `key` exists, treating an expired key as missing.
    pub fn contains_key(&mut self, key: &[u8]) -> bool {
        self.remove_if_expired(key);
        self.data.contains_key(key)
    }

    /// Stores `value` at `key`, discarding any existing expiry.
    pub fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.deadlines.remove(&key);
        self.data.insert(key, value);
    }

    /// Stores `value` at `key`, preserving any existing expiry.
    pub fn update(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.data.insert(key, value);
    }

    /// Removes `key` and any expiry, returning whether it existed. An expired
    /// key counts as already gone.
    pub fn remove(&mut self, key: &[u8]) -> bool {
        self.remove_if_expired(key);
        self.deadlines.remove(key);
        self.data.remove(key).is_some()
    }

    /// Sets `key`'s expiry to the absolute `deadline`, in milliseconds since
    /// the Unix epoch.
    pub fn set_expiry(&mut self, key: &[u8], deadline: i64) {
        self.deadlines.insert(key.to_vec(), deadline);
    }

    /// Removes `key`'s expiry, returning whether one was removed.
    pub fn persist(&mut self, key: &[u8]) -> bool {
        if !self.contains_key(key) {
            return false;
        }
        self.deadlines.remove(key).is_some()
    }

    /// Returns the expiry state of `key`.
    pub fn expiry(&mut self, key: &[u8]) -> Expiry {
        if !self.contains_key(key) {
            return Expiry::Missing;
        }
        match self.deadlines.get(key) {
            Some(&deadline) => Expiry::At(deadline),
            None => Expiry::Never,
        }
    }

    /// Actively reaps expired keys by sampling keys that have a deadline and
    /// removing those whose deadline has passed, repeating while a sample comes
    /// back mostly stale, bounded by a time limit so a large backlog clears
    /// across cycles instead of stalling the caller.
    pub fn expire_cycle(&mut self, prng: &mut Prng) {
        // This runs on the same thread that serves client commands, so scanning
        // every key that has a deadline would stall request handling. Instead we
        // estimate from a random sample of up to `SAMPLE_SIZE` keys. The share of
        // the sample that turns out expired approximates the share across all keys
        // with a deadline. A mostly stale sample, more than `STALE_THRESHOLD_PERCENT`%
        // expired, means many expired keys likely remain, so we sample again, while
        // a mostly fresh one means little is left to reclaim, so we stop. Repeated
        // sampling can still run long when a large share expired at once, so each
        // cycle is capped by `TIME_LIMIT`, and any remaining keys are reclaimed on
        // later ticks.

        const SAMPLE_SIZE: usize = 20;
        const STALE_THRESHOLD_PERCENT: usize = 25;
        const TIME_LIMIT: Duration = Duration::from_millis(25);

        let start = Instant::now();
        let now = Self::now();

        while !self.deadlines.is_empty() && start.elapsed() < TIME_LIMIT {
            let count = SAMPLE_SIZE.min(self.deadlines.len());
            let sample: Vec<Vec<u8>> = self
                .deadlines
                .random_keys(prng, count)
                .into_iter()
                .cloned()
                .collect();

            let mut expired = 0;
            for key in &sample {
                if self
                    .deadlines
                    .get(key)
                    .is_some_and(|&deadline| now > deadline)
                {
                    self.remove_entry(key);
                    expired += 1;
                }
            }

            if expired * 100 <= sample.len() * STALE_THRESHOLD_PERCENT {
                break;
            }
        }
    }

    /// The current wall-clock time, in milliseconds since the Unix epoch.
    pub fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is set before the Unix epoch")
            .as_millis() as i64
    }

    /// Removes `key` from both maps if it has expired.
    fn remove_if_expired(&mut self, key: &[u8]) {
        if self
            .deadlines
            .get(key)
            .is_some_and(|&deadline| Self::now() > deadline)
        {
            self.remove_entry(key);
        }
    }

    /// Removes `key` from both the value and deadline maps.
    fn remove_entry(&mut self, key: &[u8]) {
        self.data.remove(key);
        self.deadlines.remove(key);
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_lazily_expires_a_past_deadline() {
        let mut store = Store::new();
        store.set(b"k".to_vec(), b"v".to_vec());
        store.set_expiry(b"k", 1);

        assert!(store.get(b"k").is_none());
        assert!(!store.contains_key(b"k"));
    }

    #[test]
    fn read_keeps_a_future_deadline() {
        let mut store = Store::new();
        store.set(b"k".to_vec(), b"v".to_vec());
        store.set_expiry(b"k", Store::now() + 100_000);

        assert_eq!(store.get(b"k"), Some(&b"v".to_vec()));
    }

    #[test]
    fn set_clears_an_existing_expiry() {
        let mut store = Store::new();
        store.set(b"k".to_vec(), b"v".to_vec());
        store.set_expiry(b"k", Store::now() + 100_000);

        store.set(b"k".to_vec(), b"v2".to_vec());

        assert!(matches!(store.expiry(b"k"), Expiry::Never));
    }

    #[test]
    fn update_preserves_an_existing_expiry() {
        let mut store = Store::new();
        store.set(b"k".to_vec(), b"v".to_vec());
        store.set_expiry(b"k", Store::now() + 100_000);

        store.update(b"k".to_vec(), b"v2".to_vec());

        assert!(matches!(store.expiry(b"k"), Expiry::At(_)));
    }

    #[test]
    fn persist_reports_whether_an_expiry_was_removed() {
        let mut store = Store::new();
        store.set(b"k".to_vec(), b"v".to_vec());
        assert!(!store.persist(b"k"));

        store.set_expiry(b"k", Store::now() + 100_000);
        assert!(store.persist(b"k"));
        assert!(matches!(store.expiry(b"k"), Expiry::Never));
    }

    #[test]
    fn expire_cycle_reaps_a_past_deadline() {
        let mut store = Store::new();
        let mut prng = Prng::new(0);

        store.set(b"k".to_vec(), b"v".to_vec());
        store.set_expiry(b"k", 1);
        store.expire_cycle(&mut prng);

        assert_eq!(store.data.len(), 0);
        assert_eq!(store.deadlines.len(), 0);
    }

    #[test]
    fn expire_cycle_keeps_a_future_deadline() {
        let mut store = Store::new();
        let mut prng = Prng::new(0);

        store.set(b"k".to_vec(), b"v".to_vec());
        store.set_expiry(b"k", Store::now() + 100_000);
        store.expire_cycle(&mut prng);

        assert_eq!(store.data.len(), 1);
        assert_eq!(store.deadlines.len(), 1);
    }

    #[test]
    fn expire_cycle_leaves_keys_without_a_deadline() {
        let mut store = Store::new();
        let mut prng = Prng::new(0);

        store.set(b"k".to_vec(), b"v".to_vec());
        store.expire_cycle(&mut prng);

        assert_eq!(store.data.len(), 1);
    }

    #[test]
    fn expire_cycle_reaps_past_the_sample_size_when_mostly_stale() {
        let mut store = Store::new();
        let mut prng = Prng::new(0);

        for i in 0..100u8 {
            store.set(vec![i], b"v".to_vec());
            store.set_expiry(&[i], 1);
        }
        store.expire_cycle(&mut prng);

        assert_eq!(store.data.len(), 0);
        assert_eq!(store.deadlines.len(), 0);
    }
}
