#![allow(deprecated)] // tolerate pyo3's IntoPy deprecations across versions

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyNotImplementedError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};
use serde_json::Value as JsonValue;

use crate::MonaDB;
use crate::error::Error;
use crate::value::Value;

create_exception!(_monadb, MonaDBError, PyException);

/// Map a monadb error into a Python `monadb.Error`. `Error::pretty` already
/// renders the caret-annotated message for syntax errors and falls back to the
/// debug form otherwise, so we defer to it (and inherit any future formatting).
fn to_pyerr(err: &Error, sql: &str) -> PyErr {
    MonaDBError::new_err(err.pretty(sql))
}

/// Convert a monadb `Value` into a native Python object.
fn value_to_py(py: Python<'_>, value: &Value) -> PyObject {
    match value {
        Value::Oid(oid) => oid.into_py(py),
        Value::Bytes(bytes) => PyBytes::new(py, bytes).into(),
        // Everything JSON-shaped goes through the serde_json bridge, which
        // preserves object key order.
        other => json_to_py(py, &other.clone().into_json()),
    }
}

/// Recursively convert a `serde_json::Value` into a native Python object.
/// Object key order is preserved (serde_json `preserve_order` is enabled).
fn json_to_py(py: Python<'_>, json: &JsonValue) -> PyObject {
    match json {
        JsonValue::Null => py.None(),
        JsonValue::Bool(b) => b.into_py(py),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py(py)
            } else if let Some(u) = n.as_u64() {
                u.into_py(py)
            } else {
                n.as_f64().unwrap_or(f64::NAN).into_py(py)
            }
        }
        JsonValue::String(s) => s.into_py(py),
        JsonValue::Array(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(json_to_py(py, item)).expect("append to list");
            }
            list.into()
        }
        JsonValue::Object(members) => {
            let dict = PyDict::new(py);
            for (key, val) in members {
                dict.set_item(key, json_to_py(py, val))
                    .expect("set dict item");
            }
            dict.into()
        }
    }
}

/// A DuckDB-style connection/cursor over a MonaDB database.
///
/// `execute()` eagerly materializes the full result set (which also commits the
/// statement's transaction, since monadb commits on iterator exhaustion); the
/// `fetch*` methods then walk that buffer.
#[pyclass(unsendable, name = "Connection")]
pub struct Connection {
    /// `None` once closed — dropping the handle releases the LMDB environment so
    /// the same path can be reopened (heed forbids opening an env twice at once).
    db: Option<MonaDB>,
    result: Vec<Value>,
    cursor: usize,
}

impl Connection {
    /// Run `sql`, draining all rows into `self.result` and resetting the cursor.
    fn run(&mut self, sql: &str, parameters: Option<&PyObject>) -> PyResult<()> {
        if parameters.is_some() {
            return Err(PyNotImplementedError::new_err(
                "parameterized queries are not supported yet",
            ));
        }

        let out = {
            let db = self
                .db
                .as_mut()
                .ok_or_else(|| MonaDBError::new_err("connection is closed"))?;
            let mut rows = db.query(sql, false).map_err(|e| to_pyerr(&e, sql))?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().map_err(|e| to_pyerr(&e, sql))? {
                out.push(row);
            }
            out
        };

        self.result = out;
        self.cursor = 0;
        Ok(())
    }

    fn ensure_open(&self) -> PyResult<()> {
        if self.db.is_none() {
            return Err(MonaDBError::new_err("connection is closed"));
        }
        Ok(())
    }

    /// Convert and return rows `[cursor, end)`, advancing the cursor to `end`.
    fn drain(&mut self, py: Python<'_>, end: usize) -> Vec<PyObject> {
        let rows = self.result[self.cursor..end]
            .iter()
            .map(|v| value_to_py(py, v))
            .collect();
        self.cursor = end;
        rows
    }
}

#[pymethods]
impl Connection {
    /// Execute `sql` and return the connection so calls can be chained
    /// (`con.execute(...).fetchall()`).
    #[pyo3(signature = (sql, parameters=None))]
    fn execute<'py>(
        mut slf: PyRefMut<'py, Self>,
        sql: &str,
        parameters: Option<PyObject>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.run(sql, parameters.as_ref())?;
        Ok(slf)
    }

    /// Alias of [`execute`](Self::execute); the relation API is deferred.
    #[pyo3(signature = (query, parameters=None))]
    fn sql<'py>(
        slf: PyRefMut<'py, Self>,
        query: &str,
        parameters: Option<PyObject>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::execute(slf, query, parameters)
    }

    /// Return the next row (a Python object) or `None` when exhausted.
    fn fetchone(&mut self, py: Python<'_>) -> PyResult<PyObject> {
        self.ensure_open()?;
        if self.cursor < self.result.len() {
            let idx = self.cursor;
            self.cursor += 1;
            Ok(value_to_py(py, &self.result[idx]))
        } else {
            Ok(py.None())
        }
    }

    /// Return up to `size` rows, clamped to the remaining buffer.
    #[pyo3(signature = (size=1))]
    fn fetchmany(&mut self, py: Python<'_>, size: usize) -> PyResult<Vec<PyObject>> {
        self.ensure_open()?;
        let end = self.cursor.saturating_add(size).min(self.result.len());
        Ok(self.drain(py, end))
    }

    /// Return all remaining rows.
    fn fetchall(&mut self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        self.ensure_open()?;
        let end = self.result.len();
        Ok(self.drain(py, end))
    }

    /// DBAPI-style column metadata derived from the last result's first row,
    /// `(name, None, None, None, None, None, None)` per column; `None` when the
    /// rows are not objects. Names are borrowed from the row — no clone.
    #[getter]
    fn description(&self, py: Python<'_>) -> PyObject {
        let Some(Value::Object(obj)) = self.result.first() else {
            return py.None();
        };
        let list = PyList::empty(py);
        for (name, _) in obj.iter() {
            let tuple = PyTuple::new(
                py,
                [
                    name.into_py(py),
                    py.None(),
                    py.None(),
                    py.None(),
                    py.None(),
                    py.None(),
                    py.None(),
                ],
            )
            .expect("build description tuple");
            list.append(tuple).expect("append description row");
        }
        list.into()
    }

    /// Close the connection, releasing the underlying database handle.
    /// Subsequent operations raise `monadb.Error`.
    fn close(&mut self) {
        self.db = None;
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<PyObject>,
        _exc_value: Option<PyObject>,
        _traceback: Option<PyObject>,
    ) -> bool {
        self.close();
        false
    }
}

/// Open a connection. `":memory:"` (or omitted) opens an in-memory database;
/// any other string is treated as a filesystem path.
#[pyfunction]
#[pyo3(signature = (database=None, read_only=false))]
fn connect(database: Option<&str>, read_only: bool) -> PyResult<Connection> {
    if read_only {
        return Err(PyNotImplementedError::new_err(
            "read_only connections are not supported yet",
        ));
    }
    let db = match database {
        None | Some(":memory:") => MonaDB::memory(),
        Some(path) => MonaDB::open(path),
    }
    .map_err(|e| to_pyerr(&e, ""))?;

    Ok(Connection {
        db: Some(db),
        result: Vec::new(),
        cursor: 0,
    })
}

#[pymodule]
fn _monadb(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Connection>()?;
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add("Error", m.py().get_type::<MonaDBError>())?;
    Ok(())
}
