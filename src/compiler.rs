use sqlparser::ast::{self, Statement};

use crate::catalog::Catalog;
use crate::{parser, Program, Result, Vop};

#[macro_export]
macro_rules! unsupported {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return Err(crate::error::Error::Unsupported(msg.to_string()))
    }}
}

/// Compiler produces OP codes from the RQL query.
/// 
/// References
/// - https://github.com/lua/lua/blob/v5.4/lparser.c
/// - https://github.com/sqlite/sqlite/blob/master/src/build.c
/// - https://github.com/sqlite/sqlite/blob/master/src/select.c
pub struct Compiler<'a> {
    catalog: &'a Catalog,
}

impl <'a> Compiler<'a> {

    pub fn new(catalog: &Catalog) -> Compiler {
        Compiler { catalog }
    }

    pub fn compile(&self, rql: &str) -> Result<Program> {
        match parser::parse(rql)? {
            Statement::CreateTable(create_table) => self.create_table(create_table),
            Statement::Insert(insert) => self.insert(insert),
            _ => unsupported!("Unsupported statement"),
        }
    }

    /// Compile a CREATE TABLE statement.
    pub fn create_table(&self, create_table: ast::CreateTable) -> Result<Program> {
        let table = parser::parse_create_table(&create_table)?;
        let op = Vop::create_table(table);
        Ok(vec![op])
    }

    pub fn insert(&self, insert: ast::Insert) -> Result<Program> {
        let op = parser::parse_insert(&insert)?;
        Ok(vec![op])
    }
}
