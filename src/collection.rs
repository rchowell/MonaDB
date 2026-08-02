//! Collection handles: the core mapping operations.
//!
//! A handle is (shared database state, table name, scope flag). A
//! database-scoped handle runs each operation in its own transaction; a
//! transaction-scoped handle uses the open write transaction, which is what
//! gives read-your-writes inside a `with` block.

use std::ops::Bound as RangeBound;
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use redb::{ReadableTable, ReadableTableMetadata, Table, TableDefinition, TableHandle};

use crate::db::DbInner;
use crate::doc::{bytes_to_doc, doc_to_bytes};
use crate::error::{self, TransactionError};
use crate::keys::{encode, key_from_py, key_to_py, prefix_from_py, successor};

/// An inclusive/exclusive byte-range pair, as redb's `range` wants it.
type Bounds<'a> = (RangeBound<&'a [u8]>, RangeBound<&'a [u8]>);

/// One stored entry, owned.
type Entry = (Vec<u8>, Vec<u8>);

/// The stored table shape: order-preserving key bytes to BSON document bytes.
pub type Def<'a> = TableDefinition<'a, &'static [u8], &'static [u8]>;

/// The reads a collection needs, in an object-safe form.
///
/// redb's own `ReadableTable` has generic methods and so cannot become a trait
/// object, yet the two read sources have different types — an owned
/// `ReadOnlyTable` from a snapshot, and a `Table<'txn>` borrowed from the open
/// write transaction. This trait is the seam between them. Each method converts
/// while the value is still borrowed, so no copy of the stored bytes is made.
pub trait Readable {
    fn get_doc(&self, py: Python<'_>, key: &[u8]) -> PyResult<Option<Py<PyDict>>>;
    fn has(&self, key: &[u8]) -> PyResult<bool>;
    fn count(&self) -> PyResult<u64>;
    fn collect_range(&self, bounds: Bounds<'_>) -> PyResult<Vec<Entry>>;
    fn first_entry(&self) -> PyResult<Option<Entry>>;
    fn last_entry(&self) -> PyResult<Option<Entry>>;
}

macro_rules! impl_readable {
    ($t:ty) => {
        impl Readable for $t {
            fn get_doc(&self, py: Python<'_>, key: &[u8]) -> PyResult<Option<Py<PyDict>>> {
                match self.get(key).map_err(error::storage)? {
                    Some(guard) => Ok(Some(bytes_to_doc(py, guard.value())?)),
                    None => Ok(None),
                }
            }

            fn has(&self, key: &[u8]) -> PyResult<bool> {
                Ok(self.get(key).map_err(error::storage)?.is_some())
            }

            fn count(&self) -> PyResult<u64> {
                self.len().map_err(error::storage)
            }

            fn collect_range(&self, bounds: Bounds<'_>) -> PyResult<Vec<Entry>> {
                let mut out = Vec::new();
                for entry in self.range::<&[u8]>(bounds).map_err(error::storage)? {
                    let (k, v) = entry.map_err(error::storage)?;
                    out.push((k.value().to_vec(), v.value().to_vec()));
                }
                Ok(out)
            }

            fn first_entry(&self) -> PyResult<Option<Entry>> {
                Ok(self
                    .first()
                    .map_err(error::storage)?
                    .map(|(k, v)| (k.value().to_vec(), v.value().to_vec())))
            }

            fn last_entry(&self) -> PyResult<Option<Entry>> {
                Ok(self
                    .last()
                    .map_err(error::storage)?
                    .map(|(k, v)| (k.value().to_vec(), v.value().to_vec())))
            }
        }
    };
}

impl_readable!(redb::ReadOnlyTable<&'static [u8], &'static [u8]>);
impl_readable!(Table<'_, &'static [u8], &'static [u8]>);

#[pyclass]
pub struct Collection {
    inner: Arc<DbInner>,
    name: String,
    txn_scoped: bool,
}

impl Collection {
    pub fn new(inner: Arc<DbInner>, name: String, txn_scoped: bool) -> Self {
        Collection {
            inner,
            name,
            txn_scoped,
        }
    }

    pub fn def(&self) -> Def<'_> {
        TableDefinition::new(&self.name)
    }

    /// Runs a read over the table, passing `None` when the collection does not
    /// exist — an absent collection reads as empty, never as an error.
    ///
    /// The transaction-scoped branch checks `list_tables` first because
    /// `WriteTransaction::open_table` *creates* a missing table, which would
    /// silently vivify a collection on a pure read.
    fn read<R>(&self, f: impl FnOnce(Option<&dyn Readable>) -> PyResult<R>) -> PyResult<R> {
        if self.txn_scoped {
            let active = self.inner.active.lock().expect("txn poisoned");
            let txn = active
                .as_ref()
                .ok_or_else(|| TransactionError::new_err("transaction is not open"))?;
            let exists = txn
                .list_tables()
                .map_err(error::storage)?
                .any(|h| h.name() == self.name);
            if !exists {
                return f(None);
            }
            let table = txn.open_table(self.def()).map_err(error::storage)?;
            f(Some(&table))
        } else {
            let read = self.inner.begin_read()?;
            match read.open_table(self.def()) {
                Ok(table) => f(Some(&table)),
                Err(redb::TableError::TableDoesNotExist(_)) => f(None),
                Err(e) => Err(error::storage(e)),
            }
        }
    }

    /// Runs a write on the table.
    ///
    /// Transaction scope joins the open transaction. Database scope is
    /// gate, `begin_write`, apply, commit — the gate is released on every path,
    /// success or failure.
    fn write<R>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&mut Table<'_, &'static [u8], &'static [u8]>) -> PyResult<R>,
    ) -> PyResult<R> {
        if self.txn_scoped {
            let active = self.inner.active.lock().expect("txn poisoned");
            let txn = active
                .as_ref()
                .ok_or_else(|| TransactionError::new_err("transaction is not open"))?;
            let mut table = txn.open_table(self.def()).map_err(error::storage)?;
            f(&mut table)
        } else {
            py.detach(|| self.inner.acquire_gate())?;
            let result = (|| {
                let txn = self.inner.begin_write()?;
                let out = {
                    let mut table = txn.open_table(self.def()).map_err(error::storage)?;
                    f(&mut table)?
                };
                txn.commit().map_err(error::storage)?;
                Ok(out)
            })();
            self.inner.gate.release();
            result
        }
    }

    /// Whether this collection exists on disk.
    fn exists(&self) -> PyResult<bool> {
        Ok(self.inner.names()?.contains(&self.name))
    }

    /// Opens a scan.
    ///
    /// Database scope returns a streaming [`DocIter`]: `ReadOnlyTable::range`
    /// yields a `Range<'static>` that holds its own transaction guard, so the
    /// iterator keeps its snapshot alive with nothing borrowed from here.
    ///
    /// Transaction scope materializes instead. A `Table<'txn>` borrows the open
    /// write transaction and cannot cross the FFI boundary beside it without a
    /// self-reference, and this crate forbids `unsafe`. Read-your-writes is
    /// preserved; the cost is holding the result in memory.
    fn scan(
        &self,
        py: Python<'_>,
        lo: &RangeBound<Vec<u8>>,
        hi: &RangeBound<Vec<u8>>,
        mode: u8,
        reverse: bool,
    ) -> PyResult<Py<PyAny>> {
        if self.txn_scoped {
            let items = self.read(|table| match table {
                None => Ok(Vec::new()),
                Some(t) => t.collect_range((bound_ref(lo), bound_ref(hi))),
            })?;
            let list = PyList::empty(py);
            let ordered: Box<dyn Iterator<Item = &Entry>> = if reverse {
                Box::new(items.iter().rev())
            } else {
                Box::new(items.iter())
            };
            for (k, v) in ordered {
                list.append(emit(py, mode, k, v)?)?;
            }
            Ok(list.into_any().unbind())
        } else {
            let read = self.inner.begin_read()?;
            let range = match read.open_table(self.def()) {
                Ok(table) => Some(
                    table
                        .range::<&[u8]>((bound_ref(lo), bound_ref(hi)))
                        .map_err(error::storage)?,
                ),
                Err(redb::TableError::TableDoesNotExist(_)) => None,
                Err(e) => return Err(error::storage(e)),
            };
            let iter = DocIter {
                range: Mutex::new(range),
                mode,
                reverse,
            };
            Ok(Py::new(py, iter)?.into_any())
        }
    }
}

/// Borrows an owned bound as a slice bound.
fn bound_ref(b: &RangeBound<Vec<u8>>) -> RangeBound<&[u8]> {
    match b {
        RangeBound::Included(v) => RangeBound::Included(v.as_slice()),
        RangeBound::Excluded(v) => RangeBound::Excluded(v.as_slice()),
        RangeBound::Unbounded => RangeBound::Unbounded,
    }
}

/// Renders one entry per mode: 0 = key, 1 = document, 2 = (key, document).
fn emit(py: Python<'_>, mode: u8, k: &[u8], v: &[u8]) -> PyResult<Py<PyAny>> {
    Ok(match mode {
        0 => key_to_py(py, k)?,
        1 => bytes_to_doc(py, v)?.into_any(),
        _ => PyTuple::new(py, [key_to_py(py, k)?, bytes_to_doc(py, v)?.into_any()])?
            .into_any()
            .unbind(),
    })
}

/// A streaming iterator over a read snapshot that it owns outright.
#[pyclass]
pub struct DocIter {
    range: Mutex<Option<redb::Range<'static, &'static [u8], &'static [u8]>>>,
    mode: u8,
    reverse: bool,
}

#[pymethods]
impl DocIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let mut guard = self.range.lock().expect("iter poisoned");
        let Some(range) = guard.as_mut() else {
            return Ok(None);
        };
        let step = if self.reverse {
            range.next_back()
        } else {
            range.next()
        };
        match step {
            None => {
                *guard = None; // release the snapshot as soon as it is spent
                Ok(None)
            }
            Some(Err(e)) => Err(error::storage(e)),
            Some(Ok((k, v))) => Ok(Some(emit(py, self.mode, k.value(), v.value())?)),
        }
    }
}

#[pymethods]
impl Collection {
    /// Point lookup; `KeyError` on a missing key or a missing collection.
    fn get(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let k = encode(&key_from_py(key)?);
        let repr = key.repr()?.to_string();
        self.read(|table| {
            let Some(table) = table else {
                return Err(PyKeyError::new_err(repr.clone()));
            };
            match table.get_doc(py, &k)? {
                Some(doc) => Ok(doc.into_any()),
                None => Err(PyKeyError::new_err(repr.clone())),
            }
        })
    }

    /// Upsert. Assignment overwrites, exactly as it does for a `dict`.
    fn put(&self, py: Python<'_>, key: &Bound<'_, PyAny>, doc: &Bound<'_, PyAny>) -> PyResult<()> {
        let k = encode(&key_from_py(key)?);
        let v = doc_to_bytes(doc)?;
        self.write(py, |table| {
            table
                .insert(k.as_slice(), v.as_slice())
                .map_err(error::storage)?;
            Ok(())
        })
    }

    /// Delete; `KeyError` on a missing key.
    ///
    /// A missing collection short-circuits before taking the write gate, so a
    /// failed delete never creates the table it was looking in.
    fn delete(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<()> {
        let k = encode(&key_from_py(key)?);
        let repr = key.repr()?.to_string();
        if !self.exists()? {
            return Err(PyKeyError::new_err(repr));
        }
        let existed = self.write(py, |table| {
            Ok(table.remove(k.as_slice()).map_err(error::storage)?.is_some())
        })?;
        if existed {
            Ok(())
        } else {
            Err(PyKeyError::new_err(repr))
        }
    }

    fn contains(&self, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        let k = encode(&key_from_py(key)?);
        self.read(|table| match table {
            None => Ok(false),
            Some(t) => t.has(&k),
        })
    }

    fn len(&self) -> PyResult<u64> {
        self.read(|table| match table {
            None => Ok(0),
            Some(t) => t.count(),
        })
    }

    /// Whole-collection iteration; `mode` selects keys, values, or items.
    fn iter_(&self, py: Python<'_>, mode: u8, reverse: bool) -> PyResult<Py<PyAny>> {
        self.scan(
            py,
            &RangeBound::Unbounded,
            &RangeBound::Unbounded,
            mode,
            reverse,
        )
    }

    /// Half-open `[start, stop)` scan; either bound may be `None` for unbounded.
    #[pyo3(signature = (start, stop, mode))]
    fn range_(
        &self,
        py: Python<'_>,
        start: Option<&Bound<'_, PyAny>>,
        stop: Option<&Bound<'_, PyAny>>,
        mode: u8,
    ) -> PyResult<Py<PyAny>> {
        let lo = match start {
            Some(s) => RangeBound::Included(encode(&key_from_py(s)?)),
            None => RangeBound::Unbounded,
        };
        let hi = match stop {
            Some(s) => RangeBound::Excluded(encode(&key_from_py(s)?)),
            None => RangeBound::Unbounded,
        };
        self.scan(py, &lo, &hi, mode, false)
    }

    /// Every key sharing a prefix: a `str`/`bytes` text prefix, or a tuple of
    /// leading components.
    fn prefix_(&self, py: Python<'_>, p: &Bound<'_, PyAny>, mode: u8) -> PyResult<Py<PyAny>> {
        let lo = prefix_from_py(p)?;
        let hi = match successor(&lo) {
            Some(hi) => RangeBound::Excluded(hi),
            None => RangeBound::Unbounded,
        };
        self.scan(py, &RangeBound::Included(lo), &hi, mode, false)
    }

    /// The smallest entry, or `None` when the collection is empty or absent.
    fn first(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.read(|table| match table {
            None => Ok(None),
            Some(t) => match t.first_entry()? {
                None => Ok(None),
                Some((k, v)) => Ok(Some(emit(py, 2, &k, &v)?)),
            },
        })
    }

    /// The largest entry, or `None` when the collection is empty or absent.
    fn last(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.read(|table| match table {
            None => Ok(None),
            Some(t) => match t.last_entry()? {
                None => Ok(None),
                Some((k, v)) => Ok(Some(emit(py, 2, &k, &v)?)),
            },
        })
    }
}
