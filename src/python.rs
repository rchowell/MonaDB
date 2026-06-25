#![allow(deprecated)] // tolerate pyo3's IntoPy deprecations across versions

//! Python bindings (pyo3) exposing a DuckDB-style `Connection`.
//!
//! Feature-gated behind `python`; the default build never compiles this module.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyNotImplementedError};
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

fn description_for_rows(py: Python<'_>, result: &[Value]) -> PyObject {
    let Some(members) = result.first().and_then(Value::members) else {
        return py.None();
    };
    let list = PyList::empty(py);
    for (name, _) in members {
        let tuple = PyTuple::new(
            py,
            [
                name.as_str().into_py(py),
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

/// A DuckDB-style connection/cursor over a MonaDB database.
///
/// `execute()` eagerly materializes the full result set (which also commits the
/// statement's transaction, since monadb commits on iterator exhaustion); the
/// `fetch*` methods then walk that buffer.
#[pyclass(unsendable, name = "Connection")]
pub struct Connection {
    /// `None` once closed — dropping the handle releases the LMDB environment so
    /// the same path can be reopened (heed forbids opening an env twice at once).
    db: Option<Rc<RefCell<MonaDB>>>,
    result: Vec<Value>,
    cursor: usize,
}

/// A prepared statement cached from a prior `prepare` call.
#[pyclass(unsendable, name = "Statement")]
pub struct Statement {
    plan: Rc<Plan>,
    db: Rc<RefCell<MonaDB>>,
    result: Vec<Value>,
    cursor: usize,
}

impl Statement {
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

impl Connection {
    /// Run `sql` with bound `params`, draining all rows into `self.result` and
    /// resetting the cursor.
    fn run(&mut self, sql: &str, params: &Params) -> PyResult<()> {
        let out = {
            let db = self
                .db
                .as_ref()
                .ok_or_else(|| MonaDBError::new_err("connection is closed"))?;
            let mut rows = db
                .borrow_mut()
                .query_with(sql, params)
                .map_err(|e| to_pyerr(&e, sql))?;
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

    /// Errors with `monadb.Error` if the connection has been closed.
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
    /// (`db.execute(...).fetchall()`).
    #[pyo3(signature = (sql, parameters=None))]
    fn execute<'py>(
        mut slf: PyRefMut<'py, Self>,
        sql: &str,
        parameters: Option<PyObject>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = slf.py();
        let params = match &parameters {
            None => Params::none(),
            Some(obj) => py_to_params(obj.bind(py))?,
        };
        slf.run(sql, &params)?;
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

    /// Parse and cache `sql` for repeated execution via [`Statement`].
    fn prepare(&self, sql: &str) -> PyResult<Statement> {
        self.ensure_open()?;
        let db = self.db.as_ref().expect("ensure_open");
        let plan = db
            .borrow_mut()
            .cached_plan(sql)
            .map_err(|e| to_pyerr(&e, sql))?;
        Ok(Statement {
            plan,
            db: Rc::clone(db),
            result: Vec::new(),
            cursor: 0,
        })
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
        description_for_rows(py, &self.result)
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

#[pymethods]
impl Statement {
    /// Execute the prepared statement and return it for chaining.
    #[pyo3(signature = (parameters=None))]
    fn execute<'py>(
        mut slf: PyRefMut<'py, Self>,
        parameters: Option<PyObject>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = slf.py();
        let params = match &parameters {
            None => Params::none(),
            Some(obj) => py_to_params(obj.bind(py))?,
        };
        let out = {
            let sql = slf.plan.sql();
            let mut rows = slf
                .db
                .borrow_mut()
                .execute_plan(&slf.plan, &params)
                .map_err(|e| to_pyerr(&e, sql))?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().map_err(|e| to_pyerr(&e, sql))? {
                out.push(row);
            }
            out
        };
        slf.result = out;
        slf.cursor = 0;
        Ok(slf)
    }

    /// Returns the original SQL text passed to `prepare`.
    #[getter]
    fn sql(&self) -> &str {
        self.plan.sql()
    }

    /// DBAPI-style column metadata from the last result's first row.
    #[getter]
    fn description(&self, py: Python<'_>) -> PyObject {
        description_for_rows(py, &self.result)
    }

    /// Return the next buffered row, or `None` when exhausted.
    fn fetchone(&mut self, py: Python<'_>) -> PyResult<PyObject> {
        if self.cursor < self.result.len() {
            let idx = self.cursor;
            self.cursor += 1;
            Ok(value_to_py(py, &self.result[idx]))
        } else {
            Ok(py.None())
        }
    }

    #[pyo3(signature = (size=1))]
    fn fetchmany(&mut self, py: Python<'_>, size: usize) -> PyResult<Vec<PyObject>> {
        let end = self.cursor.saturating_add(size).min(self.result.len());
        Ok(self.drain(py, end))
    }

    fn fetchall(&mut self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let end = self.result.len();
        Ok(self.drain(py, end))
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

/// Open a connection. `":memory:"` (or omitted) opens an in-memory database;
/// any other string is treated as a filesystem path.
#[pyfunction]
#[pyo3(signature = (database=None, *, read_only=false, config=None))]
fn connect(
    database: Option<&str>,
    read_only: bool,
    config: Option<&Bound<'_, PyDict>>,
) -> PyResult<Connection> {
    if read_only {
        return Err(PyNotImplementedError::new_err(
            "read_only connections are not supported yet",
        ));
    }
    let cfg = config_from_py(config)?;
    let db = match database {
        None | Some(":memory:") => MonaDB::memory_with_config(cfg),
        Some(path) => MonaDB::open_with_config(path, cfg),
    }
    .map_err(|e| to_pyerr(&e, ""))?;

    Ok(Connection {
        db: Some(Rc::new(RefCell::new(db))),
        result: Vec::new(),
        cursor: 0,
    })
}

#[pymodule]
fn _monadb(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Connection>()?;
    m.add_class::<Statement>()?;
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add("Error", m.py().get_type::<MonaDBError>())?;
    Ok(())
}
