//! Error surface: three exception types, everything else is a Python builtin.
//!
//! `KeyError`, `TypeError`, and `ValueError` are raised directly at the call
//! sites that detect them. Everything a caller cannot anticipate — a storage
//! fault, a corrupt encoding — arrives here and becomes `monadb.Error`.

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(monadb, Error, PyException, "Base MonaDB error.");
create_exception!(monadb, BusyError, Error, "The write gate timed out.");
create_exception!(
    monadb,
    TransactionError,
    Error,
    "Nested, closed, or misused transaction."
);

/// Wraps an internal fault (storage or codec) as `monadb.Error`.
pub fn internal(msg: impl std::fmt::Display) -> PyErr {
    Error::new_err(msg.to_string())
}

/// Maps any redb error — `DatabaseError`, `TransactionError`, `TableError`,
/// `StorageError`, `CommitError` — to `monadb.Error`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy() {
        Python::attach(|py| {
            assert!(
                py.get_type::<BusyError>()
                    .is_subclass(&py.get_type::<Error>())
                    .unwrap()
            );
            assert!(
                py.get_type::<TransactionError>()
                    .is_subclass(&py.get_type::<Error>())
                    .unwrap()
            );
        });
    }
}
