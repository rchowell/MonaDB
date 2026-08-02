use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pyo3::prelude::*;
use redb::backends::InMemoryBackend;
use redb::{ReadTransaction, ReadableDatabase, TableHandle, WriteTransaction};

use crate::error;

/// Shared state behind `Database` and every `Collection` handle.
pub struct Store {
    /// `None` after close; every operation on a closed database then raises.
    database: Mutex<Option<Arc<redb::Database>>>,
    /// Durable mode fsync's on each commit.
    durable: bool,
}

impl Store {

    pub fn file(path: PathBuf, durable: bool) -> PyResult<Self> {
        let database = redb::Database::create(path).map_err(error::storage)?;
        let database = Arc::new(database);
        Ok(Self { 
            database: Mutex::new(Some(database)),
            durable,
        })
    }

    pub fn memory(durable: bool) -> PyResult<Self> {
        let database = redb::Database::builder()
                .create_with_backend(InMemoryBackend::new())
                .map_err(error::storage)?;
        let database = Arc::new(database);
        Ok(Self { 
            database: Mutex::new(Some(database)),
            durable,
        })
    }

    /// The open database, or an error once it has been closed.
    ///
    /// The handle is cloned out from under the lock rather than used in place.
    /// `begin_write` waits for any other writer, and holding this mutex across
    /// that wait would block readers — which must never block.
    fn database(&self) -> PyResult<Arc<redb::Database>> {
        self.database
            .lock()
            .expect("db poisoned")
            .clone()
            .ok_or_else(error::closed)
    }

    /// Begins a read snapshot. Never blocks — redb readers are MVCC.
    pub fn begin_read(&self) -> PyResult<ReadTransaction> {
        self.database()?.begin_read().map_err(error::storage)
    }

    /// Begins a write transaction, waiting for any other writer.
    ///
    /// redb serializes writers internally and waits without a deadline, so this
    /// can block for as long as another writer takes. It waits without the GIL:
    /// holding it would stall every other Python thread, readers included, for
    /// that whole time.
    pub fn begin_write(&self, py: Python<'_>) -> PyResult<WriteTransaction> {
        let db = self.database()?;
        let durable = self.durable;
        py.detach(move || {
            let mut txn = db.begin_write().map_err(error::storage)?;
            if !durable {
                txn.set_durability(redb::Durability::None)
                    .map_err(error::storage)?;
            }
            Ok(txn)
        })
    }

    /// Sorted collection names, from a fresh snapshot.
    pub fn names(&self) -> PyResult<Vec<String>> {
        let mut names: Vec<String> = self
            .begin_read()?
            .list_tables()
            .map_err(error::storage)?
            .map(|h| h.name().to_string())
            .collect();
        names.sort();
        Ok(names)
    }

    /// Closes the underlying database.
    ///
    /// A write in flight on another thread holds its own handle, so the file
    /// stays open until that call finishes rather than being closed under it.
    pub fn close(&self) {
        self.database.lock().expect("db poisoned").take();
    }
}
