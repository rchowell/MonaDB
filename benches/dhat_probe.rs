//! dhat heap-allocation probe for the PlanCache cache-hit path.
//!
//! Profiles N repeated cache hits for two workloads, writing `dhat-heap.json`
//! to the project root. Open the file in the DHAT viewer to see total bytes
//! allocated and live bytes per call site:
//!   https://nnethercote.github.io/dh_view/dh_view.html
//!
//! Run:
//!   cargo bench --bench dhat_probe

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use monadb::{MonaDB, Params, Value};

const N: usize = 1_000;

fn setup_select() -> MonaDB {
    let mut db = MonaDB::memory().unwrap();
    db.query_with("select $1;", &Params::positional(vec![Value::int(0)]))
        .unwrap()
        .finish()
        .unwrap();
    db
}

fn setup_keyed_table() -> MonaDB {
    let mut db = MonaDB::memory().unwrap();
    db.execute("create table t (id int);").unwrap();
    for i in 1..=100_i64 {
        db.execute(&format!(r#"insert into t ({{"id": {i}}});"#))
            .unwrap();
    }
    db.query("select t[50];").unwrap().finish().unwrap();
    db
}

fn main() {
    // Both DBs are created and primed BEFORE the profiler starts, so setup
    // allocations don't pollute the per-call-site data.
    let mut db_select = setup_select();
    let mut db_lookup = setup_keyed_table();

    let _profiler = dhat::Profiler::new_heap();

    // ── query_with cache hit ──────────────────────────────────────────────────
    for _ in 0..N {
        db_select
            .query_with("select $1;", &Params::positional(vec![Value::int(1)]))
            .unwrap()
            .finish()
            .unwrap();
    }

    // ── point lookup: normalize → cache hit → btree get ──────────────────────
    for _ in 0..N {
        db_lookup
            .query("select t[1];")
            .unwrap()
            .next()
            .unwrap();
    }

    eprintln!("{N} iterations × 2 workloads → dhat-heap.json");
}
