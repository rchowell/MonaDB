pub use crate::catalog::{ColumnSchema, ColumnType, TableSchema};
pub use crate::cursor::Cursor;
pub use crate::transaction::{ReadTxn, WriteTxn};

use std::path::Path;
use std::sync::Arc;

use heed::types::Bytes;
use heed::{Database, Env as HeedEnv, EnvFlags, EnvOpenOptions};

use crate::Result;

/// Reserved capacity for future named DBs (branches, commits, etc.).
const HEED_MAX_DBS: u32 = 8;

/// Default virtual address-space reservation for the file (1 GiB).
const HEED_MMAP_SIZE: usize = 1024 * 1024 * 1024;

/// The meta database name.
const HEED_DB_META: &str = "meta";

/// The data database name.
const HEED_DB_DATA: &str = "data";

/// The storage engine, cheap to clone.
#[derive(Clone)]
pub struct Storage {
    /// Shared storage environment, shared across transactions.
    env: Arc<Env>,
}

impl Storage {
    /// Open or create a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let env = Env::open(path)?;
        let env = Arc::new(env);
        Ok(Self { env })
    }

    /// Returns a read transaction.
    pub fn read(&self) -> Result<ReadTxn<'_>> {
        ReadTxn::open(&self.env)
    }

    /// Returns a write transaction handle.
    pub fn write(&self) -> Result<WriteTxn<'_>> {
        WriteTxn::open(&self.env)
    }
}

/// Storage inner wraps LMDB with handles to our 'meta'
/// and 'data' databases (B+ trees). This gets wrapped
/// in an Arc so that it can be borrowed for the lifetime
/// of a transaction and so it's cheap to clone.
pub struct Env {
    /// Shared heed environment, like a connection to LMDB.
    pub heed: HeedEnv,
    /// Typed handle for the meta database.
    pub meta: Database<Bytes, Bytes>,
    /// Typed handle for the data database.
    pub data: Database<Bytes, Bytes>,
}

impl Env {
    /// Open or create a database at `path`. The path refers to a single file.
    pub(crate) fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Heed 0.20 makes env open unsafe, mmap-backed file with no syscall recovery.
        let heed = unsafe {
            EnvOpenOptions::new()
                .map_size(HEED_MMAP_SIZE)
                .max_dbs(HEED_MAX_DBS)
                .flags(EnvFlags::NO_SUB_DIR)
                .open(path.as_ref())?
        };
        // Create both named DBs; this is idempotent across restarts.
        let mut wtxn = heed.write_txn()?;
        let meta = heed.create_database::<Bytes, Bytes>(&mut wtxn, Some(HEED_DB_META))?;
        let data = heed.create_database::<Bytes, Bytes>(&mut wtxn, Some(HEED_DB_DATA))?;
        wtxn.commit()?;
        Ok(Self { heed, meta, data })
    }
}
