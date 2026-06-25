//! Allocation-count microbench for the warm point-lookup hot path.
//!
//! Attributes per-op heap traffic to the two ways MonaDB serves a keyed point
//! lookup, isolating the fixed (N-independent) overhead the REPORT pins the
//! `xs`/`sm` SQLite gap on:
//!
//!   - `adhoc`    — `db.query("select t[1];")`: full `normalize` (lex + template
//!                  `String` + `Vec` + hash) → plan-cache hit → keyed btree get.
//!   - `prepared` — `db.execute_prepared(stmt, [1])`: NO `normalize`; one cached
//!                  program bound to the key param.
//!
//! The `adhoc − prepared` delta is the auto-parameterization cost (plan 03a).
//! After plan 01C bakes the table handle into the program, the `prepared` row
//! also sheds the per-`Open` `hex` `String` + `open_database` dbi walk — re-run
//! and compare the `prepared` row across the two commits.
//!
//! Run:
//!   cargo bench --bench alloc_hotpath

mod alloc;

use std::hint::black_box;

use monadb::{MonaDB, Value};

/// Install the counting allocator so the bracketed loops attribute heap traffic.
#[global_allocator]
static ALLOC: alloc::Counting = alloc::Counting::new();

/// Rows preloaded into the keyed table; cardinality is irrelevant to the fixed
/// per-op overhead this bench targets, but a non-trivial tree is more realistic.
const N: i64 = 1000;
/// Timed iterations per variant; per-op figures divide the totals by this.
const ITERS: usize = 2000;

fn primed_table() -> MonaDB {
    let mut db = MonaDB::memory().unwrap();
    db.execute("create table t (id int);").unwrap();
    for i in 1..=N {
        db.execute(&format!(r#"insert into t ({{"id": {i}}});"#))
            .unwrap();
    }
    // Prime the ad-hoc plan cache: "select t[K]" normalizes to "select t[?]".
    db.query("select t[500];").unwrap().finish().unwrap();
    db
}

fn measure_adhoc(db: &mut MonaDB) -> alloc::Stats {
    alloc::reset();
    for _ in 0..ITERS {
        let mut rows = db.query("select t[1];").unwrap();
        if let Some(v) = rows.next().unwrap() {
            black_box(v.encode().unwrap());
        }
        rows.finish().unwrap();
    }
    alloc::snapshot()
}

fn measure_prepared(db: &mut MonaDB) -> alloc::Stats {
    alloc::reset();
    for _ in 0..ITERS {
        let mut rows = db
            .prepare_cached("select t[?];")
            .unwrap()
            .query([Value::int(1)])
            .unwrap();
        if let Some(v) = rows.next().unwrap() {
            black_box(v.encode().unwrap());
        }
        rows.finish().unwrap();
    }
    alloc::snapshot()
}

fn report(label: &str, stats: alloc::Stats) {
    let allocs = stats.alloc_count as f64 / ITERS as f64;
    let bytes = stats.total_bytes as f64 / ITERS as f64;
    println!(
        "  {label:<10}  {allocs:>8.1} allocs/op   {bytes:>12.1} bytes/op   \
         (peak +{} B)",
        stats.peak_bytes
    );
}

fn main() {
    let mut db = primed_table();
    // Warm both paths (page cache + any first-run resolution) before measuring.
    db.query("select t[1];").unwrap().finish().unwrap();
    db.prepare_cached("select t[?];")
        .unwrap()
        .query([Value::int(1)])
        .unwrap()
        .finish()
        .unwrap();

    let adhoc = measure_adhoc(&mut db);
    let prepared = measure_prepared(&mut db);

    println!("point-lookup hot path (N={N}, {ITERS} iters/variant):");
    report("adhoc", adhoc);
    report("prepared", prepared);
}
