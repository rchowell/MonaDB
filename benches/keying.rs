//! Microbenchmark: plan-cache *key* strategies — the gate that picked raw-SQL
//! keying over a semantic hash (see `docs/references/caching-plans.md`, iter 7).
//! All three strategies are reproduced **inline** (like `benches/cache.rs`) so
//! the comparison survives `src/` changes — notably, this bench keeps the old
//! lex-fold alive even though `SqlLexer` no longer hashes:
//!
//!   1. `lex`   — semantic hash by folding the token stream → `u64` key.
//!   2. `parse` — a full parse (builds + drops the AST) on the keying path → `u64`.
//!   3. `str`   — FxHash of the raw bytes → `String` key (no lex, no parse).
//!
//! Groups: `derive_*` (pure key cost), `hit_*` (derive + warm-cache get → `Rc`),
//! `miss_*` (the keying work a miss pays before the constant bind/compile).
//!
//! Run:
//!   cargo bench --bench keying

use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHasher};

use monadb::parser::SqlParser;
use monadb::{SqlLexer, Token};

// ── value stand-in ──────────────────────────────────────────────────────────

#[derive(Clone)]
#[allow(dead_code)]
struct MockPlan {
    sql: String,
    program: Vec<u64>,
}

impl MockPlan {
    fn of(sql: &str) -> Self {
        MockPlan { sql: sql.to_owned(), program: vec![0, 1, 2] }
    }
}

// ── key derivations (the three strategies, reproduced inline) ────────────────

/// Strategy 1: fold the token stream into a `u64` (the former `SqlLexer` hash).
fn klex(sql: &str) -> u64 {
    let mut h = FxHasher::default();
    for item in SqlLexer::new(sql) {
        let Ok((_, tok, _)) = item else { break };
        std::mem::discriminant(&tok).hash(&mut h);
        match &tok {
            Token::Number(s)
            | Token::Identifier(s)
            | Token::String(s)
            | Token::NumberedParam(s)
            | Token::NamedParam(s) => s.hash(&mut h),
            _ => {}
        }
    }
    h.finish()
}

/// Strategy 2: a full parse (builds + drops the AST) plus the `u64` key. The
/// rejected parse-first design read the hash off the parser's own lexer; that
/// mechanism was removed with the pivot, so we approximate with `klex` — a slight
/// over-count of the key extraction, immaterial since #2 is dominated by the
/// ~700 ns parse and was rejected anyway.
fn kparse(sql: &str) -> u64 {
    let lex = SqlLexer::new(sql);
    let pos = Cell::new(0u32);
    let _ast = SqlParser::new().parse(&pos, lex).expect("parse");
    klex(sql)
}

/// Strategy 3: FxHash of the raw bytes — the String path's get-time key work.
fn kstr(sql: &str) -> u64 {
    let mut h = FxHasher::default();
    sql.hash(&mut h);
    h.finish()
}

// ── inlined caches (reproduced like `benches/cache.rs`) ──────────────────────

struct U64Cache<V> {
    map: FxHashMap<u64, (Rc<V>, usize)>,
    cap: usize,
    tick: usize,
}

impl<V> U64Cache<V> {
    fn new(cap: usize) -> Self {
        U64Cache { map: FxHashMap::default(), cap, tick: 0 }
    }
    fn get(&mut self, key: u64) -> Option<Rc<V>> {
        self.tick += 1;
        let (val, ts) = self.map.get_mut(&key)?;
        *ts = self.tick;
        Some(Rc::clone(val))
    }
    fn put(&mut self, key: u64, val: V) {
        self.tick += 1;
        let rc = Rc::new(val);
        self.map.insert(key, (rc, self.tick));
        if self.map.len() > self.cap
            && let Some(&lru) = self.map.iter().min_by_key(|(_, (_, ts))| *ts).map(|(k, _)| k)
        {
            self.map.remove(&lru);
        }
    }
}

struct StringCache<V> {
    map: FxHashMap<String, (Rc<V>, usize)>,
    cap: usize,
    tick: usize,
}

impl<V> StringCache<V> {
    fn new(cap: usize) -> Self {
        StringCache { map: FxHashMap::default(), cap, tick: 0 }
    }
    fn get(&mut self, key: &str) -> Option<Rc<V>> {
        self.tick += 1;
        let (val, ts) = self.map.get_mut(key)?;
        *ts = self.tick;
        Some(Rc::clone(val))
    }
    fn put(&mut self, key: &str, val: V) {
        self.tick += 1;
        let rc = Rc::new(val);
        self.map.insert(key.to_owned(), (rc, self.tick));
        if self.map.len() > self.cap
            && let Some(lru) =
                self.map.iter().min_by_key(|(_, (_, ts))| *ts).map(|(k, _)| k.clone())
        {
            self.map.remove(&lru);
        }
    }
}

// ── setup ────────────────────────────────────────────────────────────────────

const CAP: usize = 256;

/// Representative SQL shapes the cache actually serves.
const INPUTS: &[&str] = &[
    "select t[123];",              // point lookup
    "select docs[\"t042\", 5700];", // composite key
    "select * from t limit 10;",   // scan
    "select t[?];",                // param (prepared-style)
];

fn fill_u64() -> U64Cache<MockPlan> {
    let mut c = U64Cache::new(CAP);
    for i in 0..CAP {
        let k = format!("select col_{i} from t;");
        c.put(klex(&k), MockPlan::of(&k));
    }
    // Hot inputs inserted last → MRU, survive eviction. Keyed by the semantic
    // lex hash so both `klex` and `kparse` lookups hit (identical fold).
    for sql in INPUTS {
        c.put(klex(sql), MockPlan::of(sql));
    }
    c
}

fn fill_string() -> StringCache<MockPlan> {
    let mut c = StringCache::new(CAP);
    for i in 0..CAP {
        let k = format!("select col_{i} from t;");
        c.put(&k, MockPlan::of(&k));
    }
    for sql in INPUTS {
        c.put(sql, MockPlan::of(sql));
    }
    c
}

thread_local! {
    static U64: RefCell<U64Cache<MockPlan>> = RefCell::new(fill_u64());
    static STRING: RefCell<StringCache<MockPlan>> = RefCell::new(fill_string());
}

fn main() {
    divan::main();
}

// ── derive: pure key cost ─────────────────────────────────────────────────────

#[divan::bench(args = INPUTS)]
fn derive_lex(sql: &str) -> u64 {
    klex(sql)
}

#[divan::bench(args = INPUTS)]
fn derive_parse(sql: &str) -> u64 {
    kparse(sql)
}

#[divan::bench(args = INPUTS)]
fn derive_str(sql: &str) -> u64 {
    kstr(sql)
}

// ── hit: derive key + cache get (the ad-hoc steady-state hot path) ────────────

#[divan::bench(args = INPUTS)]
fn hit_lex_u64(sql: &str) -> Rc<MockPlan> {
    let key = klex(sql);
    U64.with(|c| c.borrow_mut().get(key).unwrap())
}

#[divan::bench(args = INPUTS)]
fn hit_parse_u64(sql: &str) -> Rc<MockPlan> {
    let key = kparse(sql);
    U64.with(|c| c.borrow_mut().get(key).unwrap())
}

#[divan::bench(args = INPUTS)]
fn hit_string(sql: &str) -> Rc<MockPlan> {
    STRING.with(|c| c.borrow_mut().get(sql).unwrap())
}

// ── miss: keying work before the (constant) bind/compile ──────────────────────

// Strategy 1: lex for the key, THEN parse to compile → the SQL is lexed twice.
#[divan::bench(args = INPUTS)]
fn miss_lex_u64(sql: &str) -> u64 {
    let key = klex(sql);
    let _ = kparse(sql);
    key
}

// Strategy 2: a single parse yields both the key and the AST to compile.
#[divan::bench(args = INPUTS)]
fn miss_parse_u64(sql: &str) -> u64 {
    kparse(sql)
}

// Strategy 3: hash bytes for the key, parse once to compile.
#[divan::bench(args = INPUTS)]
fn miss_string(sql: &str) -> u64 {
    let key = kstr(sql);
    let _ = kparse(sql);
    key
}
