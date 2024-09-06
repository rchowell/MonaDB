use std::borrow::BorrowMut;

use crate::catalog::Catalog;
use crate::parser::Parser;
use crate::table::Table;
use crate::value::Row;
use crate::{Program, Result, Vop};

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
        self.push(Vop::Init);
        {
            // traverse the parse tree
            let mut parser = Parser::new(self.borrow_mut());
            parser.parse(rql)?;
        }
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

    /// TEMPORARY
    pub fn scan(&mut self, table: &str, alias: &str) -> Result<()> {
        let pc = self.pc();
        self.push(Vop::open(table));
        self.push(Vop::next(alias, pc + 3));
        self.push(Vop::Return);

        // TODO replace me
        self.push(Vop::row());

        self.push(Vop::next(alias, pc + 3));
        self.push(Vop::Return);
        Ok(())
    }

    /// TEMPORARY
    pub fn star(&mut self, table: &str, alias: &str) -> Result<()> {
        let pc = self.pc();
        self.push(Vop::open(table));
        self.push(Vop::next(alias, pc + 3));
        self.push(Vop::Return);
        self.push(Vop::Spread);
        self.push(Vop::Return);
        self.push(Vop::next(alias, pc + 3));
        Ok(())
    }

    /// Return current pc index.
    #[inline]
    fn pc(&self) -> usize {
        self.program.len()
    }

    /// Push an instruction to the program.
    #[inline]
    fn push(&mut self, op: Vop) {
        self.program.push(op);
    }
}
