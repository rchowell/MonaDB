use crate::catalog::Catalog;
use crate::error::err_unknown_routine;
use crate::ir::*;
use crate::lexer::RqlLexer;
use crate::parser::RqlParser;
use crate::value::Value;
use crate::{Code, Result, Vop};
use std::vec;

#[macro_export]
macro_rules! unsupported {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return Err(crate::error::Error::Unsupported(msg.to_string()))
    }}
}

/// Compiler translates RQL queries to Vops.
pub struct Compiler<'cat> {
    catalog: &'cat Catalog,
    code: Code,
    vars: Vec<Var>,
}

/// Variable bindings where [depth] represents stack position.
pub struct Var {
    pub name: String,
}

impl<'cat> Compiler<'cat> {
    pub fn new(catalog: &Catalog) -> Compiler {
        Compiler {
            catalog,
            code: vec![],
            vars: vec![],
        }
    }

    pub fn compile(mut self, rql: &str) -> Result<Code> {
        self.op_init();
        match parse(rql)? {
            Statement::Create(create) => self.cc_create(create)?,
            Statement::Delete(delete) => self.cc_delete(delete),
            Statement::Drop(drop) => self.cc_drop(drop),
            Statement::Insert(insert) => self.cc_insert(insert)?,
            Statement::Select(select) => self.cc_select(select)?,
        };
        self.op_exit();
        Ok(self.code)
    }

    /// Append an instruction to the program.
    /// TODO REMOVE ME
    #[inline]
    fn emit(&mut self, op: Vop) {
        self.code.push(op);
    }

    fn cc_create(&mut self, create: Create) -> Result<()> {
        match create {
            Create::Table(table) => self.emit(Vop::create_table(table)),
        }
        Ok(())
    }

    fn cc_drop(&mut self, table: String) {
        self.emit(Vop::drop(table));
    }

    fn cc_delete(&mut self, table: String) {
        self.emit(Vop::clear(table));
    }

    fn cc_insert(&mut self, insert: Insert) -> Result<()> {
        self.emit(Vop::Transaction);
        for _ in insert.source {
            // let tbl = insert.target.clone();
            // let dst = self.cc_expr(expr)?;
            unsupported!("`insert` statement")
        }
        self.emit(Vop::Commit);
        Ok(())
    }

    fn cc_select(&mut self, select: Select) -> Result<()> {
        let jmp = self.cc_from(select.inp)?;
        let _ = self.cc_expr_obj(select.sel)?;
        self.op_return();
        self.next(jmp) // <-- loop and patch jmp
    }

    /// Compile a `from` clause.
    ///
    /// 1. Define the `from` alias aka the new variable.
    /// 2. Open the table as a new cursor and rewind.
    /// 3. Emit a `push` for this cursor.
    ///
    fn cc_from(&mut self, from: From) -> Result<usize> {
        // TODO handle paths
        let table = match from.src {
            FromSource::Table(table) => table,
            FromSource::Path(path) => todo!("from path: {:?}", path),
        };

        // define the from alias
        self.define(from.var);
        self.emit(Vop::open(table));
        self.emit(Vop::rewind(0)); // <-- PATCH ME
        self.op_loadc(0); // TODO multiple cursors

        Ok(self.pc())
    }

    /// Loop
    ///
    /// 1. Emit a `Vop::Next` with jmp to start of loop.
    /// 2. Patch the rewind instruction BEFORE the loop.
    ///
    fn next(&mut self, jmp: usize) -> Result<()> {
        self.emit(Vop::next(jmp));
        self.patch(jmp - 1, self.pc() + 1)?;
        Ok(())
    }

    /// Patch the jump at pc[offset] = dest with the current pc.
    fn patch(&mut self, offset: usize, dest: usize) -> Result<()> {
        match self.code.get_mut(offset).unwrap() {
            Vop::Rewind { jmp } => *jmp = dest,
            _ => unsupported!("cannot patch jump at pc[{}]", offset),
        }
        Ok(())
    }

    //------------------------------
    // EXPRESSIONS
    //------------------------------

    fn cc_expr(&mut self, expr: Expr) -> Result<()> {
        match expr {
            Expr::Jpe(jpe) => self.cc_expr_jpe(jpe),
            Expr::Jpi(jpi) => self.cc_expr_jpi(jpi),
            Expr::Jpk(jpk) => self.cc_expr_jpk(jpk),
            Expr::Lit(val) => self.cc_expr_lit(val),
            Expr::Obj(obj) => self.cc_expr_obj(obj),
            Expr::Op(op) => self.cc_expr_op(op),
            Expr::Var(var) => self.cc_expr_var(var),
        }
    }

    fn cc_expr_op(&mut self, op: Op) -> Result<()> {
        self.cc_expr(*op.lhs)?;
        self.cc_expr(*op.rhs)?;
        match op.sym.as_str() {
            // prec. 0
            "*" => todo!("mul"),
            "/" => todo!("div"),
            // prec. 1
            "+" => todo!("add"),
            "-" => todo!("sub"),
            // prec. 2
            "<" => todo!("lt"),
            "<=" => todo!("le"),
            "=" => todo!("eq"),
            ">=" => todo!("ge"),
            ">" => todo!("gt"),
            _ => return Err(err_unknown_routine(op.sym.as_str()))
        }
        // Ok(())
    }

    fn cc_expr_jpe(&mut self, jpe: Jpe) -> Result<()> {
        self.cc_expr(*jpe.inp)?;
        self.cc_expr(*jpe.exp)?;
        self.op_jpe();
        Ok(())
    }

    fn cc_expr_jpi(&mut self, jpi: Jpi) -> Result<()> {
        self.cc_expr(*jpi.inp)?;
        self.op_jpi(jpi.idx);
        Ok(())
    }

    fn cc_expr_jpk(&mut self, jpk: Jpk) -> Result<()> {
        self.cc_expr(*jpk.inp)?;
        self.op_jpk(jpk.key);
        Ok(())
    }

    fn cc_expr_lit(&mut self, value: Value) -> Result<()> {
        self.op_push(value);
        Ok(())
    }

    fn cc_expr_obj(&mut self, obj: Obj) -> Result<()> {
        self.op_obj();
        for m in obj {
            match m {
                Member::Assign(name, expr) => {
                    self.cc_expr(expr)?;
                    self.op_obj_assign(name);
                }
                Member::Spread(expr) => {
                    self.cc_expr(expr)?;
                    self.op_obj_spread();
                }
            }
        }
        Ok(())
    }

    fn cc_expr_var(&mut self, name: String) -> Result<()> {
        for (idx, var) in self.vars.iter().enumerate() {
            if var.name == name {
                self.op_loadv(idx);
                return Ok(());
            }
        }
        unsupported!("undefined variable: {}", name)
    }

    //------------------------------
    // INSTRUCTIONS
    //------------------------------

    fn op_exit(&mut self) {
        self.code.push(Vop::Exit)
    }

    fn op_init(&mut self) {
        self.code.push(Vop::Init)
    }

    fn op_jpe(&mut self) {
        self.code.push(Vop::Jpe);
    }

    fn op_jpi(&mut self, idx: usize) {
        self.code.push(Vop::Jpi(idx));
    }

    fn op_jpk(&mut self, key: String) {
        self.code.push(Vop::Jpk(key));
    }

    fn op_loadc(&mut self, cursor: usize) {
        self.code.push(Vop::LoadC(cursor))
    }

    fn op_loadv(&mut self, idx: usize) {
        self.code.push(Vop::LoadV(idx))
    }

    fn op_obj(&mut self) {
        self.code.push(Vop::Obj);
    }

    fn op_obj_assign(&mut self, name: String) {
        self.code.push(Vop::ObjAssign { name });
    }

    fn op_obj_spread(&mut self) {
        self.code.push(Vop::ObjSpread);
    }

    fn op_push(&mut self, value: Value) {
        self.code.push(Vop::Push(value))
    }

    fn op_return(&mut self) {
        self.code.push(Vop::Return);
    }

    //------------------------------
    // HELPERS
    //------------------------------

    /// Return current pc index.
    fn pc(&self) -> usize {
        self.code.len() - 1
    }

    /// Define a variable in the current scope.
    fn define(&mut self, name: String) {
        self.vars.push(Var { name });
    }
}

/// TODO why is this here?
/// Parse the RQL query into the IR.
fn parse(rql: &str) -> Result<Statement> {
    let rl = RqlLexer::new(rql);
    let rp = RqlParser::new();
    let ir = rp.parse(rl)?;
    Ok(ir)
}
