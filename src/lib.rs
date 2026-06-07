pub mod error;
pub mod ir;

mod binder;
mod catalog;
mod compiler;
mod cursor;
mod display;
mod lexer;
mod storage;
mod transaction;
mod value;
mod visitor;
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
use tempfile::TempDir;

use crate::{
    binder::Binder,
    catalog::Catalog,
    ir::Statement,
    lexer::SqlLexer,
    parser::SqlParser,
    vm::{Program, Rows, VM},
};

/// The user-facing database handle.
pub struct MonaDB {
    /// The storage engine over LMDB.
    storage: Storage,
    /// The catalog reference for semantic analysis.
    catalog: Catalog,
}

impl MonaDB {
    /// Open or create a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<MonaDB> {
        let storage = Storage::open(path)?;
        let catalog = Catalog::load(&storage)?;
        Ok(MonaDB { storage, catalog })
    }

    /// Open an in-memory database.
    pub fn memory() -> Result<MonaDB> {
        let tmp_dir = TempDir::new()?;
        let tmp_pth = tmp_dir.path().join("memory.db");
        Self::open(&tmp_pth)
    }

    /// Run a query, returning a lazy iterator over its result rows. The
    /// statement's transaction commits once the iterator is exhausted.
    /// Mirrors rusqlite's `Connection::query`.
    pub fn query(&mut self, sql: &str, debug: bool) -> Result<Rows> {
        let mut stmt = Self::parse(sql)?;
        self.bind(&mut stmt)?;
        let program = self.compile(stmt)?;
        if debug {
            Self::debug(&program);
        }
        let vm = VM::init(self.storage.clone(), program);
        Ok(Rows::new(vm))
    }

    /// Run a statement to completion, committing it, and return the number of
    /// rows produced. Mirrors rusqlite's `Connection::execute`.
    pub fn execute(&mut self, sql: &str) -> Result<u64> {
        self.query(sql, false)?.finish()
    }

    /// Phase 1: Parse input string into our AST (no binding or compilation).
    pub fn parse(sql: &str) -> Result<Statement> {
        let l = SqlLexer::new(sql);
        let p = SqlParser::new();
        Ok(p.parse(l)?)
    }

    /// Phase 2: Bind all tables and variable references in the AST.
    fn bind(&self, statement: &mut Statement) -> Result<()> {
        let cat = self.catalog.clone();
        let txn = self.storage.read_txn()?;
        let mut binder = Binder::new(cat, &txn);
        binder.bind(statement)?;
        txn.commit()
    }

    /// Phase 3: Compilation is pure bytecode generation.
    #[allow(clippy::unused_self)]
    fn compile(&self, statement: Statement) -> Result<Program> {
        let cc = Compiler::new();
        cc.compile(statement)
    }

    fn debug(program: &Program) {
        println!();
        println!("addr\toperation");
        println!("----\t---------");
        for (addr, op) in program.instructions.iter().enumerate() {
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

        db.execute("create table t (id int);").unwrap();
        // let _ = db.query("select * from t;", true);
    }
}
