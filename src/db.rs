//! Database handle: open, close, collection handles, transaction begin.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use redb::backends::InMemoryBackend;
use redb::{Database, ReadTransaction, ReadableDatabase, TableHandle, WriteTransaction};

use crate::collection::Collection;
use crate::error::{self, TransactionError};
use crate::txn::{Gate, GateError, Txn};

/// Shared state behind `Db`, `Txn`, and every `Collection` handle.
pub struct DbInner {
    /// `None` after close; every operation on a closed database then raises.
    pub db: Mutex<Option<Database>>,
    /// The write gate — timeout and re-entry guard — in front of `begin_write`.
    pub gate: Gate,
    /// The one open explicit write transaction, if any. Guarded by `gate`.
    pub active: Mutex<Option<WriteTransaction>>,
    pub timeout: Duration,
    pub durable: bool,
}

impl DbInner {
    /// Runs `f` with the open database, or raises if it has been closed.
    pub fn with_db<R>(&self, f: impl FnOnce(&Database) -> PyResult<R>) -> PyResult<R> {
        let guard = self.db.lock().expect("db poisoned");
        match guard.as_ref() {
            Some(db) => f(db),
            None => Err(TransactionError::new_err("database is closed")),
        }
    }

    /// Begins a read snapshot. Never blocks — redb readers are MVCC.
    pub fn begin_read(&self) -> PyResult<ReadTransaction> {
        self.with_db(|db| db.begin_read().map_err(error::storage))
    }

    /// Acquires the write gate, mapping refusal onto the two exception types.
    ///
    /// Must be called inside `Python::detach`: waiting while holding the GIL
    /// would stall the very threads that could release the gate.
    pub fn acquire_gate(&self) -> Result<(), PyErr> {
        match self.gate.acquire(self.timeout) {
            Ok(()) => Ok(()),
            Err(GateError::Busy) => Err(error::busy(format!(
                "write gate timed out after {:?}",
                self.timeout
            ))),
            Err(GateError::Reentrant) => Err(error::txn(
                "a transaction is already open on this thread",
            )),
        }
    }

    /// Begins a write transaction with the configured durability.
    pub fn begin_write(&self) -> PyResult<WriteTransaction> {
        self.with_db(|db| {
            let mut txn = db.begin_write().map_err(error::storage)?;
            if !self.durable {
                txn.set_durability(redb::Durability::None)
                    .map_err(error::storage)?;
            }
            Ok(txn)
        })
    }

    /// Sorted collection names, from the active write transaction if one is
    /// open, otherwise from a fresh snapshot.
    pub fn names(&self) -> PyResult<Vec<String>> {
        let from_active = {
            let active = self.active.lock().expect("txn poisoned");
            match active.as_ref() {
                Some(txn) => Some(
                    txn.list_tables()
                        .map_err(error::storage)?
                        .map(|h| h.name().to_string())
                        .collect::<Vec<_>>(),
                ),
                None => None,
            }
        };
        let mut names = match from_active {
            Some(names) => names,
            None => self
                .begin_read()?
                .list_tables()
                .map_err(error::storage)?
                .map(|h| h.name().to_string())
                .collect(),
        };
        names.sort();
        Ok(names)
    }
}

/// Validates a collection name — redb panics on an empty table name.
pub fn check_name(name: &str) -> PyResult<()> {
    if name.is_empty() {
        return Err(PyValueError::new_err("collection name must be non-empty"));
    }
    Ok(())
}

/// The database handle exposed to Python.
#[pyclass]
pub struct Db {
    pub inner: Arc<DbInner>,
}

/// Opens a database: in-memory when `path` is `None`, otherwise file-backed.
#[pyfunction]
#[pyo3(signature = (path=None, timeout=5.0, durable=true))]
pub fn open(path: Option<PathBuf>, timeout: f64, durable: bool) -> PyResult<Db> {
    if !timeout.is_finite() || timeout < 0.0 {
        return Err(PyValueError::new_err(
            "timeout must be a non-negative number",
        ));
    }
    let db = match path {
        Some(p) => Database::create(p).map_err(error::storage)?,
        None => Database::builder()
            .create_with_backend(InMemoryBackend::new())
            .map_err(error::storage)?,
    };
    Ok(Db {
        inner: Arc::new(DbInner {
            db: Mutex::new(Some(db)),
            gate: Gate::new(),
            active: Mutex::new(None),
            timeout: Duration::from_secs_f64(timeout),
            durable,
        }),
    })
}

#[pymethods]
impl Db {
    fn names(&self) -> PyResult<Vec<String>> {
        self.inner.names()
    }

    fn has(&self, name: &str) -> PyResult<bool> {
        Ok(self.inner.names()?.iter().any(|n| n == name))
    }

    /// Drops a collection in its own write transaction; `KeyError` if absent.
    #[pyo3(name = "drop")]
    fn drop_(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        check_name(name)?;
        py.detach(|| self.inner.acquire_gate())?;
        let outcome = (|| {
            let txn = self.inner.begin_write()?;
            let def = redb::TableDefinition::<&[u8], &[u8]>::new(name);
            let existed = txn.delete_table(def).map_err(error::storage)?;
            txn.commit().map_err(error::storage)?;
            Ok::<bool, PyErr>(existed)
        })();
        self.inner.gate.release();
        if outcome? {
            Ok(())
        } else {
            Err(PyKeyError::new_err(name.to_string()))
        }
    }

    fn collection(&self, name: String) -> PyResult<Collection> {
        check_name(&name)?;
        Ok(Collection::new(Arc::clone(&self.inner), name, false))
    }

    /// Begins an explicit write transaction: gate, `begin_write`, park it in
    /// `active` so collection handles can find it.
    fn begin(&self, py: Python<'_>) -> PyResult<Txn> {
        py.detach(|| self.inner.acquire_gate())?;
        match self.inner.begin_write() {
            Ok(txn) => {
                *self.inner.active.lock().expect("txn poisoned") = Some(txn);
                Ok(Txn {
                    inner: Arc::clone(&self.inner),
                })
            }
            Err(e) => {
                self.inner.gate.release();
                Err(e)
            }
        }
    }

    /// Closes the database, aborting any open transaction.
    fn close(&self) {
        // Dropping a `WriteTransaction` without committing aborts it.
        if self
            .inner
            .active
            .lock()
            .expect("txn poisoned")
            .take()
            .is_some()
        {
            self.inner.gate.release();
        }
        self.inner.db.lock().expect("db poisoned").take();
    }
}
