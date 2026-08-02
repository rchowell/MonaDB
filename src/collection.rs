use std::ops::Bound as RangeBound;
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use redb::{Range, ReadOnlyTable, ReadableTable, ReadableTableMetadata, Table, TableDefinition, TableError};

use crate::doc::{bytes_to_doc, doc_to_bytes};
use crate::error;
use crate::keys::{encode, key_from_py, key_to_py, prefix_from_py, successor};
use crate::store::Store;

/// The stored table shape: order-preserving key bytes to BSON document bytes.
pub type Def<'a> = TableDefinition<'a, &'static [u8], &'static [u8]>;

/// A read snapshot of one collection's table.
type Snapshot = ReadOnlyTable<&'static [u8], &'static [u8]>;


#[pyclass]
pub struct Collection {
    store: Arc<Store>,
    name: String,
}

impl Collection {
    pub fn new(store: Arc<Store>, name: String) -> Self {
        Collection { store, name }
    }

    pub fn def(&self) -> Def<'_> {
        TableDefinition::new(&self.name)
    }

    /// Opens the table for reading, or `None` when the collection does not
    /// exist — an absent collection reads as empty, never as an error.
    ///
    /// The table is returned rather than borrowed because `ReadOnlyTable` holds
    /// an `Arc` of its transaction guard: the snapshot outlives the
    /// `ReadTransaction` that opened it.
    fn read(&self) -> PyResult<Option<Snapshot>> {
        match self.store.begin_read()?.open_table(self.def()) {
            Ok(table) => Ok(Some(table)),
            Err(TableError::TableDoesNotExist(_)) => Ok(None),
            Err(e) => Err(error::storage(e)),
        }
    }

    /// Runs a write on the table in a transaction of its own.
    ///
    /// The commit releases the GIL: it is where the fsync happens, and holding
    /// the GIL through it would stall every other Python thread.
    fn write<R>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&mut Table<'_, &'static [u8], &'static [u8]>) -> PyResult<R>,
    ) -> PyResult<R> {
        let txn = self.store.begin_write(py)?;
        let out = {
            let mut table = txn.open_table(self.def()).map_err(error::storage)?;
            // An error here returns without committing, and dropping the
            // transaction aborts it.
            f(&mut table)?
        };
        py.detach(move || txn.commit().map_err(error::storage))?;
        Ok(out)
    }

    /// Opens a streaming scan over a key range.
    fn scan(
        &self,
        py: Python<'_>,
        lo: &RangeBound<Vec<u8>>,
        hi: &RangeBound<Vec<u8>>,
        mode: u8,
        reverse: bool,
    ) -> PyResult<Py<CollectionIter>> {
        let range = match self.read()? {
            Some(table) => Some(
                table
                    .range::<&[u8]>((bound_ref(lo), bound_ref(hi)))
                    .map_err(error::storage)?,
            ),
            None => None,
        };
        Py::new(
            py,
            CollectionIter {
                range: Mutex::new(range),
                mode,
                reverse,
            },
        )
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

#[pymethods]
impl Collection {
    /// Point lookup; `KeyError` on a missing key or a missing collection.
    fn get(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyDict>> {
        let k = encode(&key_from_py(key)?);
        let repr = key.repr()?.to_string();
        let Some(table) = self.read()? else {
            return Err(PyKeyError::new_err(repr));
        };
        match table.get(k.as_slice()).map_err(error::storage)? {
            Some(guard) => bytes_to_doc(py, guard.value()),
            None => Err(PyKeyError::new_err(repr)),
        }
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

    /// Upserts many documents in one commit.
    ///
    /// Everything is encoded before the transaction opens, so a bad key or
    /// document raises without having written anything.
    fn put_many(
        &self,
        py: Python<'_>,
        items: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
    ) -> PyResult<()> {
        let mut encoded = Vec::with_capacity(items.len());
        for (key, doc) in items {
            encoded.push((encode(&key_from_py(&key)?), doc_to_bytes(&doc)?));
        }
        if encoded.is_empty() {
            return Ok(()); // an empty update must not vivify the collection
        }
        self.write(py, |table| {
            for (k, v) in &encoded {
                table
                    .insert(k.as_slice(), v.as_slice())
                    .map_err(error::storage)?;
            }
            Ok(())
        })
    }

    /// Delete; `KeyError` on a missing key.
    fn delete(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<()> {
        let k = encode(&key_from_py(key)?);
        let repr = key.repr()?.to_string();
        // `WriteTransaction::open_table` *creates* a missing table, so a delete
        // against a collection that does not exist has to fail before the write
        // opens, or the failed delete vivifies the collection. This is the only
        // path that can create a table by accident — reads cannot.
        if self.read()?.is_none() {
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
        match self.read()? {
            Some(table) => Ok(table.get(k.as_slice()).map_err(error::storage)?.is_some()),
            None => Ok(false),
        }
    }

    fn len(&self) -> PyResult<u64> {
        match self.read()? {
            Some(table) => table.len().map_err(error::storage),
            None => Ok(0),
        }
    }

    /// Whole-collection iteration; `mode` selects keys, values, or items.
    fn iter_(&self, py: Python<'_>, mode: u8, reverse: bool) -> PyResult<Py<CollectionIter>> {
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
    ) -> PyResult<Py<CollectionIter>> {
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
    fn prefix_(&self, py: Python<'_>, p: &Bound<'_, PyAny>, mode: u8) -> PyResult<Py<CollectionIter>> {
        let lo = prefix_from_py(p)?;
        let hi = match successor(&lo) {
            Some(hi) => RangeBound::Excluded(hi),
            None => RangeBound::Unbounded,
        };
        self.scan(py, &RangeBound::Included(lo), &hi, mode, false)
    }

    /// The smallest entry, or `None` when the collection is empty or absent.
    fn first(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let Some(table) = self.read()? else {
            return Ok(None);
        };
        match table.first().map_err(error::storage)? {
            Some((k, v)) => Ok(Some(emit(py, 2, k.value(), v.value())?)),
            None => Ok(None),
        }
    }

    /// The largest entry, or `None` when the collection is empty or absent.
    fn last(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let Some(table) = self.read()? else {
            return Ok(None);
        };
        match table.last().map_err(error::storage)? {
            Some((k, v)) => Ok(Some(emit(py, 2, k.value(), v.value())?)),
            None => Ok(None),
        }
    }
}

/// A streaming iterator over a read snapshot that it owns outright.
#[pyclass]
pub struct CollectionIter {
    range: Mutex<Option<Range<'static, &'static [u8], &'static [u8]>>>,
    mode: u8,
    reverse: bool,
}

#[pymethods]
impl CollectionIter {
    #[allow(clippy::self_named_constructors)]
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
