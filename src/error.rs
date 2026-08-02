use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

// The general MonaDB error type.
create_exception!(monadb, Error, PyException, "Base MonaDB error.");

create_exception!(monadb, BusyError, Error, "The write gate timed out.");

create_exception!(
    monadb,
    TransactionError,
    Error,
    "Nested, closed, or misused transaction."
);

/// Wraps an internal fault a mondadb error.
pub fn internal(msg: impl std::fmt::Display) -> PyErr {
    Error::new_err(msg.to_string())
}

/// Maps any redb error to a monadb error.
pub fn storage<E: std::fmt::Display>(e: E) -> PyErr {
    Error::new_err(e.to_string())
}

/// Raises `monadb.TransactionError`.
pub fn txn(msg: impl std::fmt::Display) -> PyErr {
    TransactionError::new_err(msg.to_string())
}

/// Raises `monadb.BusyError`.
pub fn busy(msg: impl std::fmt::Display) -> PyErr {
    BusyError::new_err(msg.to_string())
}

/// Adds the exception types to the extension module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("Error", m.py().get_type::<Error>())?;
    m.add("BusyError", m.py().get_type::<BusyError>())?;
    m.add("TransactionError", m.py().get_type::<TransactionError>())?;
    Ok(())
}
