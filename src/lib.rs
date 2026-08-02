mod collection;
mod db;
mod doc;
mod error;
mod keys;
mod store;

use pyo3::prelude::*;

#[pymodule]
fn _monadb(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<db::Database>()?;
    m.add_class::<collection::Collection>()?;
    m.add_class::<collection::CollectionIter>()?;
    m.add_function(wrap_pyfunction!(db::open, m)?)?;
    error::register(m)?;
    Ok(())
}
