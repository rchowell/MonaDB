//! MonaDB benchmark adapter.

use monadb::Config;
use monadb::MonaDB;
use monadb::Value;
use tempfile::TempDir;

use super::config::Workload;
use super::fixtures::{
    DocSpec, render_monadb_composite_prefix_array, render_monadb_composite_select,
    render_monadb_insert, render_monadb_single_range_batch, render_monadb_single_select,
};
use super::store::DocStore;

/// A temporary MonaDB instance for benchmarking.
pub struct MonaDbBench {
    db: MonaDB,
    _dir: TempDir,
}

impl MonaDbBench {
    /// Opens a fresh file-backed database in a temporary directory.
    ///
    /// Honors `MONADB_BENCH_NOSYNC` (any non-empty value) to open with
    /// [`Config::nosync`] (`MDB_NOSYNC`), the relaxed-durability analogue of
    /// SQLite's `synchronous=NORMAL` used in this harness.
    pub fn open() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("bench.db");
        let config = match std::env::var_os("MONADB_BENCH_NOSYNC") {
            Some(v) if !v.is_empty() => Config::default().nosync(),
            _ => Config::default(),
        };
        let db = MonaDB::open_with_config(&path, config).expect("open monadb");
        Self { db, _dir: dir }
    }
}

impl DocStore for MonaDbBench {
    fn create_table(&mut self, workload: Workload) {
        let sql = if workload.is_composite() {
            "create table docs (tenant string, seq int);"
        } else {
            "create table docs (id int);"
        };
        self.db.execute(sql).expect("create table");
    }

    fn insert(&mut self, spec: &DocSpec) {
        let sql = render_monadb_insert(spec);
        self.db.execute(&sql).expect("insert");
    }

    fn select_single(&mut self, id: i64) -> usize {
        let sql = render_monadb_single_select(id);
        drain_one(&mut self.db, &sql)
    }

    fn select_composite(&mut self, tenant: &str, seq: i64) -> usize {
        let sql = render_monadb_composite_select(tenant, seq);
        drain_one(&mut self.db, &sql)
    }

    fn select_single_range(&mut self, lo: i64, hi: i64) -> usize {
        let sql = render_monadb_single_range_batch(lo, hi);
        drain_array(&mut self.db, &sql)
    }

    fn select_composite_prefix(&mut self, tenant: &str) -> usize {
        let sql = render_monadb_composite_prefix_array(tenant);
        drain_array(&mut self.db, &sql)
    }
}

/// Runs a query expected to yield a single scalar row, fully decoding it.
fn drain_one(db: &mut MonaDB, sql: &str) -> usize {
    let mut rows = db.query(sql, false).expect("select");
    let consumed = match rows.next().expect("next") {
        Some(value) => {
            let _ = value.encode().expect("encode");
            1
        }
        None => 0,
    };
    rows.finish().expect("finish");
    consumed
}

/// Runs a query whose single row is an array of documents, decoding each item.
fn drain_array(db: &mut MonaDB, sql: &str) -> usize {
    let mut rows = db.query(sql, false).expect("select");
    let mut count = 0usize;
    if let Some(Value::Array(items)) = rows.next().expect("next") {
        for item in items.iter() {
            let _ = item.encode().expect("encode");
            count += 1;
        }
    }
    rows.finish().expect("finish");
    count
}
