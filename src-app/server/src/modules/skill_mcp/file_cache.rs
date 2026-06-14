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
}
