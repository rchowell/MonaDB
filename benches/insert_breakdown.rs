//! Insert latency breakdown: autocommit vs batched commit vs prepared params.
//!
//! Quantifies how much of insert cost is per-commit fsync vs SQL pipeline overhead.
//!
//! ```sh
//! cargo bench --bench insert_breakdown
//! MONADB_BENCH_M=100 MONADB_BENCH_PROFILE=md cargo bench --bench insert_breakdown
//! ```

mod config;
mod fixtures;

use std::hint::black_box;
use std::time::Instant;

use monadb::{Config, MonaDB, Value};
use tempfile::TempDir;

use config::Profile;
use fixtures::{DocSpec, encoded_json_bytes, render_monadb_insert};

/// Timed operations per mode (override with `MONADB_BENCH_M`).
fn ops_count() -> usize {
    std::env::var("MONADB_BENCH_M")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
}

/// Document profile (override with `MONADB_BENCH_PROFILE`, default `xs`).
fn profile() -> Profile {
    std::env::var("MONADB_BENCH_PROFILE")
        .ok()
        .and_then(|s| Profile::parse(&s))
        .unwrap_or(Profile::Xs)
}

fn open_db(nosync: bool) -> (TempDir, MonaDB) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("bench.db");
    let config = if nosync {
        Config::default().nosync()
    } else {
        Config::default()
    };
    let db = MonaDB::open_with_config(&path, &config).expect("open");
    (dir, db)
}

fn setup(db: &mut MonaDB) {
    db.execute("create table docs (id int);")
        .expect("create");
}

fn specs(profile: Profile, base: i64, n: usize) -> Vec<DocSpec> {
    (0..n)
        .map(|i| DocSpec::single(profile, base + i as i64))
        .collect()
}

fn doc_value(spec: &DocSpec) -> Value {
    Value::from_json(serde_json::from_slice(&encoded_json_bytes(spec)).expect("fixture json"))
}

fn ns_per_op(elapsed: std::time::Duration, n: usize) -> f64 {
    elapsed.as_nanos() as f64 / n.max(1) as f64
}

fn bench_autocommit(db: &mut MonaDB, specs: &[DocSpec]) {
    for spec in specs {
        let sql = render_monadb_insert(spec);
        db.execute(&sql).expect("insert");
    }
}

fn bench_explicit_txn(db: &mut MonaDB, specs: &[DocSpec]) {
    db.begin_transaction().expect("begin");
    for spec in specs {
        let sql = render_monadb_insert(spec);
        db.execute(&sql).expect("insert");
    }
    db.commit_transaction().expect("commit");
}

fn bench_multi_value(db: &mut MonaDB, specs: &[DocSpec]) {
    let mut parts: Vec<String> = Vec::with_capacity(specs.len());
    for spec in specs {
        let body = render_monadb_insert(spec);
        let inner = body
            .strip_prefix("insert into docs (")
            .and_then(|s| s.strip_suffix(");"))
            .expect("insert shape");
        parts.push(inner.to_owned());
    }
    let sql = format!("insert into docs ({});", parts.join(", "));
    db.execute(&sql).expect("batch insert");
}

fn bench_prepared_param(db: &mut MonaDB, specs: &[DocSpec]) {
    let mut stmt = db.prepare("insert into docs ($1);").expect("prepare");
    for spec in specs {
        stmt.execute([doc_value(spec)]).expect("insert");
    }
}

fn main() {
    fixtures::assert_profile_sizes();
    let n = ops_count();
    let profile = profile();
    let base = 10_000_i64;
    let specs = specs(profile, base, n);

    println!(
        "insert_breakdown: profile={} ops={}",
        profile.label(),
        n
    );
    println!(
        "{:<24} {:>12} ns/op",
        "mode", "latency"
    );

    let modes: &[(&str, bool, fn(&mut MonaDB, &[DocSpec]))] = &[
        ("autocommit", false, bench_autocommit),
        ("explicit_txn", false, bench_explicit_txn),
        ("multi_value", false, bench_multi_value),
        ("prepared_param", false, bench_prepared_param),
        // Same driver as autocommit; the `nosync` flag is what differs.
        ("relaxed_autocommit", true, bench_autocommit),
    ];

    for (label, nosync, run) in modes {
        let (_dir, mut db) = open_db(*nosync);
        setup(&mut db);
        let start = Instant::now();
        run(&mut db, &specs);
        let elapsed = start.elapsed();
        black_box(&db);
        println!("{:<24} {:>12.0}", label, ns_per_op(elapsed, n));
    }
}
