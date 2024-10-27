// public modules
pub mod error;
pub mod value;
pub mod lexer;
pub mod rows;

// lalrpop module
lalrpop_mod!(
    #[allow(clippy::ptr_arg)]
    #[rustfmt::skip]
    pub parser
);

// internal modules
mod catalog;
mod compiler;
mod cursor;
mod ir;
mod vm;

use std::cell::RefCell;
use std::path::Path;
use std::result;

use compiler::Compiler;
use cursor::Cursor;
use error::Error;
use catalog::Catalog;
use ir::Table;
use lalrpop_util::lalrpop_mod;
use rows::Rows;
use value::Value;

use crate::vm::*;

/// A typedef of the result returned by many methods.
pub type Result<T, E = Error> = result::Result<T, E>;

/// Rho represents the database connection.
pub struct MonaDB {
    catalog: RefCell<Catalog>,
}

impl MonaDB {

    pub fn open<P>(path: P) -> Result<MonaDB>
    where P: AsRef<Path> {
        let catalog = Catalog::open(path)?;
        Ok(MonaDB { 
            catalog: RefCell::new(catalog),
        })
    }

    pub fn memory() -> Result<MonaDB> {
        let catalog = Catalog::memory()?;
        Ok(MonaDB {
            catalog: RefCell::new(catalog),
        })
    }

    pub fn info(&self) {
        println!("{:?}", self.catalog.borrow());
    }

    pub fn prepare(&self, rql: &str) -> Result<Code> {
        let catalog = self.catalog.borrow();
        let compiler = Compiler::new(&catalog);
        compiler.compile(rql)
    }

    pub fn exec(&mut self, rql: &str, debug: bool) -> Result<Rows<'_>> {
        let program = self.prepare(rql)?;

        // >> DEBUG
        if debug {
            println!();
            println!("-[Program]------");
            for (addr, op) in program.iter().enumerate() {
                println!("{}: {:?}", addr, op);
            }
            println!("--------");
            println!();
        }
        // >> DEBUG

        let vm = VM::init(self, program);
        Ok(Rows::new(vm))
    }

    /// Clear all entries in a table.
    pub fn clear(&self, table: &str) -> Result<()> {
        self.catalog.borrow_mut().clear(table)
    }

    /// Create a table in the catalog.
    pub fn create_table(&self, table: &Table) -> Result<()> {
        self.catalog.borrow_mut().create_table(table)
    }

    // Drop a table in the catalog.
    pub fn drop_table(&self, table: &str) -> Result<()> {
        self.catalog.borrow_mut().drop(table)
    }

    // Insert value into the table.
    pub fn insert(&self, table: &str, value: Value) -> Result<()> {
        self.catalog.borrow_mut().insert(table, value)
    }

    // Insert values into the table.
    pub fn insert_batch(&self, table: &str, values: &[Value]) -> Result<()> {
        self.catalog.borrow_mut().insert_batch(table, values)
    }

    // Opens a cursor for the table.
    pub fn scan(&self, table: &str) -> Result<Cursor> {
        self.catalog.borrow_mut().scan(table)
    }

    pub fn transaction(&self) {
        self.catalog.borrow_mut().transaction()
    }

    pub fn commit(&self) -> Result<()> {
        self.catalog.borrow_mut().commit()
    }
}
