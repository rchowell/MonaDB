use serde_json::json;

use crate::error::Error;
use crate::ir::{
    Constructor, Create, Expr, Insert, Iter, Jpe, Jpi, Jpk, Limit, Member, Obj, Op, Select, Source,
    Statement, ToSql,
};
use crate::transaction::TransactionMode;
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

/// Compiler translates SQL queries to Vops.
pub struct Compiler {
    code: Program,
    vars: Vec<Var>,
    counters: usize,
    txn: Option<TransactionMode>,
    csr: usize,
}

/// Variable bindings where [depth] represents stack position.
pub struct Var {
    pub name: String,
}

/// Track patches (instruction, offset).
type Patch = (usize, usize);

#[allow(dead_code, unused)]
impl Compiler {
    pub fn new() -> Compiler {
        Compiler {
            code: vec![],
            vars: vec![],
            counters: 0,
            txn: None,
            csr: 0,
        }
    }

    pub fn compile(mut self, statement: Statement) -> Result<Program> {
        // Setup Block
        //
        // addr 0: Init          -> jumps to addr N
        self.emit_init(0);

        // Body Block
        //
        // addr 1: [body start]
        // ...
        // addr M:      Halt            -> end of body
        #[allow(clippy::single_match_else)]
        match statement {
            Statement::Create(create) => self.cc_create(create),
            // Statement::Delete(delete) => self.cc_delete(delete)?,
            // Statement::Drop(drop) => self.cc_drop(drop),
            // Statement::Insert(insert) => self.cc_insert(insert)?,
            Statement::Select(select) => self.cc_select(select)?,
            _ => unsupported!("statement not supported: {:?}", statement),
        };
        self.emit_halt();

        // Transaction Block
        //
        // addr N:      Transaction     -> opens the transaction, falls through
        // addr N+1:    Jump 1          -> jumps back to body start
        if let Some(txn) = self.txn {
            self.emit_transaction(txn);
            self.patch(0, self.pc())?;
            self.emit_jump(1);
        }

        Ok(self.code)
    }

    fn cc_create(&mut self, create: Create) {
        self.txn = Some(TransactionMode::Rw);

        // Open the system 'catalog' table
        let csr = self.next_cursor();
        let tbl = "catalog".to_string();
        self.emit_open(csr, tbl);

        // Determine the next oid before creating the btree.
        self.emit_new_oid(csr);
        self.emit_new_btree();

        // Create the record for insertion, NewBtree only peeks the stack, oid is on top.
        //
        // No need to do any key encoding work, already on the stack. Just need to push
        // the value. The insert will encode both the key and value for now. In the near
        // future (to support multiple keys) we will need the appropriate shred instructions
        // and likely an EncodeKey(n) and EncodeVal instructions prior to insert. We do
        // not need this yet since we only have single oid keys and just encode within the
        // insert operation handler.
        //
        // Simplified where 'oid' is already top of stack from the NewOid instruction.
        //
        // addr 0:  Push    { val }     -> push unencoded value {}
        // addr 1:  Insert  { csr }     -> val=encode(pop()); key=encode(pop()); cursor.put(key, val)
        //
        // Later I'll change it to be more like:
        //
        // - value is top of stack
        // - extract key 0..N pushing all to stack
        // - encode key(N), pop N and key encode
        // - encode val, pop and value encode
        // - stack is now encoded [value, key]
        // - insert does val=pop(), key=pop(), insert(key, val).
        //
        let Create::Table(tbl) = &create;
        let val = json!({
            "name": tbl.name,
            "type": "table",
            "sql": create.sql(),
        });
        self.emit_push(val);
        self.emit_insert(csr);
    }

    fn cc_drop(&mut self, table: String) {
        self.emit_drop(table);
    }

    fn cc_delete(&mut self, table: String) -> Result<()> {
        unsupported!("delete")
    }

    fn cc_insert(&mut self, insert: Insert) -> Result<()> {
        // let n = insert.source.len();
        // for v in insert.source {
        //     self.cc_expr(v)?;
        // }
        // match n {
        //     0 => unsupported!("insert with no values"),
        //     1 => self.emit_insert(insert.target),
        //     _ => unsupported!("insert with multiple values"),
        // }
        // Ok(())
        unsupported!("insert")
    }

    fn cc_select(&mut self, select: Select) -> Result<()> {
        // TODO: track current scope
        // let scope = self.vars.len();
        // let counters = self.counters;
        // let mut to_patch: Vec<Patch> = vec![];

        // TODO: initialize counters before the loop
        // let mut cnt_skip: Option<usize> = None;
        // let mut cnt_take: Option<usize> = None;
        // if let Some(limit) = &select.limit {
        //     match limit {
        //         Limit::Skip(n) => cnt_skip = self.define_counter(*n).into(),
        //         Limit::Take(n) => cnt_take = self.define_counter(*n).into(),
        //         Limit::Slice(n, m) => {
        //             cnt_skip = self.define_counter(*n).into();
        //             cnt_take = self.define_counter(*m).into();
        //         }
        //     }
        // }

        // loop open
        // let loop_ = self.cc_iter(select.from)?;
        // to_patch.push((loop_, 1)); // <- patch loop (rewind) to next+1

        // TODO: offset
        // if let Some(counter) = cnt_skip {
        //     self.emit_cnt_if_pos(counter, 0);
        //     to_patch.push((self.pc(), 0)); // <- patch cnt_if_pos to next
        // }

        // TODO: where
        // if let Some(where_) = select.where_ {
        //     self.cc_expr(where_)?;
        //     self.emit_if_not(0);
        //     to_patch.push((self.pc(), 0)); // <- patch if_not to next
        // }

        // select
        // self.cc_select_constructor(select.select, scope)?;

        // TODO: limit
        // if let Some(counter) = cnt_take {
        //     self.emit_cnt_if_zero(counter, 0);
        //     to_patch.push((self.pc(), 1)); // <- patch cnt_if_zero to next+1
        // }

        // loop close
        // self.emit_return(scope);
        // self.emit_next(0, loop_ + 1);
        // let next = self.pc();

        // apply patches and cleanup
        // for (pc, offset) in to_patch {
        //     self.patch(pc, next + offset)?;
        // }

        // TODO: scope tracking
        // self.vars.truncate(scope);
        // self.counters = counters;

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
        self.emit_open(0, "catalog".to_string());
        // self.emit_rewind(0, 0); // <- patch to n+1
        Ok(self.pc())
    }

    /// Patch the control-flow instruction at code[pc] to jump to dst.
    fn patch(&mut self, pc: usize, dst: usize) -> Result<()> {
        // TODO: actual error handling
        match self.code.get_mut(pc).unwrap() {
            Vop::Init { jmp }
            | Vop::Next { csr: _, jmp}
            | Vop::Scan { csr: _, jmp} => *jmp = dst,
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
            _ => return Err(Error::UnknownFunction(op.sym.clone())),
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

    /// Returns the next available cursor index
    fn next_cursor(&mut self) -> usize {
        let c = self.csr;
        self.csr += 1;
        c
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

    fn emit_drop(&mut self, table: String) {
        self.code.push(Vop::Drop { table });
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

    fn emit_halt(&mut self) {
        self.code.push(Vop::Halt);
    }

    fn emit_if_not(&mut self, jmp: usize) {
        self.code.push(Vop::IfNot(jmp));
    }

    fn emit_init(&mut self, jmp: usize) {
        self.code.push(Vop::Init { jmp });
    }

    fn emit_insert(&mut self, csr: usize) {
        self.code.push(Vop::Insert { csr });
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

    fn emit_load(&mut self, csr: usize) {
        self.code.push(Vop::Load { csr });
    }

    fn emit_jump(&mut self, jmp: usize) {
        self.code.push(Vop::Jump { jmp });
    }

    fn emit_next(&mut self, csr: usize, jmp: usize) {
        self.code.push(Vop::Next { csr, jmp });
    }

    fn emit_new_oid(&mut self, csr: usize) -> usize {
        self.code.push(Vop::NewOid { csr });
        self.code.len()
    }

    fn emit_new_btree(&mut self) -> usize {
        self.code.push(Vop::NewBtree);
        self.code.len()
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

    fn emit_open(&mut self, csr: usize, tbl: String) -> usize {
        self.code.push(Vop::Open { csr, tbl });
        self.code.len()
    }

    fn emit_scan(&mut self, csr: usize, jmp: usize) -> usize {
        self.code.push(Vop::Scan { csr, jmp });
        self.code.len()
    }

    fn emit_push<V: Into<Value>>(&mut self, val: V) {
        self.code.push(Vop::Push { val: val.into() });
    }

    fn emit_return(&mut self) {
        self.code.push(Vop::Return);
    }

    fn emit_transaction(&mut self, txn: TransactionMode) {
        self.code.push(Vop::Transaction { txn });
    }
}
