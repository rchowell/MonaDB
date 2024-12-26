// public modules
pub mod error;
pub mod value;
pub mod lexer;
pub mod rows;
pub mod cask;

// lalrpop module
lalrpop_mod!(
    #[allow(clippy::all)]
    #[rustfmt::skip]
    pub parser
);

// internal modules
mod connection;
mod compiler;
mod cursor;
mod ir;
mod vm;

use std::path::Path;
use std::result;

use compiler::Compiler;
use error::Error;
use connection::Connection;
use lalrpop_util::lalrpop_mod;
use rows::Rows;

use crate::vm::*;

/// A typedef of the result returned by many methods.
pub type Result<T, E = Error> = result::Result<T, E>;

/// Rho represents the database connection.
pub struct MonaDB {
    connection: Connection,
}

impl MonaDB {

    pub fn open<P>(path: P) -> Result<MonaDB>
    where P: AsRef<Path> {
        let connection = Connection::open(path)?;
        Ok(MonaDB { connection })
    }

    pub fn memory() -> Result<MonaDB> {
        let connection = Connection::memory()?;
        Ok(MonaDB { connection })
    }

    pub fn info(&self) {
        println!("{:?}", self.connection);
    }

    pub fn exec(&mut self, rql: &str, debug: bool) -> Result<Rows<'_>> {
        let program = self.prepare(rql)?;
        if debug {
            Self::debug(&program)
        }
        let vm = VM::init(&mut self.connection, program);
        Ok(Rows::new(vm))
    }

    fn prepare(&self, rql: &str) -> Result<Program> {
        let compiler = Compiler::new();
        compiler.compile(rql)
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
