pub(crate) mod keycodec;
pub(crate) mod value_codec;

pub use crate::catalog::{ColumnSchema, ColumnType, TableSchema};
pub use crate::cursor::Cursor;
pub use crate::transaction::{ReadTxn, WriteTxn};

use std::path::Path;
use std::sync::Arc;

use heed::types::Bytes;
use heed::{Database, Env as HeedEnv, EnvFlags, EnvOpenOptions};

use crate::Result;

/// Reserved capacity for future named DBs (branches, commits, etc.).
const OPT_MAX_DBS: u32 = 8;

/// Default virtual address-space reservation for the file (1 GiB).
const OPT_MMAP_SIZE: usize = 1024 * 1024 * 1024;

use crate::value::Value;

/// A row pulled out of a cursor.
///
/// `oid` is the surrogate row id for tables with no declared PK; once Phase 2 adds typed
/// PKs, it will continue to be the primary identifier exposed to the VM (typed columns
/// remain inside the JSON body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub oid: u64,
    pub val: Value,
}

/// The storage engine, cheap to clone.
#[derive(Clone)]
pub struct Storage {
    /// Shared storage inner, shared across transactions.
    inner: Arc<StorageInner>,
}

impl Storage {
    /// Open or create a database at `path`. The path refers to a single file
    /// (we set `EnvFlags::NO_SUB_DIR`); a sibling lock file is created automatically
    /// by LMDB.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let inner = StorageInner::open(path)?;
        let inner = Arc::new(inner);
        Ok(Self { inner })
    }

    /// Returns a read transaction.
    pub fn begin_read(&self) -> Result<ReadTxn<'_>> {
        ReadTxn::begin(&self.inner)
    }

    /// Returns a write transaction handle.
    pub fn begin_write(&self) -> Result<WriteTxn<'_>> {
        WriteTxn::begin(&self.inner)
    }
}

/// Storage inner wraps LMDB with handles to our 'meta'
/// and 'data' databases (B+ trees). This gets wrapped
/// in an Arc so that it can be borrowed for the lifetime
/// of a transaction and so it's cheap to clone.
pub(crate) struct StorageInner {
    /// Shared heed environment, like a connection to LMDB.
    pub heed: HeedEnv,
    /// Typed handle for the meta database.
    pub meta: Database<Bytes, Bytes>,
    /// Typed handle for the data database.
    pub data: Database<Bytes, Bytes>,
}

impl StorageInner {
    /// Open or create a database at `path`. The path refers to a single file.
    pub(crate) fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Heed 0.20 makes env open unsafe, mmap-backed file with no syscall recovery.
        let heed = unsafe {
            EnvOpenOptions::new()
                .map_size(OPT_MMAP_SIZE)
                .max_dbs(OPT_MAX_DBS)
                .flags(EnvFlags::NO_SUB_DIR)
                .open(path.as_ref())?
        };
        // Create both named DBs, this is idempotent across restarts.
        let mut wtxn = heed.write_txn()?;
        let meta = heed.create_database::<Bytes, Bytes>(&mut wtxn, Some("meta"))?;
        let data = heed.create_database::<Bytes, Bytes>(&mut wtxn, Some("data"))?;
        wtxn.commit()?;
        // Return 
        Ok(Self { heed, meta, data })
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Storage) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.mdb");
        let engine = Storage::open(&path).unwrap();
        (dir, engine)
    }

    fn create_table(engine: &Storage, name: &str, columns: Vec<ColumnSchema>) -> TableSchema {
        let mut wtxn = engine.begin_write().unwrap();
        let s = wtxn.create_table(name, columns).unwrap();
        wtxn.commit().unwrap();
        s
    }

    fn put_row(engine: &Storage, table: &str, value: Value) {
        let mut wtxn = engine.begin_write().unwrap();
        wtxn.put_row(table, value).unwrap();
        wtxn.commit().unwrap();
    }

    fn collect(engine: &Storage, table: &str) -> Vec<Row> {
        let rtxn = engine.begin_read().unwrap();
        let mut cur = rtxn.open_cursor(table).unwrap();
        let mut out = Vec::new();
        let mut alive = cur.rewind().unwrap();
        while alive {
            out.push(cur.curr().clone());
            alive = cur.next().unwrap();
        }
        out
    }

    #[test]
    fn create_then_get_table() {
        let (_dir, engine) = fresh();
        let columns = vec![
            ColumnSchema {
                name: "x".to_string(),
                typ: ColumnType::Int,
            },
            ColumnSchema {
                name: "y".to_string(),
                typ: ColumnType::Int,
            },
        ];
        let created = create_table(&engine, "points", columns.clone());
        assert_eq!(created.name, "points");
        assert_eq!(created.table_id, 1);
        assert_eq!(created.columns, columns);

        let rtxn = engine.begin_read().unwrap();
        let read = rtxn.get_table("points").unwrap();
        assert_eq!(created, read);
    }

    #[test]
    fn create_table_assigns_distinct_ids() {
        let (_dir, engine) = fresh();
        let a = create_table(&engine, "a", vec![]);
        let b = create_table(&engine, "b", vec![]);
        assert_ne!(a.table_id, b.table_id);
    }

    #[test]
    fn put_row_and_scan_in_insertion_order() {
        let (_dir, engine) = fresh();
        create_table(&engine, "points", vec![]);
        for i in 1..=3 {
            put_row(&engine, "points", Value::number(i as f64));
        }
        let rows = collect(&engine, "points");
        assert_eq!(rows.len(), 3);
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.oid, (i + 1) as u64);
            assert_eq!(row.val, Value::number((i + 1) as f64));
        }
    }

    #[test]
    fn cursor_isolates_tables_by_prefix() {
        let (_dir, engine) = fresh();
        create_table(&engine, "alpha", vec![]);
        create_table(&engine, "beta", vec![]);
        put_row(&engine, "alpha", Value::string("a1".to_string()));
        put_row(&engine, "alpha", Value::string("a2".to_string()));
        put_row(&engine, "beta", Value::string("b1".to_string()));

        let alpha_rows = collect(&engine, "alpha");
        assert_eq!(alpha_rows.len(), 2);
        assert_eq!(alpha_rows[0].val, Value::string("a1".to_string()));
        assert_eq!(alpha_rows[1].val, Value::string("a2".to_string()));

        let beta_rows = collect(&engine, "beta");
        assert_eq!(beta_rows.len(), 1);
        assert_eq!(beta_rows[0].val, Value::string("b1".to_string()));
    }

    #[test]
    fn empty_table_scans_clean() {
        let (_dir, engine) = fresh();
        create_table(&engine, "empty", vec![]);
        assert!(collect(&engine, "empty").is_empty());
    }

    #[test]
    fn unknown_table_errors() {
        let (_dir, engine) = fresh();
        let rtxn = engine.begin_read().unwrap();
        let result = rtxn.open_cursor("nope");
        let is_match = matches!(&result, Err(crate::error::Error::UnknownTable(s)) if s == "nope");
        assert!(is_match, "expected UnknownTable(\"nope\"), got {:?}", result.err());
    }

    #[test]
    fn duplicate_create_errors() {
        let (_dir, engine) = fresh();
        create_table(&engine, "x", vec![]);
        let mut wtxn = engine.begin_write().unwrap();
        let err = wtxn.create_table("x", vec![]).unwrap_err();
        assert!(matches!(err, crate::error::Error::InternalError(_)));
    }

    #[test]
    fn aborted_write_does_not_persist() {
        let (_dir, engine) = fresh();
        create_table(&engine, "t", vec![]);
        {
            let mut wtxn = engine.begin_write().unwrap();
            wtxn.put_row("t", Value::number(99.0)).unwrap();
            wtxn.abort();
        }
        assert!(collect(&engine, "t").is_empty());
    }

    #[test]
    fn durability_across_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("durable.mdb");
        {
            let engine = Storage::open(&path).unwrap();
            create_table(&engine, "t", vec![]);
            put_row(&engine, "t", Value::number(1.0));
            put_row(&engine, "t", Value::number(2.0));
        }
        let engine = Storage::open(&path).unwrap();
        let rows = collect(&engine, "t");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].val, Value::number(1.0));
        assert_eq!(rows[1].val, Value::number(2.0));
    }

    #[test]
    fn rewind_after_exhaustion_reads_again() {
        let (_dir, engine) = fresh();
        create_table(&engine, "t", vec![]);
        put_row(&engine, "t", Value::number(1.0));
        put_row(&engine, "t", Value::number(2.0));

        let rtxn = engine.begin_read().unwrap();
        let mut cur = rtxn.open_cursor("t").unwrap();
        let pass1: Vec<u64> = std::iter::from_fn(|| {
            cur.next().ok().filter(|&b| b).map(|_| cur.curr().oid)
        })
        .collect();
        assert!(pass1.is_empty(), "next() before rewind() must be a no-op");
        let mut alive = cur.rewind().unwrap();
        let mut pass2 = Vec::new();
        while alive {
            pass2.push(cur.curr().oid);
            alive = cur.next().unwrap();
        }
        assert_eq!(pass2, vec![1, 2]);

        let mut alive = cur.rewind().unwrap();
        let mut pass3 = Vec::new();
        while alive {
            pass3.push(cur.curr().oid);
            alive = cur.next().unwrap();
        }
        assert_eq!(pass3, vec![1, 2]);
    }
}
