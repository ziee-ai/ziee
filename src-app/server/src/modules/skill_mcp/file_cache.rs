//! Per-process LRU cache for skill body + reference reads.
//!
//! Sized to 64 MiB / 5-minute TTL with FIFO-eviction (good enough for
//! the access pattern: a small set of skills loaded repeatedly across
//! a chat session; rare invalidations on edit).
//!
//! Key: `(skill_id, file_relative_path, mtime_nanos)`. The mtime
//! component is the implicit invalidator — a re-extracted bundle
//! changes mtime on every file. Explicit invalidation for editable
//! metadata is via `invalidate_skill(skill_id)` from the skill update
//! / delete handlers.
//!
//! The cache is process-local. Multi-process deployments don't share;
//! that's acceptable for a read-only content cache (worst case is N
//! cold reads instead of 1).

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use uuid::Uuid;

const CAPACITY_BYTES: usize = 64 * 1024 * 1024; // 64 MiB
const TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct CacheKey {
    pub skill_id: Uuid,
    pub rel_path: String,
    pub mtime_nanos: i128,
    /// M-5: `load_skill` caches the frontmatter-STRIPPED body of SKILL.md,
    /// while `read_skill_file("SKILL.md")` caches the RAW file. Both share
    /// (skill_id, "SKILL.md", mtime); without this discriminator they collide
    /// and return each other's content. `true` = stripped body.
    pub stripped: bool,
}

struct Entry {
    content: String,
    inserted_at: Instant,
}

struct Inner {
    map: HashMap<CacheKey, Entry>,
    insertion_order: Vec<CacheKey>,
    total_bytes: usize,
}

impl Inner {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            insertion_order: Vec::new(),
            total_bytes: 0,
        }
    }

    fn get(&mut self, key: &CacheKey) -> Option<String> {
        let entry = self.map.get(key)?;
        if entry.inserted_at.elapsed() > TTL {
            // Expired — drop it.
            let bytes = entry.content.len();
            self.map.remove(key);
            self.insertion_order.retain(|k| k != key);
            self.total_bytes = self.total_bytes.saturating_sub(bytes);
            return None;
        }
        Some(entry.content.clone())
    }

    fn put(&mut self, key: CacheKey, content: String) {
        // Replace existing entry if present (same key).
        if let Some(old) = self.map.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(old.content.len());
            self.insertion_order.retain(|k| k != &key);
        }
        let bytes = content.len();
        // Evict FIFO until there's room. (LRU would require touch-on-get;
        // this is good enough for the read-mostly access pattern.)
        while self.total_bytes + bytes > CAPACITY_BYTES && !self.insertion_order.is_empty() {
            let oldest = self.insertion_order.remove(0);
            if let Some(e) = self.map.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(e.content.len());
            }
        }
        self.insertion_order.push(key.clone());
        self.map.insert(
            key,
            Entry {
                content,
                inserted_at: Instant::now(),
            },
        );
        self.total_bytes += bytes;
    }

    fn invalidate_skill(&mut self, skill_id: Uuid) {
        let keys: Vec<CacheKey> = self
            .map
            .keys()
            .filter(|k| k.skill_id == skill_id)
            .cloned()
            .collect();
        for k in keys {
            if let Some(e) = self.map.remove(&k) {
                self.total_bytes = self.total_bytes.saturating_sub(e.content.len());
            }
            self.insertion_order.retain(|kk| kk != &k);
        }
    }
}

static CACHE: OnceLock<Mutex<Inner>> = OnceLock::new();

fn cache() -> &'static Mutex<Inner> {
    CACHE.get_or_init(|| Mutex::new(Inner::new()))
}

pub fn get(key: &CacheKey) -> Option<String> {
    // Poisoned mutex → recover; this is a content cache, no invariants
    // to protect.
    let mut guard = cache().lock().unwrap_or_else(|p| p.into_inner());
    guard.get(key)
}

pub fn put(key: CacheKey, content: String) {
    let mut guard = cache().lock().unwrap_or_else(|p| p.into_inner());
    guard.put(key, content);
}

pub fn invalidate_skill(skill_id: Uuid) {
    let mut guard = cache().lock().unwrap_or_else(|p| p.into_inner());
    guard.invalidate_skill(skill_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: Uuid, path: &str) -> CacheKey {
        CacheKey {
            skill_id: id,
            rel_path: path.to_string(),
            mtime_nanos: 1,
            stripped: false,
        }
    }

    #[test]
    fn put_then_get_returns_same_content() {
        let id = Uuid::new_v4();
        let k = key(id, "SKILL.md");
        put(k.clone(), "hello".to_string());
        assert_eq!(get(&k).as_deref(), Some("hello"));
    }

    #[test]
    fn invalidate_skill_drops_only_matching_entries() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        put(key(a, "f1"), "a1".to_string());
        put(key(a, "f2"), "a2".to_string());
        put(key(b, "f1"), "b1".to_string());
        invalidate_skill(a);
        assert!(get(&key(a, "f1")).is_none());
        assert!(get(&key(a, "f2")).is_none());
        assert_eq!(get(&key(b, "f1")).as_deref(), Some("b1"));
    }

    #[test]
    fn put_same_key_replaces_content_bytes_accounting() {
        let id = Uuid::new_v4();
        let k = key(id, "SKILL.md");
        put(k.clone(), "small".to_string());
        put(k.clone(), "smaller".to_string()); // replaces
        assert_eq!(get(&k).as_deref(), Some("smaller"));
    }

    // audit id all-32c98a5e0b5b — resilience of the MCP skill-content cache: the
    // global cache mutex is intentionally poison-RECOVERING
    // (`lock().unwrap_or_else(|p| p.into_inner())`) so a panic in one request
    // while holding the lock can't wedge the cache for every later skill read.
    // Poison the mutex from a panicking thread, then assert get/put still work.
    #[test]
    fn cache_survives_a_poisoned_mutex() {
        let id = Uuid::new_v4();
        let k = key(id, "poison.md");
        put(k.clone(), "before".to_string());

        // Poison the global mutex by panicking while it's held.
        let _ = std::thread::spawn(|| {
            let _guard = cache().lock().unwrap();
            panic!("intentional poison");
        })
        .join();

        // Despite the poisoned lock, reads + writes recover and succeed.
        assert_eq!(
            get(&k).as_deref(),
            Some("before"),
            "get must recover from a poisoned cache mutex"
        );
        let k2 = key(id, "after.md");
        put(k2.clone(), "after".to_string());
        assert_eq!(
            get(&k2).as_deref(),
            Some("after"),
            "put must recover from a poisoned cache mutex"
        );
    /// Resilience: an entry older than the 5-minute TTL is treated as a MISS and
    /// dropped on access (Inner::get TTL branch). Tested against a LOCAL Inner so
    /// it can't race the process-global CACHE the other tests use. Backdates
    /// `inserted_at` instead of sleeping 5 minutes.
    #[test]
    fn expired_entries_are_evicted_on_get() {
        let mut inner = Inner::new();
        let k = key(Uuid::new_v4(), "SKILL.md");
        inner.put(k.clone(), "hello".to_string());
        assert_eq!(inner.get(&k).as_deref(), Some("hello"), "fresh entry hits");

        // Make the entry older than TTL.
        inner.map.get_mut(&k).unwrap().inserted_at =
            Instant::now().checked_sub(TTL + Duration::from_secs(1)).unwrap();

        assert!(inner.get(&k).is_none(), "expired entry must be a miss");
        assert_eq!(inner.total_bytes, 0, "expired entry must be dropped from accounting");
    }

    /// Resilience: FIFO capacity eviction — putting past CAPACITY_BYTES evicts the
    /// OLDEST entry first (Inner::put eviction loop). Local Inner; ~40 MiB strings
    /// so two entries exceed the 64 MiB cap.
    #[test]
    fn fifo_eviction_drops_oldest_past_capacity() {
        let mut inner = Inner::new();
        let id = Uuid::new_v4();
        let big = "x".repeat(40 * 1024 * 1024); // 40 MiB each → two exceed 64 MiB

        inner.put(key(id, "older"), big.clone());
        inner.put(key(id, "newer"), big.clone()); // forces eviction of "older"

        assert!(inner.get(&key(id, "older")).is_none(), "oldest entry must be evicted (FIFO)");
        assert_eq!(
            inner.get(&key(id, "newer")).map(|s| s.len()),
            Some(40 * 1024 * 1024),
            "the most-recent entry survives"
        );
        assert!(inner.total_bytes <= CAPACITY_BYTES, "pool must stay within the cap");
    }
}
