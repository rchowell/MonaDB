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
    ptr: usize,
}

impl<'cat> Compiler<'cat> {
    pub fn new(catalog: &Catalog) -> Compiler {
        Compiler {
            catalog,
            program: vec![],
            ptr: 0,
        }
    }

    pub fn compile(mut self, rql: &str) -> Result<Program> {
        self.push(Vop::init());
        {
            let mut parser = Parser::new(self.borrow_mut());
            parser.parse(rql)?;
        }
        self.push(Vop::exit());
        Ok(self.program)
    }

    /// PUsh a `Vop::Clear` instruction.
    pub fn clear(&mut self, table: &str) {
        self.push(Vop::clear(table));
    }

    /// Push a `Vop::CreateTable` instruction.
    pub fn create_table(&mut self, table: Table) -> Result<()> {
        self.push(Vop::create_table(table));
        Ok(())
    }

    /// Push a `Vop::Drop` instruction.
    pub fn drop(&mut self, table: String) -> Result<()> {
        self.push(Vop::drop(table));
        Ok(())
    }

    /// Push a `Vop::Insert` instruction.
    pub fn insert(&mut self, table: String, row: Row) -> Result<()> {
        self.push(Vop::insert(table, row));
        Ok(())
    }

    /// Push a `Vop::Open` for the given table, scan into the binding, and return the pc.
    pub fn open_scan(&mut self, table: &str, alias: &str) -> Result<usize> {
        self.push(Vop::open(table));
        self.push(Vop::rewind(0)); // <-- PATCH ME
        self.push(Vop::bind(alias));
        Ok(self.pc())
    }

    /// Loop 
    /// 
    /// 1. Emit a `Vop::Next` with jmp to start of loop.
    /// 2. Patch the rewind instruction BEFORE the loop.
    /// 
    pub fn next(&mut self, jmp: usize) -> Result<()> {
        self.push(Vop::next(jmp));
        self.patch(jmp - 1, self.pc() + 1)?;
        Ok(())
    }

    pub fn return_(&mut self, ptr: usize) {
        self.push(Vop::Return { ptr });
    }

    /// Patch the jump at pc[offset] = dest with the current pc.
    pub fn patch(&mut self, offset: usize, dest: usize) -> Result<()> {
        match self.program.get_mut(offset).unwrap() {
            Vop::Rewind { jmp } => *jmp = dest,
            _ => unsupported!("cannot patch jump at pc[{}]", offset),
        }
        Ok(())
    }

    /// Push a `Vop::Row` instruction for SELECT *.
    pub fn spread(&mut self) {
        let dest = self.alloc(1);
        self.push(Vop::spread(dest));
    }

    /// Push a `Vop::Obj` instruction.
    pub fn obj(&mut self, ptr: usize, members: Vec<String>) {
        self.push(Vop::obj(ptr, members));
    }

    pub fn var(&mut self, name: &str, dest: usize) {
        self.push(Vop::var(name, dest));
    }

    /// "Allocates" n registers and returns a pointer to the first register.
    pub fn alloc(&mut self, n: usize) -> usize {
        let curr = self.ptr;
        self.ptr = curr + n;
        curr
    }

    /// Free n registers.
    pub fn free(&mut self, n: usize) {
        self.ptr -= n;
    }

    /// Return current pc index.
    #[inline]
    fn pc(&self) -> usize {
        self.program.len() - 1
    }

    /// Push an instruction to the program.
    #[inline]
    fn push(&mut self, op: Vop) {
        self.program.push(op);
    }
}
