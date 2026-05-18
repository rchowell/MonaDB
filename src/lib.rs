pub mod error;
pub mod ir;

mod catalog;
mod compiler;
mod cursor;
mod display;
mod value;
mod lexer;
mod storage;
mod transaction;
mod vm;

// lalrpop module
lalrpop_mod!(
    #[allow(clippy::all)]
    #[rustfmt::skip]
    pub parser
);

use std::path::Path;

use compiler::Compiler;
use error::Result;
use lalrpop_util::lalrpop_mod;
use storage::Storage;

use crate::{catalog::Catalog, ir::Statement, lexer::SqlLexer, parser::SqlParser, vm::*};

/// The user-facing database handle.
pub struct MonaDB {
    /// The single catalog interface.
    catalog: Catalog,
    /// The storage engine over LMDB.
    storage: Storage,
}

impl MonaDB {
    /// Open or create a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<MonaDB> {
        let storage = Storage::open(path)?;
        let catalog = Catalog::load(storage.clone())?;
        Ok(MonaDB { catalog, storage })
    }

    /// Execute the given sql statement(s).
    pub fn exec(&mut self, sql: &str, debug: bool) -> Result<Rows> {
        let statement = Self::parse(sql)?;
        let program = Self::compile(statement)?;
        if debug {
            Self::debug(&program);
        }
        let vm = VM::init(self.storage.clone(), program);
        Ok(Rows::new(vm))
    }

    pub fn parse(sql: &str) -> Result<Statement> {
        let l = SqlLexer::new(sql);
        let p = SqlParser::new();
        Ok(p.parse(l)?)
    }

    pub fn compile(statement: Statement) -> Result<Program> {
        let c = Compiler::new();
        c.compile(statement)
    }

    fn debug(program: &Program) {
        println!();
        println!("addr\toperation");
        println!("----\t---------");
        for (addr, op) in program.iter().enumerate() {
            println!("{addr:04}\t{op:?}");
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_select_bytecode() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = MonaDB::open(&db_path).unwrap();

        db.exec("create table t (id int);", true).unwrap();
        // let _ = db.exec("select * from t;", true);
    }
}
