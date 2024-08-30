pub mod errors;
pub mod table;
pub mod vm;
pub mod value;

use std::path::Path;

use crate::table::Row;
use crate::{errors::RhoResult, table::Table};
use crate::vm::*;

pub struct Rho {
    schema: Table,
}

impl Rho {

    pub fn open<P>(path: P) -> RhoResult<Rho>
    where P: AsRef<Path> {
        // load system tables.
        let schema = Table::open(path)?;
        Ok(Rho { schema })
    }

    pub fn prepare(&self, rql: String) -> RhoResult<Program> {
        // Hardcoded scan.
        let program = vec![
            Vop::row(0, 0),  // 0
            Vop::next(0, 0), // 1
            Vop::return_(),  // 2
        ];
        Ok(program)
    }

    ///
    pub fn exec(&self, rql: String) {
        // Initialize the virtual machine.
        let program = self.prepare(rql).unwrap();
        let mut vm = Vm {
            cursor: Vcursor::new(&self.schema),
            sink: Box::new(Printer {}),
        };
        vm.execute(&program);
    }

    pub fn close(&self) {}
}

struct Printer {}

impl Vsink for Printer {
    fn write(&self, row: &Row) {
        println!("{}", row);
    }
}
