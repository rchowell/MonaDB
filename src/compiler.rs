use std::borrow::BorrowMut;

use crate::catalog::Catalog;
use crate::parser::Parser;
use crate::table::Table;
use crate::value::Row;
use crate::{error, Program, Result, Vop};

#[macro_export]
macro_rules! unsupported {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return Err(crate::error::Error::Unsupported(msg.to_string()))
    }}
}

/// Compiler produces OP codes from the RQL query.
///
/// It holds the necessary context to build instructions from the parse tree.
///
/// References
/// - https://github.com/lua/lua/blob/v5.4/lparser.c
/// - https://github.com/sqlite/sqlite/blob/master/src/build.c
/// - https://github.com/sqlite/sqlite/blob/master/src/select.c
pub struct Compiler<'cat> {
    catalog: &'cat Catalog,
    program: Program,
}

impl<'cat> Compiler<'cat> {
    pub fn new(catalog: &Catalog) -> Compiler {
        Compiler {
            catalog,
            program: vec![],
        }
    }

    pub fn compile(mut self, rql: &str) -> Result<Program> {
        self.push(Vop::init());
        {
            let mut parser = Parser::new(self.borrow_mut());
            parser.parse(rql)?;
        }
        self.push(Vop::exit());
        Ok(self.program)
    }

    /// Push a `Vop::CreateTable` instruction.
    pub fn create_table(&mut self, table: Table) -> Result<()> {
        self.push(Vop::create_table(table));
        Ok(())
    }

    /// Push a `Vop::DropTable` instruction.
    pub fn drop_table(&mut self, table: String) -> Result<()> {
        self.push(Vop::drop_table(table));
        Ok(())
    }

    /// Push a `Vop::Insert` instruction.
    pub fn insert(&mut self, table: String, row: Row) -> Result<()> {
        self.push(Vop::insert(table, row));
        Ok(())
    }

    /// Push a `Vop::Open` for the given table, scan into the binding, and return the pc.
    pub fn open_scan(&mut self, table: &str, alias: &str) -> Result<usize> {
        self.push(Vop::open(table));
        self.push(Vop::rewind(0)); // <-- PATCH ME
        self.push(Vop::bind(alias));
        Ok(self.pc())
    }

    /// Loop 
    /// 
    /// 1. Emit a `Vop::Next` with jmp to start of loop.
    /// 2. Patch the rewind instruction BEFORE the loop.
    /// 3. Emit an exit (TEMPORARY)
    /// 
    pub fn next(&mut self, jmp: usize) -> Result<()> {
        self.push(Vop::next(jmp));
        self.patch(jmp - 1, self.pc() + 1)?;
        Ok(())
    }

    // Patch the jump at pc[offset] = dest with the current pc.
    pub fn patch(&mut self, offset: usize, dest: usize) -> Result<()> {
        match self.program.get_mut(offset).unwrap() {
            Vop::Rewind { jmp } => *jmp = dest,
            _ => unsupported!("cannot patch jump for {:?}", offset),
        }
        Ok(())
    }

    pub fn spread(&mut self) -> Result<()> {
        self.push(Vop::spread());
        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        self.push(Vop::Init);
        Ok(())
    }

    /// TEMPORARY FOR TESTING – PUSH A BUNCH OF NO-OPs
    pub fn no_op(&mut self, n: u8) {
        for _ in 0..n {
            self.push(Vop::Init);
        }
    }

    /// Return current pc index.
    #[inline]
    fn pc(&self) -> usize {
        self.program.len() - 1
    }

    /// Push an instruction to the program.
    #[inline]
    fn push(&mut self, op: Vop) {
        self.program.push(op);
    }
}
