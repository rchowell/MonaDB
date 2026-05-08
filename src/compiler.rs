use crate::error::err_unknown_routine;
use crate::ir::*;
use crate::lexer::RqlLexer;
use crate::parser::RqlParser;
use crate::value::Value;
use crate::{Program, Result, Vop};
use std::vec;

#[macro_export]
macro_rules! unsupported {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return Err($crate::error::Error::Unsupported(msg.to_string()))
    }}
}

/// Compiler translates RQL queries to Vops.
pub struct Compiler {
    code: Program,
    vars: Vec<Var>,
    counters: usize,
}

/// Variable bindings where [depth] represents stack position.
pub struct Var {
    pub name: String,
}

/// Track patches (instruction, offset).
type Patch = (usize, usize);

impl Compiler  {
    pub fn new() -> Compiler {
        Compiler {
            code: vec![],
            vars: vec![],
            counters: 0,
        }
    }

    pub fn compile(mut self, rql: &str) -> Result<Program> {
        self.emit_init();
        match parse(rql)? {
            Statement::Create(create) => self.cc_create(create)?,
            Statement::Delete(delete) => self.cc_delete(delete),
            Statement::Drop(drop) => self.cc_drop(drop),
            Statement::Insert(insert) => self.cc_insert(insert)?,
            Statement::Select(select) => self.cc_select(select)?,
        };
        self.emit_exit();
        Ok(self.code)
    }

    fn cc_create(&mut self, create: Create) -> Result<()> {
        match create {
            Create::Table(table) => self.emit_create_table(table),
        }
        Ok(())
    }

    fn cc_drop(&mut self, table: String) {
        self.emit_drop(table);
    }

    fn cc_delete(&mut self, table: String) {
        self.emit_clear(table);
    }

    fn cc_insert(&mut self, insert: Insert) -> Result<()> {
        let n = insert.source.len();
        for v in insert.source {
            self.cc_expr(v)?;
        }
        match n {
            0 => unsupported!("insert with no values"),
            1 => self.emit_insert(insert.target),
            n => self.emit_insert_batch(insert.target, n),
        }
        Ok(())
    }

    fn cc_select(&mut self, select: Select) -> Result<()> {

        // track current scope
        let scope = self.vars.len();
        let counters = self.counters;
        let mut to_patch: Vec<Patch> = vec![];

        // initialize counters before the loop
        let mut cnt_skip: Option<usize> = None;
        let mut cnt_take: Option<usize> = None;
        if let Some(fetch) = &select.fetch {
            match fetch {
                Limit::Skip(n) => cnt_skip = self.define_counter(*n).into(),
                Limit::Take(n) => cnt_take = self.define_counter(*n).into(),
                Limit::Slice(n, m) => {
                    cnt_skip = self.define_counter(*n).into();
                    cnt_take = self.define_counter(*m).into();
                },
            }
        }

        // loop open
        let loop_ = self.cc_iter(select.from)?;
        to_patch.push((loop_, 1)); // <- patch loop (rewind) to next+1

        // skip (offset)
        if let Some(counter) = cnt_skip {
            self.emit_cnt_if_pos(counter, 0);
            to_patch.push((self.pc(), 0)); // <- patch cnt_if_pos to next
        }

        // where
        if let Some(where_) = select.where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            to_patch.push((self.pc(), 0)); // <- patch if_not to next
        }
        self.cc_select_constructor(select.select, scope)?;

        // take (limit)
        if let Some(counter) = cnt_take {
            self.emit_cnt_if_zero(counter, 0);
            to_patch.push((self.pc(), 1)); // <- patch cnt_if_zero to next+1
        }

        // loop close
        self.emit_return(scope);
        self.emit_next(0, loop_ + 1);
        let next = self.pc();

        // apply patches and cleanup
        for (pc, offset) in to_patch {
            self.patch(pc, next + offset)?;
        }
        self.vars.truncate(scope);
        self.counters = counters;

        Ok(())
    }

    fn cc_select_constructor(&mut self, constructor: Constructor, sos: usize) -> Result<()> {
        match constructor {
            Constructor::None => (),
            Constructor::Star => {
                self.emit_obj();
                let mut i = sos; // start-of-scope
                let n = self.vars.len();
                while i < n {
                    self.emit_load(i);
                    self.emit_obj_spread();
                    i += 1;
                }
            }
            Constructor::Expr(expr) => self.cc_expr(expr)?,
            Constructor::List(members) => self.cc_expr_obj(members)?,
        }
        Ok(())
    }

    /// Compile an iteration operator, callers must patch the return pc.
    ///
    /// | ADDR | INSTRUCTION | NOTE
    /// +------+-------------+-----------
    /// | 0    | open        |
    /// | 1    | rewind      | if empty, jump to n+1
    /// | 2    |  (loop)     |
    /// | ..   |             |
    /// | n    | next        | if next, jump to 2
    /// | n+1  | ...         |   
    ///
    /// TODO handle paths
    /// TODO multiple cursors
    ///
    fn cc_iter(&mut self, iter: Iter) -> Result<usize> {
        let table = match iter.src {
            Source::Table(table) => table,
            Source::Path(path) => unsupported!("from path: {:?}", path),
            Source::Value(_) => unsupported!("from value"),
        };
        // define the iteration variable
        self.define(iter.var);
        self.emit_open(table);
        self.emit_rewind(0, 0); // <- patch to n+1
        Ok(self.pc())
    }

    /// Patch the control-flow instruction at code[pc] to jump to dst.
    fn patch(&mut self, pc: usize, dst: usize) -> Result<()> {
        match self.code.get_mut(pc).unwrap() {
            Vop::CntIfPos(_, jmp) => *jmp = dst,
            Vop::CntIfZero(_, jmp) => *jmp = dst,
            Vop::If(jmp) => *jmp = dst,
            Vop::IfNot(jmp) => *jmp = dst,
            Vop::Next(_, jmp) => *jmp = dst,
            Vop::Rewind(_, jmp) => *jmp = dst,
            _ => unsupported!("cannot patch instruction at pc[{}]", pc),
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
            "*" => self.code.push(Vop::Mul),
            "/" => self.code.push(Vop::Div),
            "+" => self.code.push(Vop::Add),
            "-" => self.code.push(Vop::Sub),
            "<" => self.code.push(Vop::Lt),
            "<=" => self.code.push(Vop::Le),
            "=" => self.code.push(Vop::Eq),
            ">=" => self.code.push(Vop::Ge),
            ">" => self.code.push(Vop::Gt),
            "!=" => self.code.push(Vop::Ne),
            _ => return Err(err_unknown_routine(op.sym.as_str())),
        };
        Ok(())
    }

    fn cc_expr_jpe(&mut self, jpe: Jpe) -> Result<()> {
        self.cc_expr(*jpe.inp)?;
        self.cc_expr(*jpe.exp)?;
        self.emit_jpe();
        Ok(())
    }

    fn cc_expr_jpi(&mut self, jpi: Jpi) -> Result<()> {
        self.cc_expr(*jpi.inp)?;
        self.emit_jpi(jpi.idx);
        Ok(())
    }

    fn cc_expr_jpk(&mut self, jpk: Jpk) -> Result<()> {
        self.cc_expr(*jpk.inp)?;
        self.emit_jpk(jpk.key);
        Ok(())
    }

    fn cc_expr_lit(&mut self, value: Value) -> Result<()> {
        self.emit_push(value);
        Ok(())
    }

    fn cc_expr_obj(&mut self, obj: Obj) -> Result<()> {
        self.emit_obj();
        for m in obj {
            match m {
                Member::Assign(name, expr) => {
                    self.cc_expr(expr)?;
                    self.emit_obj_assign(name);
                }
                Member::Spread(expr) => {
                    self.cc_expr(expr)?;
                    self.emit_obj_spread();
                }
            }
        }
        Ok(())
    }

    fn cc_expr_var(&mut self, name: String) -> Result<()> {
        for (idx, var) in self.vars.iter().enumerate() {
            if var.name == name {
                self.emit_load(idx);
                return Ok(());
            }
        }
        unsupported!("undefined variable: {}", name)
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

    /// Define a counter with the given value.
    fn define_counter(&mut self, n: u64) -> usize {
        let c = self.counters;
        self.emit_cnt_set(c, n);
        self.counters += 1;
        c
    }

    //------------------------------
    // INSTRUCTIONS
    //------------------------------

    fn emit_clear(&mut self, table: String) {
        self.code.push(Vop::Clear { table })
    }

    fn emit_create_table(&mut self, table: Table) {
        self.code.push(Vop::CreateTable { table })
    }

    fn emit_drop(&mut self, table: String) {
        self.code.push(Vop::Drop { table })
    }

    fn emit_exit(&mut self) {
        self.code.push(Vop::Exit)
    }

    fn emit_cnt_set(&mut self, counter: usize, value: u64) {
        self.code.push(Vop::CntSet(counter, value));
    }

    fn emit_cnt_if_pos(&mut self, counter: usize, jmp: usize) {
        self.code.push(Vop::CntIfPos(counter, jmp));
    }

    fn emit_cnt_if_zero(&mut self, counter: usize, jmp: usize) {
        self.code.push(Vop::CntIfZero(counter, jmp));
    }

    fn emit_if_not(&mut self, jmp: usize) {
        self.code.push(Vop::IfNot(jmp))
    }

    fn emit_init(&mut self) {
        self.code.push(Vop::Init)
    }

    fn emit_insert(&mut self, table: String) {
        self.code.push(Vop::Insert(table))
    }

    fn emit_insert_batch(&mut self, table: String, n: usize) {
        self.code.push(Vop::InsertBatch(table, n))
    }

    fn emit_jpe(&mut self) {
        self.code.push(Vop::Jpe);
    }

    fn emit_jpi(&mut self, idx: usize) {
        self.code.push(Vop::Jpi(idx));
    }

    fn emit_jpk(&mut self, key: String) {
        self.code.push(Vop::Jpk(key));
    }

    fn emit_load(&mut self, idx: usize) {
        self.code.push(Vop::Load(idx))
    }

    fn emit_next(&mut self, cursor: usize, jmp: usize) {
        self.code.push(Vop::Next(cursor, jmp));
    }

    fn emit_obj(&mut self) {
        self.code.push(Vop::Obj);
    }

    fn emit_obj_assign(&mut self, name: String) {
        self.code.push(Vop::ObjAssign(name));
    }

    fn emit_obj_spread(&mut self) {
        self.code.push(Vop::ObjSpread);
    }

    fn emit_open(&mut self, table: String) {
        self.code.push(Vop::Open(table));
    }

    fn emit_push(&mut self, value: Value) {
        self.code.push(Vop::Push(value))
    }

    fn emit_rewind(&mut self, cursor: usize, jmp: usize) {
        self.code.push(Vop::Rewind(cursor, jmp));
    }

    fn emit_return(&mut self, tofs: usize) {
        self.code.push(Vop::Return(tofs));
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
