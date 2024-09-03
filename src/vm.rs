use crate::{table::Table, Rho};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Vop is a virtual machine instruction code.
/// 
/// TODOs
/// - Lookup VM design patterns for Rust
/// - Consider codes from Lua and SQLite, but those are C
#[derive(Debug)]
pub enum Vop {
    CreateTable { table: Table },
}

impl Vop {

    pub fn create_table(table: Table) -> Vop {
        Vop::CreateTable { table }
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

    pub fn execute(&mut self, program: &Program) {
        let mut pc: usize = 0;
        loop {
            let op = &program[pc];
            pc += 1;
            match op {
                Vop::CreateTable { table } => {
                    self.db.create_table(table).expect("Error creating table");
                    break;
                },
            }
        }
    }
}
