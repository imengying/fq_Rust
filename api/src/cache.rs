use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct TtlCache<V> {
    entries: Arc<DashMap<String, CacheEntry<V>>>,
    ttl: Option<Duration>,
}

#[derive(Clone)]
struct CacheEntry<V> {
    value: V,
    expires_at: Option<Instant>,
}

impl<V: Clone> TtlCache<V> {
    pub fn new(ttl: Option<Duration>) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        let entry = self.entries.get(key)?;
        if is_expired(entry.expires_at) {
            drop(entry);
            self.entries.remove(key);
            return None;
        }
        Some(entry.value.clone())
    }

    pub fn insert(&self, key: impl Into<String>, value: V) {
        let expires_at = self.ttl.map(|ttl| Instant::now() + ttl);
        self.entries.insert(
            key.into(),
            CacheEntry {
                value,
                expires_at,
            },
        );
    }

    /// Remove all expired entries. Call periodically from a background task.
    pub fn sweep_expired(&self) {
        let now = Instant::now();
        self.entries
            .retain(|_, entry| entry.expires_at.map(|exp| now < exp).unwrap_or(true));
    }
}

fn is_expired(expires_at: Option<Instant>) -> bool {
    expires_at
        .map(|value| Instant::now() >= value)
        .unwrap_or(false)
}

