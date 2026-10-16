//! Tracks which `(device, inode)` pairs of multiply linked files were already counted.

use std::collections::HashSet;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{Mutex, PoisonError};

const SHARDS: usize = 64;

/// A tiny multiplicative hasher. The keys are inode numbers, which need no DoS resistance, and
/// SipHash would cost more than the lock around it.
#[derive(Default)]
pub(crate) struct FastHasher(u64);

impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.write_u64(u64::from(*byte));
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

type Set = HashSet<(u64, u64), BuildHasherDefault<FastHasher>>;

/// A sharded set, so that worker threads rarely contend on the same lock.
pub(crate) struct Hardlinks {
    shards: Vec<Mutex<Set>>,
}

impl Hardlinks {
    pub(crate) fn new() -> Self {
        Self {
            shards: (0..SHARDS).map(|_| Mutex::new(Set::default())).collect(),
        }
    }

    /// Records the file and returns whether this is the first time it is seen.
    pub(crate) fn first_seen(&self, dev: u64, ino: u64) -> bool {
        let shard = (ino.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 58) as usize % SHARDS;
        self.shards[shard]
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert((dev, ino))
    }
}
