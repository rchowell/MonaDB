// public modules
pub mod error;
pub mod value;

// internal modules
mod table;
mod catalog;
mod vm;

use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::path::Path;
use std::result;

use error::Error;
use catalog::Catalog;
use table::Table;

use crate::table::Row;
use crate::vm::*;

/// A typedef of the result returned by many methods.
pub type Result<T, E = Error> = result::Result<T, E>;

/// Rho represents the database sessection.
pub struct Rho {
    catalog: RefCell<Catalog>,
}

impl Rho {

    pub fn open<P>(path: P) -> Result<Rho>
    where P: AsRef<Path> {
        let catalog = Catalog::open(path)?;
        Ok(Rho { 
            catalog: RefCell::new(catalog),
        })
    }

    pub fn info(&self) {
        println!("{:?}", self.catalog.borrow());
    }

    pub fn prepare(&self, _rql: String) -> Result<Program> {
        todo!("prepare")
    }

    // TODO
    pub fn exec(&mut self, _rql: String) -> Result<()> {
        todo!("exec")
    }

    // TODO TEMPORARY
    pub fn create_table(&self, name: String) -> Result<()> {
        let table = Table::new(name);
        self.catalog.borrow_mut().create_table(table)
    }

    // TODO TEMPORARY
    pub fn drop_table(&self, table: String) -> Result<()> {
        self.catalog.borrow_mut().drop_table(&table)
    }

    // TODO TEMPORARY
    pub fn insert(&self, table: String, value: String) -> Result<()> {
        let row = Row::from_str(&value);
        self.catalog.borrow_mut().insert(&table, row)
    }
}

impl Drop for Rho {

    fn drop(&mut self) {
        // self.sess.borrow_mut().close().unwrap();
    }
}
