//! iai-callgrind microbenchmark: PlanCache cache-hit path.
//!
//! Measures instruction counts, L1/LLC cache hits, and estimated cycles for:
//!   - `query_with` on a primed cache — isolates `PlanCache::get` + `execute_prepared`
//!   - `query` point-lookup on a primed cache — normalize → cache hit → btree get
//!
//! Install the runner, then run:
//!   cargo install iai-callgrind-runner --version 0.14
//!   cargo bench --bench plan_cache

use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use monadb::{MonaDB, Params, Value};

// ── setup helpers ─────────────────────────────────────────────────────────────
// These run OUTSIDE Callgrind instrumentation; only the benchmark body is counted.

fn primed_select() -> MonaDB {
    let mut db = MonaDB::memory().unwrap();
    // First call is a cache miss; the bench call below is a pure hit.
    db.query_with("select $1;", &Params::positional(vec![Value::int(0)]))
        .unwrap()
        .finish()
        .unwrap();
    db
}

fn primed_keyed_table() -> MonaDB {
    let mut db = MonaDB::memory().unwrap();
    db.execute("create table t (id int);").unwrap();
    for i in 1..=1000_i64 {
        db.execute(&format!(r#"insert into t ({{"id": {i}}});"#))
            .unwrap();
    }
    // Any key normalizes to "select t[?];", so 500 primes the plan for 1.
    db.query("select t[500];").unwrap().finish().unwrap();
    db
}

/// A keyed table whose prepared `select t[?];` plan is already warm, so the
/// bench body is a pure cached-statement execute — the `normalize`-free ceiling.
fn primed_keyed_table_prepared() -> MonaDB {
    let mut db = primed_keyed_table();
    // Prime the LMDB page cache (and, post-01C, the baked handle) with one run.
    db.prepare_cached("select t[?];")
        .unwrap()
        .query([Value::int(500)])
        .unwrap()
        .finish()
        .unwrap();
    db
}

// ── benchmarks ────────────────────────────────────────────────────────────────

// Cache-hit dispatch overhead: `PlanCache::get` + `execute_prepared` on a trivial query.
// Before: deep-clone PreparedStatement + O(256) VecDeque scan + String alloc.
// After:  Rc refcount bump + u64 write.
#[library_benchmark]
#[bench::primed(primed_select())]
fn query_with_hit(mut db: MonaDB) -> u64 {
    db.query_with("select $1;", &Params::positional(vec![Value::int(1)]))
        .unwrap()
        .finish()
        .unwrap()
}

// End-to-end point lookup: normalize → cache hit → keyed btree get.
// The primary latency target; cache overhead is a meaningful fraction because
// the btree get itself is cheap at small N.
#[library_benchmark]
#[bench::k1000(primed_keyed_table())]
fn point_lookup(mut db: MonaDB) -> Option<Value> {
    db.query("select t[1];").unwrap().next().unwrap()
}

// Prepared point lookup: execute_prepared → keyed btree get, with NO per-call
// `normalize`. The delta against `point_lookup` (ad-hoc) isolates the lex +
// template + `Vec` + hash cost of auto-parameterization (plan 03a). After 01C,
// this bench also sheds the per-`Open` `hex` String + `open_database` dbi walk.
#[library_benchmark]
#[bench::k1000(primed_keyed_table_prepared())]
fn point_lookup_prepared(mut db: MonaDB) -> Option<Value> {
    db.prepare_cached("select t[?];")
        .unwrap()
        .query([Value::int(1)])
        .unwrap()
        .next()
        .unwrap()
}

library_benchmark_group!(
    name = plan_cache_group;
    benchmarks = query_with_hit, point_lookup, point_lookup_prepared
);

main!(library_benchmark_groups = plan_cache_group);
