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
        let mut parser = Parser::new(self.borrow_mut());
        parser.parse(rql)?;
        drop(parser);
        Ok(self.program)
    }

    /// Push a `Vop::CreateTable` instruction.
    pub fn create_table(&mut self, table: Table) -> Result<()> {
        self.program.push(Vop::create_table(table));
        Ok(())
    }

    /// Push a `Vop::DropTable` instruction.
    pub fn drop_table(&mut self, table: String) -> Result<()> {
        self.program.push(Vop::drop_table(table));
        self.program.push(Vop::Return);
        Ok(())
    }

    /// Push a `Vop::Insert` instruction.
    pub fn insert(&mut self, table: String, row: Row) -> Result<()> {
        self.program.push(Vop::insert(table, row));
        self.program.push(Vop::Return);
        Ok(())
    }

    /// TEMPORARY
    pub fn scan(&mut self, table: &str, alias: &str) -> Result<()> {
        let pc = self.pc();
        self.program.push(Vop::scan(table));
        self.program.push(Vop::next(alias, pc + 3));
        self.program.push(Vop::Return);
        self.program.push(Vop::row());
        self.program.push(Vop::next(alias, pc + 3));
        self.program.push(Vop::Return);
        Ok(())
    }

    /// Return current pc index.
    #[inline]
    fn pc(&self) -> usize {
        self.program.len()
    }
}
