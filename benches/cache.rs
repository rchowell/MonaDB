//! Microbenchmark: Cache<V> (src/cache.rs) vs old PlanCache
//! (HashMap + VecDeque + deep-clone, reproduced inline).
//!
//! The value type `MockStmt` approximates `PreparedStatement`'s clone cost:
//! one heap String + one heap Vec.
//!
//! Run:
//!   cargo bench --bench cache

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use monadb::Cache;

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

fn fill_new() -> Cache<MockStmt> {
    let mut c = Cache::new(CAP);
    for i in 0..CAP {
        let k = format!("select col_{i} from t;");
        c.put(&k, MockStmt::of(&k));
    }
    c.put(HOT, MockStmt::of(HOT));
    c
}

// Warm thread-locals for the get benchmarks (steady-state hit, page-cache hot).
// The put benchmarks use with_inputs to reset state each iteration.
thread_local! {
    static OLD: RefCell<OldCache>           = RefCell::new(fill_old());
    static NEW: RefCell<Cache<MockStmt>>    = RefCell::new(fill_new());
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

// get — new: Rc refcount bump + u64 write, O(1)
#[divan::bench]
fn get_new() -> Rc<MockStmt> {
    NEW.with(|c| c.borrow_mut().get(HOT).unwrap())
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

// put — new: O(1) insert + O(cap) eviction scan only when over cap
#[divan::bench]
fn put_new(bencher: divan::Bencher) {
    bencher.with_inputs(fill_new).bench_values(|mut c| {
        c.put("select new_key;", MockStmt::of("select new_key;"));
    });
}
