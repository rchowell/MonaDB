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

impl <'cat> Compiler<'cat> {

    pub fn new(catalog: &Catalog) -> Compiler {
        Compiler { 
            catalog,
            program: vec![],
         }
    }

    pub fn compile(mut self, rql: &str) -> Result<Program> {
        {
            let mut parser = Parser::new(self.borrow_mut());
            parser.parse(rql)?;
        }
        Ok(self.program)
    }

    /// Push a `Vop::CreateTable` instruction.
    pub fn create_table(&mut self, table: Table) -> Result<()> {
        let op = Vop::create_table(table);
        self.program.push(op);
        Ok(())
    }

    /// Push a `Vop::DropTable` instruction.
    pub fn drop_table(&mut self, table: String) -> Result<()> {
        let op =  Vop::drop_table(table);
        self.program.push(op);
        Ok(())
    }

    /// Push a `Vop::Insert` instruction.
    pub fn insert(&mut self, table: String, row: Row) -> Result<()> {
        let op = Vop::insert(table, row);
        self.program.push(op);
        Ok(())
    }
}
