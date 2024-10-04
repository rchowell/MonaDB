// public modules
pub mod error;
pub mod value;
pub mod lexer;

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
use value::Record;

use crate::vm::*;

/// A typedef of the result returned by many methods.
pub type Result<T, E = Error> = result::Result<T, E>;

/// Rho represents the database sessection.
pub struct Rho {
    debug: bool,
    catalog: RefCell<Catalog>,
}

impl Rho {

    pub fn open<P>(path: P) -> Result<Rho>
    where P: AsRef<Path> {
        let catalog = Catalog::open(path)?;
        Ok(Rho { 
            debug: true,
            catalog: RefCell::new(catalog),
        })
    }

    pub fn memory() -> Result<Rho> {
        let catalog = Catalog::memory()?;
        Ok(Rho {
            debug: true,
            catalog: RefCell::new(catalog),
        })
    }

    pub fn info(&self) {
        println!("{:?}", self.catalog.borrow());
    }

    pub fn prepare(&self, rql: &str) -> Result<Program> {
        let catalog = self.catalog.borrow();
        let compiler = Compiler::new(&catalog);
        compiler.compile(rql)
    }

    pub fn exec(&mut self, rql: &str) -> Result<()> {
        let program = self.prepare(rql)?;
        // >> DEBUG
        if self.debug {
            println!();
            println!("-[Program]------");
            for (addr, op) in program.iter().enumerate() {
                println!("{}: {:?}", addr, op);
            }
            println!("--------");
            println!();
        }
        // >> DEBUG
        let mut vm = VM::init(self, program);
        loop {
            match vm.next() {
                Ok(Some(row)) => println!("{:?}", row),
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
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

    // Insert rows into the table.
    pub fn insert(&self, table: &str, record: Record) -> Result<usize> {
        self.catalog.borrow_mut().insert(table, record)
    }

    // TODO TEMPORARY – REMOVE ME ??
    pub fn select(&self, table: &str) -> Result<Cursor> {
        self.catalog.borrow_mut().scan(table)
    }

    pub fn transaction(&self) {
        self.catalog.borrow_mut().transaction()
    }

    pub fn commit(&self) -> Result<()> {
        self.catalog.borrow_mut().commit()
    }
}