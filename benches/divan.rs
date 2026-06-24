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

use monadb::{MonaDB, Params, Value};

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

// One warm DB per workload, created once and reused across all iterations so
// the LMDB page cache stays hot and we measure plan dispatch, not I/O.
thread_local! {
    static SELECT_DB: RefCell<MonaDB> = RefCell::new(primed_select());
    static LOOKUP_DB: RefCell<MonaDB> = RefCell::new(primed_point_lookup());
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
