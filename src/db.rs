use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;

use crate::collection::{Collection, Def};
use crate::error;
use crate::store::Store;

#[pyclass]
pub struct Database {
    pub store: Arc<Store>,
}

#[pymethods]
impl Database {
    fn names(&self) -> PyResult<Vec<String>> {
        self.store.names()
    }

    fn has(&self, name: &str) -> PyResult<bool> {
        Ok(self.store.names()?.iter().any(|n| n == name))
    }

    /// Drops a collection in its own write transaction; `KeyError` if absent.
    #[pyo3(name = "drop")]
    fn drop_(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        check_name(name)?;
        let txn = self.store.begin_write(py)?;
        if !txn.delete_table(Def::new(name)).map_err(error::storage)? {
            // Returning here drops the transaction, which aborts it.
            return Err(PyKeyError::new_err(name.to_string()));
        }
        py.detach(move || txn.commit().map_err(error::storage))
    }

    fn collection(&self, name: String) -> PyResult<Collection> {
        check_name(&name)?;
        Ok(Collection::new(Arc::clone(&self.store), name))
    }

    fn close(&self) {
        self.store.close();
    }
}

#[pyfunction]
#[pyo3(signature = (path=None, durable=true))]
pub fn open(path: Option<PathBuf>, durable: bool) -> PyResult<Database> {
    let store = match path {
        Some(path) => Store::file(path, durable),
        None => Store::memory(durable),
    }?;
    Ok(Database { store: Arc::new(store) })
}

/// Validates a collection name, redb panics on an empty table name.
pub fn check_name(name: &str) -> PyResult<()> {
    if name.is_empty() {
        return Err(PyValueError::new_err("collection name must be non-empty"));
    }
    Ok(())
}