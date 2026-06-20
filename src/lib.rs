//! MonaDB — an embedded database with a small SQL dialect compiled to bytecode.
//!
//! A statement flows through a fixed pipeline:
//!
//!   SQL text ─▶ lexer ─▶ parser ─▶ IR ─▶ binder ─▶ compiler ─▶ Vop ─▶ VM ─▶ LMDB
//!
//! [`MonaDB`] is the public handle; `query` and `execute` drive that pipeline.

pub mod error;
pub mod ir;
/// Order-preserving key encoding. Public so the order/round-trip invariants its
/// doc comments state can be exercised by the property-based conformance tests.
pub mod schema;

mod binder;
mod catalog;
mod compiler;
mod cursor;
mod display;
mod functions;
pub mod highlight;
pub mod lexer;
mod prepared;
mod read;
mod storage;
mod transaction;
mod value;
mod visitor;
mod vm;

/// Python bindings (pyo3). Compiled only with `--features python`; all pyo3
/// code is isolated here so the default build stays Python-free.
#[cfg(feature = "python")]
mod python;

// lalrpop module
lalrpop_mod!(
    #[allow(clippy::all)]
    #[rustfmt::skip]
    pub parser
);

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use compiler::Compiler;
use error::Result;
use lalrpop_util::lalrpop_mod;
use storage::Storage;
use tempfile::TempDir;

use crate::{
    binder::Binder,
    catalog::Catalog,
    ir::Statement,
    parser::SqlParser,
    vm::{Program, Rows},
};

pub use crate::lexer::{SqlLexer, Token};
pub use crate::prepared::PreparedStatement;
pub use crate::value::{Params, Value};

/// The user-facing database handle.
pub struct MonaDB {
    /// The storage engine over LMDB.
    storage: Storage,
    /// The catalog reference for semantic analysis.
    catalog: Catalog,
    /// Incremented when CREATE/DROP changes catalog membership; compiled prepares
    /// snapshot this to detect staleness.
    catalog_generation: Rc<Cell<u64>>,
}

impl MonaDB {
    /// Open or create a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<MonaDB> {
        let storage = Storage::open(path)?;
        let catalog = Catalog::load(&storage)?;
        Ok(MonaDB {
            storage,
            catalog,
            catalog_generation: Rc::new(Cell::new(0)),
        })
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
        self.query_with(sql, &Params::none(), debug)
    }

    /// Run a parameterized query, binding `?`/`$N`/`$name` placeholders against
    /// `params` before compilation. `query` is the no-parameter form.
    pub fn query_with(&mut self, sql: &str, params: &Params, debug: bool) -> Result<Rows> {
        let stmt = self.prepare(sql)?;
        self.execute_prepared(&stmt, params, debug)
    }

    /// Run a statement to completion, committing it, and return the number of
    /// rows produced. Mirrors rusqlite's `Connection::execute`.
    pub fn execute(&mut self, sql: &str) -> Result<u64> {
        self.query(sql, false)?.finish()
    }

    /// Run a parameterized statement to completion, returning its row count.
    pub fn execute_with(&mut self, sql: &str, params: &Params) -> Result<u64> {
        self.query_with(sql, params, false)?.finish()
    }

    /// Returns the current catalog generation (for stale-prepare detection).
    pub(crate) fn catalog_generation(&self) -> u64 {
        self.catalog_generation.get()
    }

    /// Phase 1: Parse input string into our AST (no binding or compilation).
    pub fn parse(sql: &str) -> Result<Statement> {
        let l = crate::lexer::SqlLexer::new(sql);
        let p = SqlParser::new();
        // Counts positional `?` placeholders, numbering them in source order.
        let param_pos = std::cell::Cell::new(0);
        Ok(p.parse(&param_pos, l)?)
    }

    /// Phase 2: Bind all tables and variable references in the AST, and
    /// substitute parameter placeholders with their bound values.
    fn bind(&self, statement: &mut Statement, params: &Params) -> Result<()> {
        let cat = self.catalog.clone();
        let txn = self.storage.read_txn()?;
        let mut binder = Binder::new(cat, &txn);
        binder.bind(statement, params)?;
        txn.commit()
    }

    /// Phase 3: Compilation is pure bytecode generation.
    #[allow(clippy::unused_self)]
    fn compile(&self, statement: Statement) -> Result<Program> {
        let cc = Compiler::new();
        cc.compile(statement)
    }

    /// Prints a program's bytecode as an address/operation table (a debug aid).
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

    #[test]
    fn ctas_from_csv() {
        let sql = "create table people as select * from 'tests/fixtures/people.csv';";
        let mut db = MonaDB::memory().unwrap();
        db.execute(sql).unwrap();
        let mut rows = db
            .query("select * from people as r order by r.name;", false)
            .unwrap();
        let mut n = 0;
        while rows.next().unwrap().is_some() {
            n += 1;
        }
        assert_eq!(n, 2);
    }
}