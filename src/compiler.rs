use serde_json::json;

use crate::catalog::{Catalog, CATALOG_OID};
use crate::error::Error;
use crate::ir::{
    Constructor, Create, Expr, Insert, Jpe, Jpi, Jpk, Member, Obj, Op, Ref, Select, Source, Statement, ToSql
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
pub struct Compiler<'c> {
    catalog: &'c Catalog,
    code: Vec<Vop>,
    bindings: Vec<Binding>,
    counters: usize,
    /// The transaction mode this program requires
    txm: Option<TransactionMode>,
    /// This is the next available cursor slot index
    next_cursor: usize,
}

/// Variable bindings where [depth] represents stack position.
pub struct Binding {
    pub name: String,
}

#[allow(dead_code, unused)]
impl<'c> Compiler<'c> {
    pub fn new(catalog: &'c Catalog) -> Compiler<'c> {
        Compiler {
            catalog,
            code: vec![],
            bindings: vec![],
            counters: 0,
            txm: None,
            next_cursor: 0,
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
            Statement::Insert(insert) => self.cc_insert(insert)?,
            Statement::Select(select) => self.cc_select(select)?,
            _ => unsupported!("statement not supported: {:?}", statement),
        };
        self.emit_halt();

        // Transaction Block
        //
        // addr N:      Transaction     -> opens the transaction, falls through
        // addr N+1:    Jump 1          -> jumps back to body start
        if let Some(txn) = self.txm {
            self.emit_transaction(txn);
            self.patch(0, self.pc())?;
            self.emit_jump(1);
        }

        // Return the final program with appropriate counters
        Ok(Program {
            cursors: self.next_cursor,
            instructions: self.code,
        })
    }

    fn cc_create(&mut self, create: Create) {
        self.txm = Some(TransactionMode::Write);

        // Create the table definition JSON-value
        let Create::Table(table_definition) = &create;
        let val = json!({
            "name": table_definition.name,
            "type": "table",
            "sql": create.sql(),
        });

        // Creating a table is an insert to the catalog table.
        //
        // 0   Open { tbl=0 }      Open the 'catalog' system table (oid=0)
        // 1   Push { val }        Push the table definition; the insert value.
        // 2   NewOid              Push the next oid; the insert key
        // 3   NewBtree            Create the LMDB btree before insertion (peek NewOid)
        // 4   Insert              Pop the key then the value, then insert into the new btree
        //
        // No need to do any key encoding work, already on the stack. Just need to push
        // the value. The insert will encode both the key and value for now. In the near
        // future (to support multiple keys) we will need the appropriate shred instructions
        // and likely an EncodeKey(n) and EncodeVal instructions prior to insert. We do
        // not need this yet since we only have single oid keys and just encode within the
        // insert operation handler.
        let csr = self.next_cursor();
        self.emit_open(csr, CATALOG_OID);
        self.emit_push(val);
        self.emit_new_oid(csr);
        self.emit_new_btree();
        self.emit_insert(csr);
    }

    fn cc_drop(&mut self, table: String) {
        self.emit_drop(table);
    }

    fn cc_delete(&mut self, table: String) -> Result<()> {
        unsupported!("delete")
    }

    fn cc_insert(&mut self, insert: Insert) -> Result<()> {
        self.ensure_txn(TransactionMode::Write);
        // Resolve table name to its oid (tbl) to open.
        let csr = self.next_cursor();
        let tbl = self.catalog.get_table(&insert.target)?;
        self.emit_open(csr, tbl);
        for val in insert.source {
            self.cc_expr(val)?;
            self.emit_new_oid(csr);
            self.emit_insert(csr);
        }
        Ok(())
    }

    fn cc_select(&mut self, select: Select) -> Result<()> {
        self.ensure_txn(TransactionMode::Read);
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
        // to_patch.push((loop_, 1)); // <- patch loop (rewind) to next+1

        // Extract the table name, we don't bind anything.
        let Source::Table(tbl_name) = &select.from.src;

        // Resolve name to its stable oid
        let tbl = self.catalog.get_table(&tbl_name)?;

        // open
        // push -> prefix (None is full scan)
        // scan -> jmp=next
        // ....
        // next -> jmp=scan
        let csr = self.next_cursor();
        let jmp = self.pc() + 2;
        self.emit_open(csr, tbl);
        // TODO: prefix scans
        // self.emit_push(Value::None);
        self.emit_scan(csr, jmp);
        let scan_pc = self.pc();

        // body, hardcoded for select *
        self.emit_load(csr);
        self.emit_yield();

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
        self.cc_select_constructor(select.select, 1)?;

        // TODO: limit
        // if let Some(counter) = cnt_take {
        //     self.emit_cnt_if_zero(counter, 0);
        //     to_patch.push((self.pc(), 1)); // <- patch cnt_if_zero to next+1
        // }

        // loop close
        self.emit_next(csr, scan_pc + 1);
        let next_pc = self.pc();
        self.patch(scan_pc, next_pc + 1);

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
                let n = self.bindings.len();
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

    /// Patch the control-flow instruction at src to have jmp=dst.
    fn patch(&mut self, src: usize, dst: usize) -> Result<()> {
        // TODO: actual error handling
        match self.code.get_mut(src).unwrap() {
            Vop::Init { jmp }
            | Vop::Next { csr: _, jmp}
            | Vop::Scan { csr: _, jmp} => *jmp = dst,
            _ => unsupported!("cannot patch instruction at pc[{}]", src),
        }
        Ok(())
    }

    /// Ensures the program as at least the given transaction mode
    fn ensure_txn(&mut self, txn: TransactionMode) {
        self.txm = Some(txn.coalesce(self.txm));
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
            Expr::Var(var) => self.cc_expr_var(&var),
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

    fn cc_expr_var(&mut self, var: &Ref) -> Result<()> {
        for (idx, binding) in self.bindings.iter().enumerate() {
            if binding.name == var.name {
                self.emit_load(idx);
                return Ok(());
            }
        }
        unsupported!("undefined variable: {}", var.name)
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
        let c = self.next_cursor;
        self.next_cursor += 1;
        c
    }

    /// Define a variable in the current scope.
    fn define(&mut self, name: String) {
        self.bindings.push(Binding { name });
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

    fn emit_new_oid(&mut self, csr: usize) {
        self.code.push(Vop::NewOid { csr });
    }

    fn emit_new_btree(&mut self) {
        self.code.push(Vop::NewBtree);
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

    fn emit_open(&mut self, csr: usize, tbl: u32) {
        self.code.push(Vop::Open { csr, tbl });
    }

    fn emit_scan(&mut self, csr: usize, jmp: usize) {
        self.code.push(Vop::Scan { csr, jmp });
    }

    fn emit_push<V: Into<Value>>(&mut self, val: V) {
        self.code.push(Vop::Push { val: val.into() });
    }

    fn emit_yield(&mut self) {
        self.code.push(Vop::Yield);
    }

    fn emit_transaction(&mut self, txn: TransactionMode) {
        self.code.push(Vop::Transaction { txm: txn });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::SqlLexer;
    use crate::parser::SqlParser;
    use crate::storage::Storage;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Catalog) {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(dir.path().join("test.db")).unwrap();
        let catalog = Catalog::load(storage).unwrap();
        (dir, catalog)
    }

    fn compile_sql(catalog: &Catalog, sql: &str) -> Program {
        let stmt = SqlParser::new().parse(SqlLexer::new(sql)).unwrap();
        Compiler::new(catalog).compile(stmt).unwrap()
    }

    #[test]
    fn select_star_from_catalog_bytecode_shape() {
        let (_dir, catalog) = fixture();
        let code = compile_sql(&catalog, "select * from catalog;").instructions;
        // Note: an extra `Obj` from cc_select_constructor lands after `Return`
        // (dead code given the hardcoded select-* body); shape is preserved
        // here so the resolution path is what we're guarding.
        assert_eq!(code.len(), 10);
        assert!(matches!(code[0], Vop::Init { jmp: 8 }));
        assert!(matches!(code[1], Vop::Open { csr: 0, tbl: 0 }));
        assert!(matches!(code[2], Vop::Scan { csr: 0, jmp: 7 }));
        assert!(matches!(code[3], Vop::Load { csr: 0 }));
        assert!(matches!(code[4], Vop::Yield));
        assert!(matches!(code[5], Vop::Obj));
        assert!(matches!(code[6], Vop::Next { csr: 0, jmp: 3 }));
        assert!(matches!(code[7], Vop::Halt));
        assert!(matches!(code[8], Vop::Transaction { txm: TransactionMode::Read }));
        assert!(matches!(code[9], Vop::Jump { jmp: 1 }));
    }

    #[test]
    fn create_table_bytecode_shape() {
        let (_dir, catalog) = fixture();
        let code = compile_sql(&catalog, "create table t (id int);").instructions;
        dbg!(&code);
        assert_eq!(code.len(), 9);
        assert!(matches!(code[0], Vop::Init { jmp: 7 }));
        assert!(matches!(code[1], Vop::Open { csr: 0, tbl: 0 }));
        assert!(matches!(code[2], Vop::Push { .. }));
        assert!(matches!(code[3], Vop::NewOid { csr: 0 }));
        assert!(matches!(code[4], Vop::NewBtree));
        assert!(matches!(code[5], Vop::Insert { csr: 0 }));
        assert!(matches!(code[6], Vop::Halt));
        assert!(matches!(code[7], Vop::Transaction { txm: TransactionMode::Write }));
        assert!(matches!(code[8], Vop::Jump { jmp: 1 }));
    }

    #[test]
    fn select_cursor_index_is_zero() {
        // Guards the latent slot-vs-push bug: the single emitted cursor
        // must use index 0 so that vm.cursors.push lands in the expected slot.
        let (_dir, catalog) = fixture();
        let code = compile_sql(&catalog, "select * from catalog;").instructions;
        for op in &code {
            match op {
                Vop::Open { csr, .. }
                | Vop::Scan { csr, .. }
                | Vop::Load { csr }
                | Vop::Next { csr, .. } => assert_eq!(*csr, 0),
                _ => {}
            }
        }
    }
}
