//! The exception type, and the mapping from redb faults onto it.

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

// The one MonaDB error type. Everything else a caller sees is a Python
// builtin: `KeyError`, `TypeError`, `ValueError`.
create_exception!(monadb, Error, PyException, "Base MonaDB error.");

/// Wraps an internal fault as an [`Error`].
pub fn internal(msg: impl std::fmt::Display) -> PyErr {
    Error::new_err(msg.to_string())
}

/// Maps any redb error to an [`Error`].
pub fn storage<E: std::fmt::Display>(e: E) -> PyErr {
    Error::new_err(e.to_string())
}

/// Raises on any use of a database that has been closed.
pub fn closed() -> PyErr {
    Error::new_err("database is closed")
}

/// Adds the exception type to the extension module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("Error", m.py().get_type::<Error>())?;
    Ok(())
}
