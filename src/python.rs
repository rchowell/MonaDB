//! Python bindings (pyo3) exposing a list-returning `Connection`.
//!
//! Feature-gated behind `python`; the default build never compiles this module.
//! Reads materialize eagerly to a Python `list`; there is no cursor, lazy rows
//! handle, or `fetch*` buffer.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};
use serde_json::Value as JsonValue;

use crate::MonaDB;
use crate::config::Config;
use crate::error::Error;
use crate::params::Params;
use crate::statement::Plan;
use crate::value::{Object, Value};

create_exception!(_monadb, MonaDBError, PyException);
create_exception!(_monadb, DuplicateKeyError, PyException);

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

/// Collect a Python sequence (an already-confirmed list or tuple) into a
/// `Vec<Value>`, converting each element through [`py_to_value`].
fn py_seq_to_values(seq: &Bound<'_, PyAny>) -> PyResult<Vec<Value>> {
    let mut vals = Vec::with_capacity(seq.len().unwrap_or(0));
    for item in seq.try_iter()? {
        vals.push(py_to_value(&item?)?);
    }
    Ok(vals)
}

/// Convert a Python `parameters` argument into engine [`Params`]. A `list`/
/// `tuple` binds positional placeholders (`?`, `$N`); a `dict` binds named ones
/// (`$name`); anything else is a type error.
fn py_to_params(obj: &Bound<'_, PyAny>) -> PyResult<Params> {
    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut named = HashMap::with_capacity(dict.len());
        for (k, v) in dict.iter() {
            let key: String = k
                .extract()
                .map_err(|_| MonaDBError::new_err("named parameter keys must be strings"))?;
            named.insert(key, py_to_value(&v)?);
        }
        return Ok(Params::named(named));
    }
    if obj.downcast::<PyList>().is_ok() || obj.downcast::<PyTuple>().is_ok() {
        return Ok(Params::positional(py_seq_to_values(obj)?));
    }
    Err(MonaDBError::new_err(
        "parameters must be a list, tuple, or dict",
    ))
}

/// Convert a native Python object into a monadb [`Value`] (the inverse of
/// [`value_to_py`]). Recurses into lists/tuples and dicts.
fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Null);
    }
    // `bool` must be checked before `int`: in Python `bool` subclasses `int`.
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(Value::bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(Value::int(i));
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(Value::float(f));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(Value::from(s));
    }
    if let Ok(bytes) = obj.downcast::<PyBytes>() {
        return Ok(Value::Bytes(Rc::from(bytes.as_bytes())));
    }
    if obj.downcast::<PyList>().is_ok() || obj.downcast::<PyTuple>().is_ok() {
        return Ok(Value::Array(Rc::new(py_seq_to_values(obj)?)));
    }
    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut object = Object::new();
        for (k, v) in dict.iter() {
            let key: String = k
                .extract()
                .map_err(|_| MonaDBError::new_err("object parameter keys must be strings"))?;
            object.insert(Rc::from(key.as_str()), py_to_value(&v)?);
        }
        return Ok(Value::Object(Rc::new(object)));
    }
    Err(MonaDBError::new_err(
        "unsupported parameter value type (expected null, bool, int, float, str, bytes, list, or dict)",
    ))
}

/// Drive `rows` to exhaustion, converting each row into a Python object.
fn collect_rows(py: Python<'_>, mut rows: crate::vm::Rows, sql: &str) -> PyResult<Vec<PyObject>> {
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|e| to_pyerr(&e, sql))? {
        out.push(value_to_py(py, &row));
    }
    Ok(out)
}

/// A prepared statement cached from a prior `prepare` call.
#[pyclass(unsendable, name = "_Statement")]
pub struct PyStatement {
    plan: Rc<Plan>,
    db: Rc<RefCell<MonaDB>>,
}

/// Connection bridge to the MonaDB instance.
#[pyclass(unsendable, name = "_Connection")]
pub struct PyConnection {
    /// Connection becomes None so that we drop the heed env (can't open twice).
    conn: Option<Rc<RefCell<MonaDB>>>,
}

impl PyConnection {
    /// Errors with `monadb.Error` if the connection has been closed.
    fn ensure_open(&self) -> PyResult<()> {
        if self.conn.is_none() {
            return Err(MonaDBError::new_err("connection is closed"));
        }
        Ok(())
    }
}

#[pymethods]
impl PyConnection {
    /// Execute `sql` and return its rows as a Python list (empty for writes/DDL).
    #[pyo3(signature = (sql, parameters=None))]
    fn execute(
        &self,
        py: Python<'_>,
        sql: &str,
        parameters: Option<PyObject>,
    ) -> PyResult<Vec<PyObject>> {
        self.ensure_open()?;
        let db = self.conn.as_ref().expect("ensure_open");
        let params = match &parameters {
            None => Params::none(),
            Some(obj) => py_to_params(obj.bind(py))?,
        };
        let rows = db
            .borrow_mut()
            .query_with(sql, &params)
            .map_err(|e| to_pyerr(&e, sql))?;
        collect_rows(py, rows, sql)
    }

    /// Parse and cache `sql` for repeated execution via [`PyStatement`].
    fn prepare(&self, sql: &str) -> PyResult<PyStatement> {
        self.ensure_open()?;
        let db = self.conn.as_ref().expect("ensure_open");
        let plan = db
            .borrow_mut()
            .cached_plan(sql)
            .map_err(|e| to_pyerr(&e, sql))?;
        Ok(PyStatement {
            plan,
            db: Rc::clone(db),
        })
    }

    /// Run a mutating statement and return the number of rows changed.
    #[pyo3(signature = (sql, parameters=None))]
    fn execute_mutations(
        &self,
        py: Python<'_>,
        sql: &str,
        parameters: Option<PyObject>,
    ) -> PyResult<u64> {
        self.ensure_open()?;
        let db = self.conn.as_ref().expect("ensure_open");
        let params = match &parameters {
            None => Params::none(),
            Some(obj) => py_to_params(obj.bind(py))?,
        };
        let mut rows = db
            .borrow_mut()
            .query_with(sql, &params)
            .map_err(|e| to_pyerr(&e, sql))?;
        while rows.next().map_err(|e| to_pyerr(&e, sql))?.is_some() {}
        Ok(rows.mutations())
    }

    /// Return the surrogate row id the next keyless insert would allocate.
    fn peek_keyless_row_id(&self, table: &str) -> PyResult<u32> {
        self.ensure_open()?;
        let db = self.conn.as_ref().expect("ensure_open");
        db.borrow_mut()
            .peek_keyless_row_id(table)
            .map_err(|e| to_pyerr(&e, ""))
    }

    /// Close the connection, releasing the underlying database handle.
    /// Subsequent operations raise `monadb.Error`.
    fn close(&mut self) {
        self.conn = None;
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

#[pymethods]
impl PyStatement {
    /// Execute the prepared statement and return its rows as a Python list.
    #[pyo3(signature = (parameters=None))]
    fn execute(&self, py: Python<'_>, parameters: Option<PyObject>) -> PyResult<Vec<PyObject>> {
        let params = match &parameters {
            None => Params::none(),
            Some(obj) => py_to_params(obj.bind(py))?,
        };
        let sql = self.plan.sql();
        let rows = self
            .db
            .borrow_mut()
            .execute_plan(&self.plan, &params)
            .map_err(|e| to_pyerr(&e, sql))?;
        collect_rows(py, rows, sql)
    }

    /// Returns the original SQL text passed to `prepare`.
    #[getter]
    fn sql(&self) -> &str {
        self.plan.sql()
    }
}

/// Builds a [`Config`] from an optional Python `config` dict.
fn config_from_py(config: Option<&Bound<'_, PyDict>>) -> PyResult<Config> {
    let mut cfg = Config::default();
    let Some(dict) = config else {
        return Ok(cfg);
    };
    for (key, value) in dict.iter() {
        let key: String = key.extract()?;
        match key.as_str() {
            "nosync" => {
                if value.extract::<bool>()? {
                    cfg = cfg.nosync();
                }
            }
            other => {
                return Err(MonaDBError::new_err(format!(
                    "unknown config key: {other:?} (supported: nosync)"
                )));
            }
        }
    }
    Ok(cfg)
}

/// Opens a connection from a filesystem path or in-memory if path omitted.
#[pyfunction]
#[pyo3(signature = (path=None, *, config=None))]
fn connect(path: Option<&str>, config: Option<&Bound<'_, PyDict>>) -> PyResult<PyConnection> {
    let cfg = config_from_py(config)?;
    let conn = match path {
        None | Some(":memory:") => MonaDB::memory_with_config(cfg),
        Some(path) => MonaDB::open_with_config(path, cfg),
    }
    .map_err(|e| to_pyerr(&e, ""))?;

    Ok(PyConnection {
        conn: Some(Rc::new(RefCell::new(conn))),
    })
}

#[pymodule]
fn _monadb(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConnection>()?;
    m.add_class::<PyStatement>()?;
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add("Error", m.py().get_type::<MonaDBError>())?;
    m.add("DuplicateKeyError", m.py().get_type::<DuplicateKeyError>())?;
    Ok(())
}
