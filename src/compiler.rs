use std::borrow::BorrowMut;
use std::vec;
use crate::catalog::Catalog;
use crate::parser::RqlParser;
use crate::sqlparser::Parser;
use crate::table::Table;
use crate::value::Row;
use crate::{Program, Result, Vop};
use crate::ir::*;

#[macro_export]
macro_rules! unsupported {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return Err(crate::error::Error::Unsupported(msg.to_string()))
    }}
}

/// Compiler translates RQL queries to Vops.
///
/// References
/// - https://github.com/lua/lua/blob/v5.4/lparser.c
/// - https://github.com/sqlite/sqlite/blob/master/src/build.c
/// - https://github.com/sqlite/sqlite/blob/master/src/select.c
pub struct Compiler<'cat> {
    catalog: &'cat Catalog,
    program: Program,
    ptr: usize,
    // scope information
    // scope_dest: usize,
    // scope_size: usize,
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
        match &parse(rql)? {
            Statement::Delete(delete) => self.cc_delete(delete),
            Statement::Drop(drop) => self.cc_drop(drop),
            Statement::Insert(_) => todo!(),
            Statement::Select(select) => self.cc_select(select)?,
        };
        self.push(Vop::exit());
        Ok(self.program)
    }

    // TODO REMOVE ME
    pub fn compile_old(mut self, rql: &str) -> Result<Program> {
        self.push(Vop::init());
        {
            let mut parser = Parser::new(self.borrow_mut());
            parser.parse(rql)?;
        }
        self.push(Vop::exit());
        Ok(self.program)
    }

    pub fn cc_drop(&mut self, table: &str) {
        self.push(Vop::drop(table));
    }

    pub fn cc_delete(&mut self, table: &str) {
        self.push(Vop::clear(table));
    }

    fn cc_select(&mut self, select: &Select) -> Result<()> {
        let jmp = self.cc_from(&select.inp)?;
        let dst = self.cc_obj(&select.sel)?;
        self.return_(dst);
        self.next(jmp) // <-- loop and patch jmp
    }

    fn cc_from(&mut self, from: &From) -> Result<usize> {
        let tbl = &from.tbl;
        let var = &from.var;
        self.open(tbl, var)
    }

    fn cc_obj(&mut self, members: &Vec<Member>) -> Result<usize> {
        let dst = self.alloc(1);
        let mut mem: Vec<(String, usize)> = vec![];
        for m in members {
            let k = m.key.clone();
            let v = self.cc_rex(&m.val)?;
            mem.push((k, v));
        }
        self.push(Vop::obj(mem, dst));
        Ok(dst)
    }

    /// Compile an expression and return its destination register.
    fn cc_rex(&mut self, rex: &Rex) -> Result<usize> {
        match rex {
            Rex::Var(col) => Ok(self.var(col)),
            Rex::Lit(_) => todo!(),
            Rex::Obj(_) => todo!(),
            Rex::Jpi { .. } => todo!(),
            Rex::Jpk { .. } => todo!(),
            Rex::Spread(_) => todo!(),
        }
    }

    /// Push a `Vop::CreateTable` instruction.
    pub fn create_table(&mut self, table: Table) -> Result<()> {
        self.push(Vop::create_table(table));
        Ok(())
    }


    /// Push a `Vop::Insert` instruction.
    pub fn insert(&mut self, table: String, row: Row) -> Result<()> {
        self.push(Vop::insert(table, row));
        Ok(())
    }

    /// Push a `Vop::Open` for the given table, scan into the binding, and return the pc.
    pub fn open(&mut self, tbl: &str, var: &str) -> Result<usize> {
        self.push(Vop::open(tbl));
        self.push(Vop::rewind(0)); // <-- PATCH ME
        self.push(Vop::bind(var));
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

    /// Push a `Vop::Return` instruction.
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
    pub fn spread(&mut self) -> usize {
        let dest = self.alloc(1);
        self.push(Vop::spread(dest));
        dest
    }

    /// Push a `Vop::Obj` instruction.
    pub fn obj(&mut self, members: Vec<(String, usize)>) -> usize {
        let dest = self.alloc(1);
        self.push(Vop::obj(members, dest));
        dest
    }

    /// Push a `Vop::Var` instruction.
    pub fn var(&mut self, name: &str) -> usize {
        let dest = self.alloc(1);
        self.push(Vop::var(name, dest));
        dest
    }

    /// JSON Path Index
    pub fn json_path_index(&mut self, operand: usize, index: usize) -> usize {
        let dest = self.alloc(1);
        self.push(Vop::jpi(operand, index, dest));
        dest
    }

    /// JSON Path Key
    pub fn json_path_key(&mut self, operand: usize, key: &str) -> usize {
        let dest = self.alloc(1);
        self.push(Vop::jpk(&key, operand, dest));
        dest
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

/// Parse the RQL query into the IR.
fn parse(rql: &str) -> Result<Statement> {
    let rp = RqlParser::new();
    let ir = rp.parse(rql).unwrap();
    Ok(ir)
}
