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
    },
    /// Delete a table from the catalog.
    DropTable {
        table: String,
    },
}

impl Vop {

    pub fn create_table(table: Table) -> Vop {
        Vop::CreateTable { table }
    }

    pub fn drop_table(table: String) -> Vop {
        Vop::DropTable { table }
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
                    self.db.create_table(table)?;
                    break;
                },
                Vop::Insert { table, row } => {
                    self.db.insert(table, row.clone())?;
                    break;
                },
                Vop::DropTable { table } => {
                    self.db.drop_table(table)?;
                    break;
                },
            }
        }
        Ok(())
    }
}
