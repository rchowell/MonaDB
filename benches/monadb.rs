//! MonaDB benchmark adapter.
//!
//! Every operation runs through a prepared statement: each call fetches the
//! cached plan via [`MonaDB::prepare_cached`] (a raw-SQL lookup returning an
//! `Rc`-shared program — no lex, no `normalize`) and binds the key/document as
//! parameters. This is the parse-free steady-state hot path SQLite is compared
//! against. The plan cannot be stored on the adapter because [`Statement`]
//! borrows the database mutably, so `prepare_cached` is called per operation.

use monadb::Config;
use monadb::MonaDB;
use monadb::Params;
use monadb::Rows;
use monadb::Value;
use tempfile::TempDir;

use super::config::Workload;
use super::fixtures::{DocSpec, build_json};
use super::store::DocStore;

/// Full-key point lookup by single integer key.
const SELECT_SINGLE: &str = "select docs[?];";
/// Full-key point lookup by composite `(tenant, seq)` key.
const SELECT_COMPOSITE: &str = "select docs[?, ?];";
/// Partial-key prefix read — one tenant on a composite table (array result).
const SELECT_PREFIX: &str = "select docs[?];";
/// Inserts one document bound as a single object parameter.
const INSERT_DOC: &str = "insert into docs ($1);";

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

    /// Prepares `sql` (cache hit after the first call) and runs it with `params`.
    fn run(&mut self, sql: &str, params: Params) -> Rows {
        self.db
            .prepare_cached(sql)
            .expect("prepare")
            .query(params)
            .expect("query")
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
        let doc = Value::from_json(build_json(spec));
        self.db
            .prepare_cached(INSERT_DOC)
            .expect("prepare insert")
            .execute(Params::positional(vec![doc]))
            .expect("insert");
    }

    fn select_single(&mut self, id: i64) -> usize {
        let rows = self.run(SELECT_SINGLE, Params::positional(vec![Value::int(id)]));
        drain_one(rows)
    }

    fn select_composite(&mut self, tenant: &str, seq: i64) -> usize {
        let params = Params::positional(vec![Value::string(tenant.to_owned()), Value::int(seq)]);
        let rows = self.run(SELECT_COMPOSITE, params);
        drain_one(rows)
    }

    fn select_single_range(&mut self, lo: i64, hi: i64) -> usize {
        // No native range subscript exists, so a contiguous span is a batch of
        // point gets: `select [docs[?], docs[?], …];`. The template width is the
        // span length (constant across a run), so `prepare_cached` compiles it
        // once and keys later calls by the same string.
        let width = (hi - lo).max(0) as usize;
        let sql = single_range_sql(width);
        let params = Params::positional((lo..hi).map(Value::int).collect());
        let rows = self.run(&sql, params);
        drain_array(rows)
    }

    fn select_composite_prefix(&mut self, tenant: &str) -> usize {
        let rows = self.run(SELECT_PREFIX, Params::positional(vec![Value::string(tenant.to_owned())]));
        drain_array(rows)
    }
}

/// Builds `select [docs[?], docs[?], … ×width];` for a `width`-row batch get.
fn single_range_sql(width: usize) -> String {
    let mut sql = String::from("select [");
    for i in 0..width {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str("docs[?]");
    }
    sql.push_str("];");
    sql
}

/// Consumes a single-row result, fully decoding it; returns rows consumed (0/1).
fn drain_one(mut rows: Rows) -> usize {
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

/// Consumes a result whose single row is an array, decoding each item.
fn drain_array(mut rows: Rows) -> usize {
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
