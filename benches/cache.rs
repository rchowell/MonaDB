//! Microbenchmark: plan-cache key strategies, all reproduced inline so the
//! comparison is stable regardless of `src/cache.rs`:
//!   - `old`    — HashMap + VecDeque + deep-clone (the pre-refactor PlanCache)
//!   - `string` — FxHashMap<String> + Rc (the current raw-SQL-keyed Cache)
//!   - `u64`    — FxHashMap<u64> + Rc (the prior semantic-hash Cache)
//!
//! The value type `MockStmt` approximates `PreparedStatement`'s clone cost:
//! one heap String + one heap Vec.
//!
//! Run:
//!   cargo bench --bench cache

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHasher};

// ── value type ────────────────────────────────────────────────────────────────

// Approximates PreparedStatement: two independently heap-allocated fields so
// each Clone() costs two allocations (matching the old deep-clone path).
#[derive(Clone)]
#[allow(dead_code)]
struct MockStmt {
    sql: String,
    params: Vec<u64>,
}

impl MockStmt {
    fn of(sql: &str) -> Self {
        MockStmt {
            sql: sql.to_owned(),
            params: vec![0, 1, 2],
        }
    }
}

// ── old: HashMap + VecDeque + deep-clone ───────────────────────────────────
//
// Reproduced from the pre-refactor PlanCache in src/lib.rs:
//   get  → clone value (2 heap allocs) + O(cap) VecDeque scan + String alloc
//   put  → O(cap) detach scan + insert + push_back + evict

struct OldCache {
    plans: HashMap<String, MockStmt>,
    order: VecDeque<String>,
    cap: usize,
}

impl OldCache {
    fn new(cap: usize) -> Self {
        OldCache {
            plans: HashMap::new(),
            order: VecDeque::new(),
            cap,
        }
    }

    fn get(&mut self, key: &str) -> Option<MockStmt> {
        let plan = self.plans.get(key)?.clone();
        self.detach_order(key);
        self.order.push_back(key.to_owned());
        Some(plan)
    }

    fn put(&mut self, key: String, val: MockStmt) {
        self.detach_order(&key);
        self.plans.insert(key.clone(), val);
        self.order.push_back(key);
        while self.plans.len() > self.cap {
            if let Some(lru) = self.order.pop_front() {
                self.plans.remove(&lru);
            }
        }
    }

    fn detach_order(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
    }
}

// ── string-keyed Cache (FxHashMap<String> — the current src/cache.rs) ───────

struct StringCache<V> {
    map: FxHashMap<String, (Rc<V>, usize)>,
    cap: usize,
    tick: usize,
}

impl<V> StringCache<V> {
    fn new(capacity: usize) -> Self {
        StringCache {
            map: FxHashMap::default(),
            cap: capacity,
            tick: 0,
        }
    }

    fn get(&mut self, key: &str) -> Option<Rc<V>> {
        self.tick += 1;
        let (val, ts) = self.map.get_mut(key)?;
        *ts = self.tick;
        Some(Rc::clone(val))
    }

    fn put(&mut self, key: &str, val: V) -> Rc<V> {
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
}

// ── u64-keyed Cache (FxHashMap<u64> — the prior src/cache.rs) ────────────────
//
// Inlined (like `OldCache`/`StringCache`) so the String-key vs u64-key delta is
// measured in one run.

struct U64Cache<V> {
    map: FxHashMap<u64, (Rc<V>, usize)>,
    cap: usize,
    tick: usize,
}

impl<V> U64Cache<V> {
    fn new(capacity: usize) -> Self {
        U64Cache {
            map: FxHashMap::default(),
            cap: capacity,
            tick: 0,
        }
    }

    fn get(&mut self, key: u64) -> Option<Rc<V>> {
        self.tick += 1;
        let (val, ts) = self.map.get_mut(&key)?;
        *ts = self.tick;
        Some(Rc::clone(val))
    }

    fn put(&mut self, key: u64, val: V) -> Rc<V> {
        self.tick += 1;
        let rc = Rc::new(val);
        self.map.insert(key, (Rc::clone(&rc), self.tick));
        if self.map.len() > self.cap
            && let Some(&lru) = self
                .map
                .iter()
                .min_by_key(|(_, (_, ts))| *ts)
                .map(|(k, _)| k)
        {
            self.map.remove(&lru);
        }
        rc
    }
}

/// FxHash of a SQL key — the per-call key-derivation cost the u64 path pays at
/// the cache boundary (vs the String path hashing the whole template string).
fn hash_key(sql: &str) -> u64 {
    let mut h = FxHasher::default();
    sql.hash(&mut h);
    h.finish()
}

// ── setup ─────────────────────────────────────────────────────────────────────

const CAP: usize = 256;
const HOT: &str = "select t[?];";

fn fill_old() -> OldCache {
    let mut c = OldCache::new(CAP);
    for i in 0..CAP {
        let k = format!("select col_{i} from t;");
        c.put(k.clone(), MockStmt::of(&k));
    }
    // HOT goes in last — MRU, won't be evicted. Subsequent gets must scan
    // to position cap-1 to find and relink it: worst-case O(cap) scan.
    c.put(HOT.to_owned(), MockStmt::of(HOT));
    c
}

fn fill_string() -> StringCache<MockStmt> {
    let mut c = StringCache::new(CAP);
    for i in 0..CAP {
        let k = format!("select col_{i} from t;");
        c.put(&k, MockStmt::of(&k));
    }
    c.put(HOT, MockStmt::of(HOT));
    c
}

fn fill_u64() -> U64Cache<MockStmt> {
    let mut c = U64Cache::new(CAP);
    for i in 0..CAP {
        let k = format!("select col_{i} from t;");
        c.put(hash_key(&k), MockStmt::of(&k));
    }
    c.put(hash_key(HOT), MockStmt::of(HOT));
    c
}

// Warm thread-locals for the get benchmarks (steady-state hit, page-cache hot).
// The put benchmarks use with_inputs to reset state each iteration.
thread_local! {
    static OLD: RefCell<OldCache>             = RefCell::new(fill_old());
    static STRING: RefCell<StringCache<MockStmt>> = RefCell::new(fill_string());
    static U64: RefCell<U64Cache<MockStmt>>   = RefCell::new(fill_u64());
}

// ── benchmarks ────────────────────────────────────────────────────────────────

fn main() {
    divan::main();
}

// get — old: clone(sql String + params Vec) + O(256) VecDeque scan + String alloc
#[divan::bench]
fn get_old() -> MockStmt {
    OLD.with(|c| c.borrow_mut().get(HOT).unwrap())
}

// get — string: hash the HOT String + String eq, O(1)
#[divan::bench]
fn get_string() -> Rc<MockStmt> {
    STRING.with(|c| c.borrow_mut().get(HOT).unwrap())
}

// put — old: O(cap) detach scan on new key (no-op) + insert + evict
#[divan::bench]
fn put_old(bencher: divan::Bencher) {
    bencher.with_inputs(fill_old).bench_values(|mut c| {
        c.put(
            "select new_key;".to_owned(),
            MockStmt::of("select new_key;"),
        );
    });
}

// put — string: O(1) insert (`key.to_owned()`) + O(cap) eviction scan + String
// clone of the evicted key.
#[divan::bench]
fn put_string(bencher: divan::Bencher) {
    bencher.with_inputs(fill_string).bench_values(|mut c| {
        c.put("select new_key;", MockStmt::of("select new_key;"));
    });
}

// get — u64: hash a u64 + u64 eq, O(1). Key derived outside the timed body
// (deriving it is the lexer's job, measured in `benches/normalize.rs`), so this
// isolates the cache-boundary cost vs `get_string`'s long-String hash + eq.
#[divan::bench]
fn get_u64(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| hash_key(HOT))
        .bench_values(|key| U64.with(|c| c.borrow_mut().get(key).unwrap()));
}

// put — u64: O(1) insert (no `key.to_owned()`) + O(cap) eviction scan; the
// evicted key is `Copy`, so eviction sheds the String clone `put_string` pays.
#[divan::bench]
fn put_u64(bencher: divan::Bencher) {
    let key = hash_key("select new_key;");
    bencher.with_inputs(fill_u64).bench_values(move |mut c| {
        c.put(key, MockStmt::of("select new_key;"));
    });
}
