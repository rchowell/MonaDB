//! SQLite benchmark adapter.

use rusqlite::Connection;
use tempfile::TempDir;

use super::config::{SqliteStorage, Workload};
use super::fixtures::{
    DocSpec, render_sqlite_composite_prefix, render_sqlite_composite_select, render_sqlite_insert,
    render_sqlite_single_range, render_sqlite_single_select,
};
use super::store::DocStore;

/// A temporary SQLite database for benchmarking.
pub struct SqliteBench {
    conn: Connection,
    storage: SqliteStorage,
    _dir: TempDir,
}

impl SqliteBench {
    /// Opens a fresh SQLite database with benchmark pragmas.
    pub fn open(storage: SqliteStorage) -> Self {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("bench.db");
        let conn = Connection::open(&path).expect("open sqlite");
        apply_pragmas(&conn);
        Self {
            conn,
            storage,
            _dir: dir,
        }
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
        let sql = match (workload.is_composite(), self.storage) {
            (false, SqliteStorage::Text) => {
                "CREATE TABLE docs(id INTEGER PRIMARY KEY, doc TEXT NOT NULL);"
            }
            (false, SqliteStorage::Jsonb) => {
                "CREATE TABLE docs(id INTEGER PRIMARY KEY, doc BLOB NOT NULL);"
            }
            (true, SqliteStorage::Text) => {
                "CREATE TABLE docs(tenant TEXT NOT NULL, seq INTEGER NOT NULL, doc TEXT NOT NULL, PRIMARY KEY (tenant, seq));"
            }
            (true, SqliteStorage::Jsonb) => {
                "CREATE TABLE docs(tenant TEXT NOT NULL, seq INTEGER NOT NULL, doc BLOB NOT NULL, PRIMARY KEY (tenant, seq));"
            }
        };
        self.conn.execute_batch(sql).expect("create table");
    }

    fn insert(&mut self, spec: &DocSpec) {
        let sql = render_sqlite_insert(spec, self.storage);
        self.conn.execute_batch(&sql).expect("insert");
    }

    fn select_single(&mut self, id: i64) -> usize {
        let sql = render_sqlite_single_select(id);
        drain_one(&self.conn, &sql)
    }

    fn select_composite(&mut self, tenant: &str, seq: i64) -> usize {
        let sql = render_sqlite_composite_select(tenant, seq);
        drain_one(&self.conn, &sql)
    }

    fn select_single_range(&mut self, lo: i64, hi: i64) -> usize {
        let sql = render_sqlite_single_range(lo, hi);
        drain_all(&self.conn, &sql)
    }

    fn select_composite_prefix(&mut self, tenant: &str) -> usize {
        let sql = render_sqlite_composite_prefix(tenant);
        drain_all(&self.conn, &sql)
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

/// Runs a point query, fully reading the `doc` column; returns rows consumed.
fn drain_one(conn: &Connection, sql: &str) -> usize {
    match conn.query_row(sql, [], read_doc_column) {
        Ok(_) => 1,
        Err(rusqlite::Error::QueryReturnedNoRows) => 0,
        Err(err) => panic!("select failed: {err}"),
    }
}

/// Runs a multi-row query, fully reading each `doc` column; returns rows consumed.
fn drain_all(conn: &Connection, sql: &str) -> usize {
    let mut stmt = conn.prepare(sql).expect("prepare select");
    let mut rows = stmt.query([]).expect("query");
    let mut count = 0usize;
    while let Some(row) = rows.next().expect("next") {
        let _ = read_doc_column(row).expect("doc column");
        count += 1;
    }
    count
}
