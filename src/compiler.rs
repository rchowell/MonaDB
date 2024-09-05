use sqlparser::ast::{self, ObjectName, Statement};

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
            Statement::Drop { names, ..} => {
                if names.len() == 1 {
                    self.drop_table(names[0].clone())
                } else {
                    unsupported!("Expected single table name")
                }
            },
            _ => unsupported!("Unsupported statement"),
        }
    }

    /// Compile a CREATE TABLE statement.
    pub fn create_table(&self, create_table: ast::CreateTable) -> Result<Program> {
        let table = parser::parse_create_table(&create_table)?;
        let op = Vop::create_table(table);
        Ok(vec![op])
    }

    pub fn drop_table(&self, name: ObjectName) -> Result<Program> {
        let table = name.to_string();
        Ok(vec![Vop::drop_table(table)])
    }

    pub fn insert(&self, insert: ast::Insert) -> Result<Program> {
        let op = parser::parse_insert(&insert)?;
        Ok(vec![op])
    }
}
