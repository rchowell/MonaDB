//! `MonaDB`: an embedded document store with Python dict semantics, on `redb`.
//!
//! The crate is a private core behind pyo3 — it is not published to crates.io
//! and exposes no Rust API. Rust owns storage, transactions, key encoding, and
//! BSON conversion; the `monadb` Python package owns the mapping protocol and
//! the dataclass/pydantic adapter.
//!
//!   Python  monadb/            `Mapping` / `MutableMapping`, model adapter
//!     │
//!   Rust    _monadb            db · txn · collection · keys · doc · error
//!     │
//!   redb 4.1                   one table per collection, `TableDefinition<&[u8], &[u8]>`

#![forbid(unsafe_code)]

mod collection;
mod db;
mod doc;
mod error;
mod keys;
mod txn;

use pyo3::prelude::*;

/// The private extension module backing the `monadb` Python package.
#[pymodule]
fn _monadb(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<db::Db>()?;
    m.add_class::<txn::Txn>()?;
    m.add_class::<collection::Collection>()?;
    m.add_class::<collection::DocIter>()?;
    m.add_function(wrap_pyfunction!(db::open, m)?)?;
    error::register(m)?;
    Ok(())
}
