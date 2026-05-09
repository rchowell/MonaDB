// public modules
pub mod error;
pub mod value;
pub mod lexer;
pub mod rows;
pub mod storage;

// lalrpop module
lalrpop_mod!(
    #[allow(clippy::all)]
    #[rustfmt::skip]
    pub parser
);

// internal modules
mod compiler;
mod ir;
mod vm;

use std::path::Path;

use compiler::Compiler;
use error::Error;
use lalrpop_util::lalrpop_mod;
use rows::Rows;
use storage::Storage;
use tempfile::TempDir;

use crate::vm::*;

/// A typedef of the result returned by many methods.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The user-facing database handle. Holds an `Engine` and runs SQL programs.
pub struct MonaDB {
    /// The storage engine.
    storage: Storage,
    /// Held to keep an in-memory db alive for the lifetime of the handle. `Some`
    /// only for `MonaDB::memory()`; `None` for file-backed instances.
    _tmp: Option<TempDir>,
}

impl MonaDB {
    /// Open or create a database at `path`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<MonaDB> {
        let engine = Storage::open(path)?;
        Ok(MonaDB { storage: engine, _tmp: None })
    }

    /// Create an ephemeral database in a tempfile. Wipes when the handle drops.
    pub fn memory() -> Result<MonaDB> {
        let tmp = TempDir::new()?;
        let path = tmp.path().join("memory.mdb");
        let engine = Storage::open(&path)?;
        Ok(MonaDB {
            storage: engine,
            _tmp: Some(tmp),
        })
    }

    pub fn exec(&mut self, sql: &str, debug: bool) -> Result<Rows<'_>> {
        let program = self.prepare(sql)?;
        if debug {
            Self::debug(&program);
        }
        let vm = VM::init(&self.storage, program);
        Ok(Rows::new(vm))
    }

    fn prepare(&self, sql: &str) -> Result<Program> {
        let compiler = Compiler::new();
        compiler.compile(sql)
    }

    fn debug(program: &Program) {
        println!();
        println!("┌──────┬──────┬──────┐");
        println!("│ addr │ code │ args │");
        println!("├──────┼──────┼──────┤");
        for (addr, op) in program.iter().enumerate() {
            println!("│  {:03} │ {:?}", addr, op);
        }
        println!("└──────┘");
        println!();
    }
}
