// public modules
pub mod error;
pub mod value;

// lalrpop module
lalrpop_mod!(
    #[allow(clippy::ptr_arg)]
    #[rustfmt::skip]
    pub parser
);

// internal modules
mod catalog;
mod compiler;
mod ir;
mod vm;

use std::cell::RefCell;
use std::path::Path;
use std::result;

use compiler::Compiler;
use error::Error;
use catalog::Catalog;
use ir::Table;
use lalrpop_util::lalrpop_mod;
use value::Row;

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

    // Insert a row into the table.
    pub fn insert(&self, table: &str, row: Row) -> Result<()> {
        self.catalog.borrow_mut().insert(table, row)
    }

    // TODO TEMPORARY – REMOVE ME ??
    pub fn select(&self, table: &str) -> Result<Vec<Row>> {
        self.catalog.borrow_mut().scan(table)
    }
}

mod test {

    use super::*;

    #[test]
    fn test_rho() {
        let input = "SELECT a AS b FROM foo;";
        let parser = parser::RqlParser::new();
        let stmt = parser.parse(input).unwrap();
        println!("{:?}", stmt);
    }
}