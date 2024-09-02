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

    pub fn prepare(&self, rql: String) -> Result<Program> {
        // Hardcoded scan.
        let program = vec![
            Vop::row(0, 0),  // 0
            Vop::next(0, 0), // 1
            Vop::return_(),  // 2
        ];
        Ok(program)
    }

    ///
    pub fn exec(&mut self, rql: String) -> Result<()> {
        // Initialize the virtual machine.
        // let program = self.prepare(rql).unwrap();
        // let mut vm = Vm {
        //     cursor: Vcursor::new(&self.schema),
        //     sink: Box::new(Printer {}),
        // };
        // vm.execute(&program);
        Ok(())
    }

    // TODO TEMPORARY
    pub fn create_table(&self, name: String) -> Result<()> {
        let table = Table {
            name,
            rql: "todo".to_string(),
        };
        self.catalog.borrow_mut().create_table(table)
    }

    // TODO TEMPORARY
    pub fn insert_row(&self, table: String, value: String) -> Result<()> {
        let row = Row::from_str(&value);
        let catalog = self.catalog.borrow_mut();
        let table = catalog.load_table(&table)?;
        println!("INSERT INTO {} VALUES {}", table.name, row);
        Ok(())
    }
}

impl Drop for Rho {

    fn drop(&mut self) {
        // self.sess.borrow_mut().close().unwrap();
    }
}
