use serde_json::json;

use crate::catalog::CATALOG_OID;
use crate::error::Error;
use crate::ir::{
    Call, Constructor, Create, Delete, Expr, Insert, Jpe, Jpi, Jpk, Limit, Member, Obj, Var, Select, Source, Statement, ToSql
};
use crate::transaction::TransactionMode;
use crate::value::Value;
use crate::vm::{Program, Vop};
use crate::{Result};
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
    code: Vec<Vop>,
    /// Number of cursor slots required (max index + 1).
    cursor_slots: usize,
    /// Number of counter slots required (one per allocated counter).
    counter_slots: usize,
    /// The transaction mode this program requires
    txm: Option<TransactionMode>,
}

#[allow(dead_code, unused)]
impl Compiler {
    pub fn new() -> Compiler {
        Compiler {
            code: vec![],
            cursor_slots: 0,
            counter_slots: 0,
            txm: None,
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
            Statement::Create(create) => self.cc_create(&create),
            Statement::Delete(delete) => self.cc_delete(delete)?,
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

        Ok(Program {
            cursors: self.cursor_slots,
            counters: self.counter_slots,
            instructions: self.code,
        })
    }

    fn cc_create(&mut self, create: &Create) {
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
        let csr = self.alloc_cursor();
        self.emit_open(csr, CATALOG_OID);
        self.emit_push(val);
        self.emit_new_oid(csr);
        self.emit_new_btree();
        self.emit_insert(csr);
    }

    fn cc_insert(&mut self, insert: Insert) -> Result<()> {
        self.ensure_txn(TransactionMode::Write);
        // Resolve table name to its oid (tbl) to open.
        let csr = self.alloc_cursor();
        let tbl = insert.target.bind.expect("insert target should be bound to table oid");
        self.emit_open(csr, tbl);
        for val in insert.source {
            self.cc_expr(val)?;
            self.emit_new_oid(csr);
            self.emit_insert(csr);
        }
        Ok(())
    }

    fn cc_delete(&mut self, delete: Delete) -> Result<()> {
        self.ensure_txn(TransactionMode::Write);

        let Delete { from, where_ } = delete;
        let csr = from.csr.expect("delete target should be bound") as usize;
        let oid = from.oid.expect("bind pass must set oid for Table");

        // Open the target table and begin a forward scan. The scan's jmp exits
        // the loop when the table is empty or exhausted; patched once `exit` is
        // known.
        self.emit_open(csr, oid);
        self.emit_scan(csr, 0);
        let begin = self.pc();

        // Loop body: evaluate the predicate (if any) and skip the delete when
        // it is false, otherwise mark the current row for deletion.
        let body = self.code.len();
        let mut where_fail = None;
        if let Some(where_) = where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            where_fail = Some(self.pc());
        }
        self.emit_delete(csr);

        // Advance; when more rows remain, jump back to the body.
        self.emit_next(csr, body);
        let next_pc = self.pc();

        let exit = self.pc() + 1;
        self.patch(begin, exit)?;
        if let Some(pc) = where_fail {
            self.patch(pc, next_pc)?;
        }
        Ok(())
    }

    fn cc_select(&mut self, select: Select) -> Result<()> {
        self.ensure_txn(TransactionMode::Read);

        // Initialize the limit counters before the loop.
        // 
        // Limit N..M is half-open [N, M); skip N rows, then take M - N.
        // saturating, so M <= N yields nothing. Could be a static analysis
        // error, but this is fine.
        let mut cnt_skip: Option<usize> = None;
        let mut cnt_take: Option<usize> = None;
        if let Some(limit) = &select.limit {
            let (skip, take) = match limit {
                Limit::Skip(n) => (Some(*n), None),
                Limit::Take(n) => (None, Some(*n)),
                Limit::Slice(n, m) => (Some(*n), Some(m.saturating_sub(*n))),
            };
            if let Some(n) = skip {
                let c = self.alloc_counter();
                self.emit_cnt_set(c, n);
                cnt_skip = Some(c);
            }
            if let Some(n) = take {
                let c = self.alloc_counter();
                self.emit_cnt_set(c, n);
                cnt_take = Some(c);
            }
        }

        let Select { from, where_, select: constructor, .. } = select;

        // Compile the select <value>; form. `*` and `.` project a binding
        // tuple, so they are meaningless without a from clause.
        if from.is_empty() {
            if let Constructor::Star | Constructor::None = constructor {
                unsupported!("select * / select . requires a from clause");
            }
            self.cc_select_constructor(constructor, &[])?;
            self.emit_yield();
            return Ok(());
        }

        // The (alias, cursor) bindings in item order; drive `*` and `.` forms.
        let bindings: Vec<(String, usize)> = from
            .iter()
            .map(|f| (f.var.clone(), f.csr.expect("from item should be bound") as usize))
            .collect();

        let n = bindings.len();

        // Open table sources once before the loop; value sources need no open.
        for f in &from {
            if let Source::Table(_) = &f.src {
                let csr = f.csr.expect("from item should be bound") as usize;
                let oid = f.oid.expect("bind pass must set oid for Table");
                self.emit_open(csr, oid);
            }
        }

        // Begin one iteration per source, outer to inner. The sources are:
        //
        //  1. A table source begin is a Scan.
        //  2. A value source begin is an expression + Iter.
        //
        // We enter a value source on the expression so that we evaluate
        // it again. This is critical for correlated value sources.
        //
        //  - entry[i] is the entry target for the enclosing Next instruction.
        //  - begin[i] is the exhaust instruction to patch once `exit` is known.
        //
        let mut entry = vec![0usize; n];
        let mut begin = vec![0usize; n];
        for (i, f) in from.into_iter().enumerate() {
            let csr = f.csr.expect("from item should be bound") as usize;
            match f.src {
                Source::Table(_) => {
                    // FROM <table>
                    self.emit_scan(csr, 0);
                    entry[i] = self.pc();
                }
                Source::Value(expr) => {
                    // FROM <expression>
                    entry[i] = self.pc() + 1;
                    self.cc_expr(*expr)?;
                    self.emit_iter(csr, 0);
                }
            }
            begin[i] = self.pc();
        }

        // Innermost body: predicate filter, then offset/limit, then projection.
        let body = self.code.len();
        let mut where_fail = None;
        if let Some(where_) = where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            where_fail = Some(self.pc());
        }

        // Compile the offset (skip)
        let mut offset = None;
        if let Some(c) = cnt_skip {
            self.emit_cnt_if_pos(c, 0);
            offset = Some(self.pc());
        }

        // Compile the limit (take)
        let mut limit_pc = None;
        if let Some(c) = cnt_take {
            self.emit_cnt_if_zero(c, 0);
            limit_pc = Some(self.pc());
        }

        // Compile the 'select' projection (constructor).
        self.cc_select_constructor(constructor, &bindings)?;
        self.emit_yield();

        // Close the loops inner to outer. When source i advances, resume the
        // next inner source's begin block (re-evaluating a value expr), or the
        // body if i is innermost.
        let mut next_pc = vec![0usize; n];
        for i in (0..n).rev() {
            let resume = if i + 1 < n { entry[i + 1] } else { body };
            self.emit_next(bindings[i].1, resume);
            next_pc[i] = self.pc();
        }

        // Patch exhaust edges: the outermost source exits the query; an inner
        // source that exhausts (or yields nothing) advances its enclosing one.
        let exit = self.pc() + 1;
        self.patch(begin[0], exit)?;
        for i in 1..n {
            self.patch(begin[i], next_pc[i - 1])?;
        }
        let inner = next_pc[n - 1];
        if let Some(pc) = where_fail {
            self.patch(pc, inner)?;
        }
        if let Some(pc) = offset {
            self.patch(pc, inner)?;
        }
        if let Some(pc) = limit_pc {
            self.patch(pc, exit)?;
        }
        Ok(())
    }

    fn cc_select_constructor(
        &mut self,
        constructor: Constructor,
        bindings: &[(String, usize)],
    ) -> Result<()> {
        match constructor {
            // Identity `.` means project the binding tuple
            Constructor::None => {
                self.emit_obj();
                for (var, csr) in bindings {
                    self.emit_load(*csr);
                    self.emit_obj_assign(var.clone());
                }
            }
            // Spread `*` means merge all binding values into an object: an
            // object binding spreads its fields, a non-object binding (e.g. an
            // unnested scalar) is kept under its alias.
            Constructor::Star => {
                if let [(_, csr)] = bindings {
                    self.emit_load(*csr);
                } else {
                    self.emit_obj();
                    for (var, csr) in bindings {
                        self.emit_load(*csr);
                        self.emit_obj_merge(var.clone());
                    }
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
            | Vop::Next { csr: _, jmp }
            | Vop::Scan { csr: _, jmp }
            | Vop::Iter { csr: _, jmp }
            | Vop::If(jmp)
            | Vop::IfNot(jmp)
            | Vop::CntIfPos(_, jmp)
            | Vop::CntIfZero(_, jmp) => *jmp = dst,
            _ => unsupported!("cannot patch instruction at pc[{}]", src),
        }
        Ok(())
    }

    /// Ensures the program has at least the given transaction mode
    fn ensure_txn(&mut self, txn: TransactionMode) {
        self.txm = Some(txn.coalesce(self.txm));
    }

    //------------------------------
    // EXPRESSIONS
    //------------------------------

    fn cc_expr(&mut self, expr: Expr) -> Result<()> {
        match expr {
            Expr::Call(call) => self.cc_expr_call(call),
            Expr::Jpe(jpe) => self.cc_expr_jpe(jpe),
            Expr::Jpi(jpi) => self.cc_expr_jpi(jpi),
            Expr::Jpk(jpk) => self.cc_expr_jpk(jpk),
            Expr::Lit(val) => {
                self.cc_expr_lit(val);
                Ok(())
            },
            Expr::Obj(obj) => self.cc_expr_obj(obj),
            Expr::Array(items) => self.cc_expr_array(items),
            Expr::Var(var) => {
                self.cc_expr_var(&var);
                Ok(())
            },
        }
    }

    #[allow(clippy::len_zero)]
    fn cc_expr_call(&mut self, call: Call) -> Result<()> {
        let Call { name, args } = call;
        let (arity_ok, op) = match name.as_str() {
            "*"   => (args.len() == 2, Vop::Mul),
            "/"   => (args.len() == 2, Vop::Div),
            "%"   => (args.len() == 2, Vop::Rem),
            "+"   => (args.len() == 2, Vop::Add),
            "-"   => (args.len() == 2, Vop::Sub),
            "<"   => (args.len() == 2, Vop::Lt),
            "<="  => (args.len() == 2, Vop::Le),
            "="   => (args.len() == 2, Vop::Eq),
            ">="  => (args.len() == 2, Vop::Ge),
            ">"   => (args.len() == 2, Vop::Gt),
            "!="  => (args.len() == 2, Vop::Ne),
            "and" => (args.len() == 2, Vop::And),
            "or"  => (args.len() == 2, Vop::Or),
            "not"         => (args.len() == 1, Vop::Not),
            "is_null"     => (args.len() == 1, Vop::IsNull),
            "is_true"     => (args.len() == 1, Vop::IsTrue),
            "is_false"    => (args.len() == 1, Vop::IsFalse),
            "is_unknown"  => (args.len() == 1, Vop::IsUnknown),
            "between"     => (args.len() == 3, Vop::Between),
            "in_list"     => (args.len() >= 1, Vop::InList(args.len().saturating_sub(1))),
            _ => return Err(Error::UnknownFunction(name)),
        };
        if !arity_ok {
            return Err(Error::UnknownFunction(name));
        }
        for arg in args {
            self.cc_expr(arg)?;
        }
        self.code.push(op);
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

    fn cc_expr_lit(&mut self, value: Value) {
        self.emit_push(value);
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

    fn cc_expr_array(&mut self, items: Vec<Expr>) -> Result<()> {
        self.emit_arr();
        for item in items {
            self.cc_expr(item)?;
            self.emit_arr_push();
        }
        Ok(())
    }

    fn cc_expr_var(&mut self, var: &Var) {
        let csr = var.bind.expect("all variables should be bound") as usize;
        self.emit_load(csr);
    }

    //------------------------------
    // HELPERS
    //------------------------------

    /// Return current pc index.
    fn pc(&self) -> usize {
        self.code.len() - 1
    }

    /// Record that cursor slot `csr` is in use.
    fn use_cursor(&mut self, csr: usize) {
        self.cursor_slots = self.cursor_slots.max(csr + 1);
    }

    /// Allocate the next cursor slot.
    fn alloc_cursor(&mut self) -> usize {
        let csr = self.cursor_slots;
        self.cursor_slots += 1;
        csr
    }

    /// Allocate the next counter slot.
    fn alloc_counter(&mut self) -> usize {
        let cnt = self.counter_slots;
        self.counter_slots += 1;
        cnt
    }

    //------------------------------
    // INSTRUCTIONS
    //------------------------------

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
        self.use_cursor(csr);
        self.code.push(Vop::Insert { csr });
    }

    fn emit_delete(&mut self, csr: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::Delete { csr });
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
        self.use_cursor(csr);
        self.code.push(Vop::Load { csr });
    }

    fn emit_jump(&mut self, jmp: usize) {
        self.code.push(Vop::Jump { jmp });
    }

    fn emit_next(&mut self, csr: usize, jmp: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::Next { csr, jmp });
    }

    fn emit_new_oid(&mut self, csr: usize) {
        self.use_cursor(csr);
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

    fn emit_obj_merge(&mut self, name: String) {
        self.code.push(Vop::ObjMerge(name));
    }

    fn emit_arr(&mut self) {
        self.code.push(Vop::Arr);
    }

    fn emit_arr_push(&mut self) {
        self.code.push(Vop::ArrPush);
    }

    fn emit_open(&mut self, csr: usize, tbl: u32) {
        self.use_cursor(csr);
        self.code.push(Vop::Open { csr, tbl });
    }

    fn emit_scan(&mut self, csr: usize, jmp: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::Scan { csr, jmp });
    }

    fn emit_iter(&mut self, csr: usize, jmp: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::Iter { csr, jmp });
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
    use crate::catalog::Catalog;
    use crate::lexer::SqlLexer;
    use crate::parser::SqlParser;
    use crate::storage::Storage;
    use crate::binder::Binder;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Storage, Catalog) {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(dir.path().join("test.db")).unwrap();
        let catalog = Catalog::load(&storage).unwrap();
        (dir, storage, catalog)
    }

    fn compile_sql(storage: &Storage, catalog: &Catalog, sql: &str) -> Program {
        let mut stmt = SqlParser::new().parse(SqlLexer::new(sql)).unwrap();
        let txn = storage.read_txn().unwrap();
        let mut binder = Binder::new(catalog.clone(), &txn);
        binder.bind(&mut stmt).unwrap();
        txn.commit().unwrap();
        Compiler::new().compile(stmt).unwrap()
    }

    #[test]
    fn select_star_from_catalog_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select * from catalog;");
        assert_eq!(program.cursors, 1);
        let code = program.instructions;
        assert_eq!(code.len(), 9);
        assert!(matches!(code[0], Vop::Init { jmp: 7 }));
        assert!(matches!(code[1], Vop::Open { csr: 0, tbl: 0 }));
        assert!(matches!(code[2], Vop::Scan { csr: 0, jmp: 6 }));
        assert!(matches!(code[3], Vop::Load { csr: 0 }));
        assert!(matches!(code[4], Vop::Yield));
        assert!(matches!(code[5], Vop::Next { csr: 0, jmp: 3 }));
        assert!(matches!(code[6], Vop::Halt));
        assert!(matches!(code[7], Vop::Transaction { txm: TransactionMode::Read }));
        assert!(matches!(code[8], Vop::Jump { jmp: 1 }));
    }

    #[test]
    fn select_where_true_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select * from catalog where true;");
        let code = program.instructions;
        assert!(matches!(code[2], Vop::Scan { csr: 0, jmp: 8 }));
        assert!(matches!(code[3], Vop::Push { .. }));
        assert!(matches!(code[4], Vop::IfNot(7)));
        assert!(matches!(code[5], Vop::Load { csr: 0 }));
        assert!(matches!(code[6], Vop::Yield));
        assert!(matches!(code[7], Vop::Next { csr: 0, jmp: 3 }));
    }

    #[test]
    fn delete_all_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "delete from catalog;");
        assert_eq!(program.cursors, 1);
        let code = program.instructions;
        assert_eq!(code.len(), 8);
        assert!(matches!(code[0], Vop::Init { jmp: 6 }));
        assert!(matches!(code[1], Vop::Open { csr: 0, tbl: 0 }));
        assert!(matches!(code[2], Vop::Scan { csr: 0, jmp: 5 }));
        assert!(matches!(code[3], Vop::Delete { csr: 0 }));
        assert!(matches!(code[4], Vop::Next { csr: 0, jmp: 3 }));
        assert!(matches!(code[5], Vop::Halt));
        assert!(matches!(code[6], Vop::Transaction { txm: TransactionMode::Write }));
        assert!(matches!(code[7], Vop::Jump { jmp: 1 }));
    }

    #[test]
    fn delete_where_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program =
            compile_sql(&storage, &catalog, "delete from catalog where catalog.name = 'x';");
        let code = program.instructions;
        // Scan exits past the loop; a false predicate skips Delete and advances.
        assert!(matches!(code[2], Vop::Scan { csr: 0, jmp: 10 }));
        assert!(matches!(code[3], Vop::Load { csr: 0 }));
        assert!(matches!(code[7], Vop::IfNot(9)));
        assert!(matches!(code[8], Vop::Delete { csr: 0 }));
        assert!(matches!(code[9], Vop::Next { csr: 0, jmp: 3 }));
    }

    #[test]
    fn create_table_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "create table t (id int);");
        assert_eq!(program.cursors, 1);
        let code = program.instructions;
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
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select * from catalog;");
        assert_eq!(program.cursors, 1);
        let code = program.instructions;
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
