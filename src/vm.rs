use crate::{table::Table, value::Row, Rho};
use crate::Result;

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Vop is a virtual machine instruction code.
/// 
/// TODOs
/// - Lookup VM design patterns for Rust
/// - Consider codes from Lua and SQLite, but those are C
#[derive(Debug)]
pub enum Vop {
    /// Insert the table into the catalog table.
    CreateTable {
        table: Table,
    },
    /// Insert a row into a table.
    Insert {
        table: String,
        row: Row,
    }
}

impl Vop {

    pub fn create_table(table: Table) -> Vop {
        Vop::CreateTable { table }
    }

    pub fn insert(table: String, row: Row) -> Vop {
        Vop::Insert { table, row }
    }
}

/// VM holds the state of the virtual machine.
pub struct VM<'a> { 
    db: &'a Rho,
}

impl <'a> VM<'a> {

    pub fn new(db: &Rho) -> VM {
        VM { db }
    }

    pub fn execute(&mut self, program: &Program) -> Result<()> {
        let mut pc: usize = 0;
        loop {
            let op = &program[pc];
            pc += 1;
            match op {
                Vop::CreateTable { table } => {
                    self.db.create_table(table).expect("Error creating table");
                    break;
                },
                Vop::Insert { table, row } => {
                    // clone the row
                    let row = row.clone();
                    self.db.insert(table, row).expect("Error inserting row");
                    break;
                },
            }
        }
        Ok(())
    }
}
