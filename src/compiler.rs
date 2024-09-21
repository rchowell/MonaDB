use std::vec;
use crate::catalog::Catalog;
use crate::lexer::Lexer;
use crate::parser::RqlParser;
use crate::value::JValue;
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
    ptr: usize, // <- next register
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
        match parse(rql)? {
            Statement::Create(create) => self.cc_create(create)?,
            Statement::Delete(delete) => self.cc_delete(delete),
            Statement::Drop(drop) => self.cc_drop(drop),
            Statement::Insert(insert) => self.cc_insert(insert)?,
            Statement::Select(select) => self.cc_select(select)?,
        };
        self.push(Vop::exit());
        Ok(self.program)
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

    fn cc_create(&mut self, create: Create) -> Result<()> {
        match create {
            Create::Table(table) => self.push(Vop::create_table(table))
        }
        Ok(())
    }

    fn cc_drop(&mut self, table: String) {
        self.push(Vop::drop(table));
    }

    fn cc_delete(&mut self, table: String) {
        self.push(Vop::clear(table));
    }

    fn cc_insert(&mut self, insert: Insert) -> Result<()> {
        self.push(Vop::Transaction);
        for obj in insert.source {
            let tbl = insert.target.clone();
            let row = self.cc_expr_obj(obj)?;
            self.push(Vop::insert(tbl, row));
        }
        self.push(Vop::Commit);
        Ok(())
    }

    fn cc_select(&mut self, select: Select) -> Result<()> {
        let jmp = self.cc_from(select.inp)?;
        let dst = self.cc_expr_obj(select.sel)?;
        self.return_(dst);
        self.next(jmp) // <-- loop and patch jmp
    }

    fn cc_from(&mut self, from: From) -> Result<usize> {
        let tbl = from.tbl;
        let var = from.var;
        self.open(tbl, var)
    }

    /// Push a `Vop::Open` for the given table, scan into the binding, and return the pc.
    fn open(&mut self, table: String, var: String) -> Result<usize> {
        self.push(Vop::open(table));
        self.push(Vop::rewind(0)); // <-- PATCH ME
        self.push(Vop::bind(var));
        Ok(self.pc())
    }

    /// Loop
    ///
    /// 1. Emit a `Vop::Next` with jmp to start of loop.
    /// 2. Patch the rewind instruction BEFORE the loop.
    ///
    fn next(&mut self, jmp: usize) -> Result<()> {
        self.push(Vop::next(jmp));
        self.patch(jmp - 1, self.pc() + 1)?;
        Ok(())
    }

    /// Push a `Vop::Return` instruction.
    fn return_(&mut self, ptr: usize) {
        self.push(Vop::Return { ptr });
    }

    /// Patch the jump at pc[offset] = dest with the current pc.
    fn patch(&mut self, offset: usize, dest: usize) -> Result<()> {
        match self.program.get_mut(offset).unwrap() {
            Vop::Rewind { jmp } => *jmp = dest,
            _ => unsupported!("cannot patch jump at pc[{}]", offset),
        }
        Ok(())
    }

    //------------------------------
    // EXPRESSIONS
    //------------------------------

    fn cc_expr(&mut self, expr: Expr) -> Result<usize> {
        match expr {
            Expr::Var(var) => self.cc_expr_var(var),
            Expr::Val(val) => self.cc_expr_val(val),
            Expr::Obj(obj) => self.cc_expr_obj(obj),
            Expr::Jpi(jpi) => self.cc_expr_jpi(jpi),
            Expr::Jpk(jpk) => self.cc_expr_jpk(jpk),
            Expr::Spread(_) => todo!(),
        }
    }

    fn cc_expr_var(&mut self, var: String) -> Result<usize> {
        let dst = self.alloc(1);
        self.push(Vop::var(var, dst));
        Ok(dst)
    }

    fn cc_expr_val(&mut self, val: JValue) -> Result<usize> {
        let dst = self.alloc(1);
        self.push(Vop::val(val, dst));
        Ok(dst)
    }

    fn cc_expr_obj(&mut self, members: Obj) -> Result<usize> {
        let dst = self.alloc(1);
        let mut mem: Vec<(String, usize)> = vec![];
        for m in members {
            let k = m.key.clone();
            let v = self.cc_expr(m.val)?;
            mem.push((k, v));
        }
        self.push(Vop::obj(mem, dst));
        Ok(dst)
    }

    fn cc_expr_jpi(&mut self, jpi: Jpi) -> Result<usize> {
        let inp = self.cc_expr(*jpi.inp)?;
        let idx = jpi.idx;
        let dst = self.alloc(1);
        self.push(Vop::jpi(inp, idx, dst));
        Ok(dst)
    }

    fn cc_expr_jpk(&mut self, jpk: Jpk) -> Result<usize> {
        let inp = self.cc_expr(*jpk.inp)?;
        let key = jpk.key;
        let dst = self.alloc(1);
        self.push(Vop::jpk(inp, key, dst));
        Ok(dst)
    }
}

/// Parse the RQL query into the IR.
fn parse(rql: &str) -> Result<Statement> {
    let rl = Lexer::new(rql);
    let rp = RqlParser::new();
    let ir = rp.parse(rl)?;
    Ok(ir)
}
