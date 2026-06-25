use std::rc::Rc;

use rustc_hash::FxHashMap;

/// An LRU-cache optimized for reads and being small. It uses the `FxHashMap`
/// for a fast, non-cryptographic hash. This cache is keyed by the raw SQL text
/// (byte-exact), so a lookup never lexes or parses — it hashes the bytes and
/// probes. It was designed in response to slow cache-plan lookups compared to
/// `SQLite`. Redis-like naming to be cheeky.
///
/// Keys are byte-exact: two statements that differ only in whitespace are
/// distinct entries. Re-issuing a statement is the common case, and application
/// code emits byte-stable SQL, so the hit rate stays high without normalization.
pub struct Cache<V> {
    /// Cached pairs of values and tick.
    map: FxHashMap<String, (Rc<V>, usize)>,
    /// Configure cache capacity.
    cap: usize,
    /// Global monotonic counter.
    tick: usize,
}

impl<V> Cache<V> {
    /// Creates a new cache with given capacity.
    pub fn new(capacity: usize) -> Self {
        Cache {
            map: FxHashMap::default(),
            cap: capacity,
            tick: 0,
        }
    }

    /// Gets are cheap, clone the Rc releases the mutable borrow.
    pub fn get(&mut self, key: &str) -> Option<Rc<V>> {
        self.tick += 1;
        let (val, ts) = self.map.get_mut(key)?;
        *ts = self.tick;
        Some(Rc::clone(val))
    }

    /// Puts are "slow" because they do the maintenance which is ok.
    pub fn put(&mut self, key: &str, val: V) -> Rc<V> {
        self.tick += 1;
        let rc = Rc::new(val);
        self.map.insert(key.to_owned(), (Rc::clone(&rc), self.tick));
        if self.map.len() > self.cap
            && let Some(lru) = self
                .map
                .iter()
                .min_by_key(|(_, (_, ts))| *ts)
                .map(|(k, _)| k.clone())
        {
            self.map.remove(&lru);
        }
        rc
    }

    /// Deletes are cheap, no maintenance.
    pub fn del(&mut self, key: &str) {
        self.map.remove(key);
    }

    /// Returns true iff the value is in the cache
    #[allow(unused)]
    pub fn exists(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }
}
