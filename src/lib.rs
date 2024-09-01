// public modules
pub mod error;
pub mod value;

// internal modules
mod table;
mod catalog;
mod vm;

use std::path::Path;
use std::result;

use error::Error;
use rusqlite::Connection;
use catalog::Catalog;

use crate::table::Row;
use crate::vm::*;

/// A typedef of the result returned by many methods.
pub type Result<T, E = Error> = result::Result<T, E>;

/// Rho represents the database connection.
pub struct Rho {
    conn: Connection,
    catalog: Catalog,
}

impl Rho {

    pub fn open<P>(path: P) -> Result<Rho>
    where P: AsRef<Path> {
        let conn = Connection::open(path)?;
        let catalog = Catalog::load(&conn)?;
        Ok(Rho { conn, catalog })
    }

    pub fn describe(&self) {
        self.catalog.describe();
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
}

impl Drop for Rho {

    fn drop(&mut self) {
        // self.conn.borrow_mut().close().unwrap();
    }
}

struct Printer {}

impl Vsink for Printer {
    fn write(&self, row: &Row) {
        println!("{}", row);
    }
}
