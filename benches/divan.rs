//! divan wall-clock benchmarks: PlanCache cache-hit path.
//!
//! Uses a thread-local warm database so each iteration measures a true cache hit
//! on a page-cache-warm LMDB: the overhead is plan dispatch only, not I/O.
//!
//! Complements the iai-callgrind bench (deterministic instruction counts) with
//! real wall-clock timing on the local machine.
//!
//! Run:
//!   cargo bench --bench divan

use std::cell::RefCell;

use monadb::{MonaDB, Params, PreparedStatement, Value};

fn primed_select() -> MonaDB {
    let mut db = MonaDB::memory().unwrap();
    db.query_with("select $1;", &Params::positional(vec![Value::int(0)]), false)
        .unwrap()
        .finish()
        .unwrap();
    db
}

fn primed_point_lookup() -> MonaDB {
    let mut db = MonaDB::memory().unwrap();
    db.execute("create table t (id int);").unwrap();
    db.execute(r#"insert into t ({"id": 1});"#).unwrap();
    db.query("select t[1];", false).unwrap().next().unwrap();
    db
}

/// A warm DB plus a prepared `select t[?];`, for the `normalize`-free lookup.
fn primed_point_lookup_prepared() -> (MonaDB, PreparedStatement) {
    let mut db = primed_point_lookup();
    let stmt = db.prepare("select t[?];").unwrap();
    db.execute_prepared(&stmt, &Params::positional(vec![Value::int(1)]), false)
        .unwrap()
        .next()
        .unwrap();
    (db, stmt)
}

// One warm DB per workload, created once and reused across all iterations so
// the LMDB page cache stays hot and we measure plan dispatch, not I/O.
thread_local! {
    static SELECT_DB: RefCell<MonaDB> = RefCell::new(primed_select());
    static LOOKUP_DB: RefCell<MonaDB> = RefCell::new(primed_point_lookup());
    static LOOKUP_PREPARED: RefCell<(MonaDB, PreparedStatement)> =
        RefCell::new(primed_point_lookup_prepared());
}

fn main() {
    divan::main();
}

// Cache-hit dispatch overhead: PlanCache::get + execute_prepared on a trivial
// query. Before: deep-clone PreparedStatement + O(256) VecDeque scan + String
// alloc. After: Rc refcount bump + u64 write.
#[divan::bench]
fn query_with_hit() -> u64 {
    SELECT_DB.with(|db| {
        db.borrow_mut()
            .query_with("select $1;", &Params::positional(vec![Value::int(1)]), false)
            .unwrap()
            .finish()
            .unwrap()
    })
}

// End-to-end point lookup: normalize → cache hit → keyed btree get.
#[divan::bench]
fn point_lookup() -> Option<Value> {
    LOOKUP_DB.with(|db| {
        db.borrow_mut()
            .query("select t[1];", false)
            .unwrap()
            .next()
            .unwrap()
    })
}

// Prepared point lookup: execute_prepared → keyed btree get, NO `normalize`.
// The gap to `point_lookup` is the auto-parameterization cost (03a); after 01C
// this path also drops the per-`Open` handle resolution.
#[divan::bench]
fn point_lookup_prepared() -> Option<Value> {
    LOOKUP_PREPARED.with(|cell| {
        let mut guard = cell.borrow_mut();
        let (db, stmt) = &mut *guard;
        db.execute_prepared(stmt, &Params::positional(vec![Value::int(1)]), false)
            .unwrap()
            .next()
            .unwrap()
    })
}
