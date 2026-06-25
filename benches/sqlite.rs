//! SQLite benchmark adapter.
//!
//! Documents are stored as SQLite's native JSONB binary type (`jsonb(?)` into a
//! `BLOB` column), and every operation runs through a connection-cached prepared
//! statement (`prepare_cached` + bound params) — the parse-free hot path MonaDB
//! is compared against.

use rusqlite::Connection;
use rusqlite::params;
use tempfile::TempDir;

use super::config::Workload;
use super::fixtures::{DocKey, DocSpec, build_json};
use super::store::DocStore;

/// A temporary SQLite database for benchmarking.
pub struct SqliteBench {
    conn: Connection,
    _dir: TempDir,
}

impl SqliteBench {
    /// Opens a fresh SQLite database with benchmark pragmas.
    pub fn open() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("bench.db");
        let conn = Connection::open(&path).expect("open sqlite");
        apply_pragmas(&conn);
        Self { conn, _dir: dir }
    }

    /// Returns the bundled SQLite version string.
    pub fn version() -> String {
        let conn = Connection::open_in_memory().expect("open sqlite");
        conn.query_row("SELECT sqlite_version();", [], |row| row.get(0))
            .expect("sqlite version")
    }
}

impl DocStore for SqliteBench {
    fn create_table(&mut self, workload: Workload) {
        let sql = if workload.is_composite() {
            "CREATE TABLE docs(tenant TEXT NOT NULL, seq INTEGER NOT NULL, doc BLOB NOT NULL, PRIMARY KEY (tenant, seq));"
        } else {
            "CREATE TABLE docs(id INTEGER PRIMARY KEY, doc BLOB NOT NULL);"
        };
        self.conn.execute_batch(sql).expect("create table");
    }

    fn insert(&mut self, spec: &DocSpec) {
        let json = serde_json::to_string(&build_json(spec)).expect("fixture json serializes");
        match &spec.key {
            DocKey::Single(id) => {
                self.conn
                    .prepare_cached("INSERT INTO docs(id, doc) VALUES (?, jsonb(?));")
                    .expect("prepare insert")
                    .execute(params![id, json])
                    .expect("insert");
            }
            DocKey::Composite { tenant, seq } => {
                self.conn
                    .prepare_cached(
                        "INSERT INTO docs(tenant, seq, doc) VALUES (?, ?, jsonb(?));",
                    )
                    .expect("prepare insert")
                    .execute(params![tenant, seq, json])
                    .expect("insert");
            }
        }
    }

    fn select_single(&mut self, id: i64) -> usize {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT doc FROM docs WHERE id = ?;")
            .expect("prepare select");
        drain_one(stmt.query_row(params![id], read_doc_column))
    }

    fn select_composite(&mut self, tenant: &str, seq: i64) -> usize {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT doc FROM docs WHERE tenant = ? AND seq = ?;")
            .expect("prepare select");
        drain_one(stmt.query_row(params![tenant, seq], read_doc_column))
    }

    fn select_single_range(&mut self, lo: i64, hi: i64) -> usize {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT doc FROM docs WHERE id >= ? AND id < ? ORDER BY id;")
            .expect("prepare select");
        let mut rows = stmt.query(params![lo, hi]).expect("query");
        drain_all(&mut rows)
    }

    fn select_composite_prefix(&mut self, tenant: &str) -> usize {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT doc FROM docs WHERE tenant = ? ORDER BY seq;")
            .expect("prepare select");
        let mut rows = stmt.query(params![tenant]).expect("query");
        drain_all(&mut rows)
    }
}

fn apply_pragmas(conn: &Connection) {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -64000;
        ",
    )
    .expect("apply pragmas");
}

fn read_doc_column(row: &rusqlite::Row<'_>) -> rusqlite::Result<Vec<u8>> {
    match row.get::<_, rusqlite::types::Value>(0)? {
        rusqlite::types::Value::Text(text) => Ok(text.into_bytes()),
        rusqlite::types::Value::Blob(bytes) => Ok(bytes),
        other => Err(rusqlite::Error::InvalidColumnType(
            0,
            "doc".into(),
            other.data_type(),
        )),
    }
}

/// Maps a point-query result to rows consumed (1 on a row, 0 when none).
fn drain_one(result: rusqlite::Result<Vec<u8>>) -> usize {
    match result {
        Ok(_) => 1,
        Err(rusqlite::Error::QueryReturnedNoRows) => 0,
        Err(err) => panic!("select failed: {err}"),
    }
}

/// Reads every row's `doc` column from a multi-row result; returns rows consumed.
fn drain_all(rows: &mut rusqlite::Rows<'_>) -> usize {
    let mut count = 0usize;
    while let Some(row) = rows.next().expect("next") {
        let _ = read_doc_column(row).expect("doc column");
        count += 1;
    }
    count
}
