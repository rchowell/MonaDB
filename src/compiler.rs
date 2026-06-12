//! IR → `Vop` bytecode compiler.
//!
//! `cc_*` methods walk a bound [`Statement`] and append instructions through the
//! `emit_*` helpers. Control-flow ops are emitted with placeholder jump targets
//! and back-patched via [`Compiler::patch`] once the loop body's extent is known.

use serde_json::json;

use crate::catalog::CATALOG_OID;
use crate::error::Error;
use crate::ir::{
    Call, Clear, Constructor, Create, Delete, Drop, Expr, Get, Insert, Jpe, Jpi, Jpk, Limit, Member, Obj, Key, Type, Var, Select, Source, Statement, ToSql
};
use crate::schema;
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

/// Translates a bound SQL statement into a `Program` of `Vop` bytecode.
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
    /// Creates an empty compiler.
    pub fn new() -> Compiler {
        Compiler {
            code: vec![],
            cursor_slots: 0,
            counter_slots: 0,
            txm: None,
        }
    }

    /// Compiles a statement into a self-contained `Program`, laid out in three
    /// blocks. `Init` jumps over the body to the transaction block; that block
    /// opens the transaction and `Jump`s back, so the body always runs inside a
    /// live transaction. The body ends at `Halt`, which commits.
    ///
    ///   addr 0      Init         ────┐
    ///   addr 1..M   [ body ]     ◀─┐ │
    ///   addr M      Halt           │ │
    ///   addr N      Transaction   ◀─┼─┘
    ///   addr N+1    Jump           ──┘
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
            Statement::Create(create) => self.cc_create(&create)?,
            Statement::Delete(delete) => self.cc_delete(delete)?,
            Statement::Drop(drop) => self.cc_drop(&drop),
            Statement::Clear(clear) => self.cc_clear(&clear),
            Statement::Insert(insert) => self.cc_insert(insert)?,
            Statement::Select(select) => self.cc_select(select)?,
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

    /// Compiles a CREATE TABLE: insert the table's definition into the catalog.
    fn cc_create(&mut self, create: &Create) -> Result<()> {
        self.txm = Some(TransactionMode::Write);

        let Create::Table(table_definition) = &create;

        // Validate the key columns are int or string.
        for member in &table_definition.keys {
            if !matches!(member.ty, Type::Int | Type::String) {
                unsupported!("key column '{}' must be int or string", member.name);
            }
        }

        // Create the catalog table object for insertion.
        let object = json!({
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
        let csr = self.alloc_cursor();
        self.emit_open(csr, CATALOG_OID);
        self.emit_push(object);
        self.emit_new_oid(csr);
        self.emit_new_btree();
        self.emit_insert(csr);
        Ok(())
    }

    /// Compiles an INSERT: open the target, then build and store each row.
    fn cc_insert(&mut self, insert: Insert) -> Result<()> {
        self.ensure_txn(TransactionMode::Write);
        let csr = self.alloc_cursor();
        let tbl = insert.target.oid.expect("insert target should be resolved to an oid");
        self.emit_open(csr, tbl);
        let members = insert.target.keys;
        for val in insert.source {
            self.cc_expr(val)?;
            // A keyed table validates each key column is present + typed, then
            // derives its key from the row's fields; a keyless one gets a
            // surrogate id. Either way the key lands above the value on the
            // stack, ready for Insert to consume.
            if members.is_empty() {
                self.emit_new_oid(csr);
            } else {
                self.emit_new_key(members.clone());
            }
            self.emit_insert(csr);
        }
        Ok(())
    }

    /// The bound (cursor slot, table oid) of a mutation's `from`. Both are set
    /// by the binder; a missing one means the target was never bound (a compiler
    /// invariant), so error rather than panic.
    fn bound_target(from: &crate::ir::From) -> Result<(usize, u32)> {
        let Some(csr) = from.csr else {
            crate::error!("mutation target was not bound to a cursor");
        };
        let Some(oid) = from.oid else {
            crate::error!("mutation target has no table oid");
        };
        Ok((csr as usize, oid))
    }

    /// Compiles a DELETE as two passes: collect the matching keys into an array,
    /// then delete each (a scan can't mutate its own btree mid-iteration).
    fn cc_delete(&mut self, delete: Delete) -> Result<()> {
        self.ensure_txn(TransactionMode::Write);
        let Delete { from, where_ } = delete;
        let (tcsr, oid) = Self::bound_target(&from)?;

        // Pass 1. collect the keys of matching rows into an array value.
        self.emit_arr();
        self.emit_open(tcsr, oid);
        // `dcsr` iterates the collected key array in phase 2. Allocate it after
        // `tcsr` is in use so it never collides with the binder's cursor index.
        let dcsr = self.alloc_cursor();

        self.emit_scan(tcsr, 0); // jmp patched to the Close (empty/exhausted)
        let scan = self.pc();
        let top = self.code.len();

        let mut where_fail = None;
        if let Some(where_) = where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            where_fail = Some(self.pc());
        }
        self.emit_load_key(tcsr);
        self.emit_arr_push();

        self.emit_next(tcsr, top);
        let next_pc = self.pc();

        // Pass 2. Call delete for each key in the array value.
        self.emit_close(tcsr);
        let close_pc = self.pc();

        self.emit_iter(dcsr, 0); // jmp patched to the exit (empty key list)
        let iter = self.pc();
        let loop_top = self.code.len();

        self.emit_load(dcsr);
        self.emit_delete(tcsr);
        self.emit_next(dcsr, loop_top);

        let exit = self.pc() + 1;
        self.patch(scan, close_pc)?;
        self.patch(iter, exit)?;
        if let Some(pc) = where_fail {
            self.patch(pc, next_pc)?;
        }
        Ok(())
    }

    /// Compiles a DROP TABLE: delete the catalog row, then clear the data btree.
    fn cc_drop(&mut self, drop: &Drop) {
        self.ensure_txn(TransactionMode::Write);
        let oid = drop.oid.expect("drop target should be bound to table oid");

        // Dropping a table deletes its catalog row then clears its data btree.
        // The catalog delete is a point delete of a known key with no scan, so
        // it writes immediately.
        //
        // 0   Open { tbl=0 }      Open the 'catalog' system table (oid=0)
        // 1   Push { val=oid }    Push the table oid; the catalog key to delete.
        // 2   Delete              Delete catalog[oid] immediately.
        // 3   Clear { oid }       Clear the dropped table's data btree.
        //
        let csr = self.alloc_cursor();
        self.emit_open(csr, CATALOG_OID);
        self.emit_push(Value::Oid(oid));
        self.emit_delete(csr);
        self.emit_clear(oid);
    }

    /// Compiles a CLEAR: empty the table's data btree, leaving its catalog row.
    fn cc_clear(&mut self, clear: &Clear) {
        self.ensure_txn(TransactionMode::Write);
        let oid = clear.oid.expect("clear target should be bound to table oid");
        // Clearing a table empties its data btree but leaves the catalog row.
        self.emit_clear(oid);
    }

    /// Compiles a SELECT, streaming the nested-loop from/where/limit/project
    /// path. An `ORDER BY` can't stream, so it detours to [`Self::cc_order`].
    fn cc_select(&mut self, select: Select) -> Result<()> {
        self.ensure_txn(TransactionMode::Read);

        // ORDER BY can't stream: it materializes the post-where stream, sorts
        // it, then projects. That two-phase path lives in cc_order.
        if select.order.is_some() {
            return self.cc_order(select);
        }

        // Initialize the limit counters before the loop.
        let (cnt_skip, cnt_take) = self.emit_limit_counters(select.limit.as_ref());

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

        // Compile the offset/limit guards.
        let (offset, limit_pc) = self.emit_limit_checks(cnt_skip, cnt_take);

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

    /// Compiles a query with an ORDER BY. Two phases over one materialized array:
    ///
    /// Phase 1 scans the `from`/`where` stream (the same nested-loop machinery
    /// as `cc_select`) and, per surviving binding tuple, pushes a tagged element
    /// `[order_key_bytes, payload]` onto a collector array. Phase 2 sorts the
    /// collector by the key bytes, then iterates it on a payload cursor: it
    /// applies the limit, re-seeds each from-binding via `SetVal` so the select
    /// constructor compiles exactly as in the streaming path, and yields. Per
    /// the spec, select runs after limit (§4.9), so projection is post-sort.
    #[allow(clippy::too_many_lines)]
    fn cc_order(&mut self, select: Select) -> Result<()> {
        let Select { from, where_, order, limit, select: constructor } = select;
        let order = order.expect("cc_order requires an order clause");
        let dirs: Vec<bool> = order.keys.iter().map(|k| k.desc).collect();

        // The (alias, cursor) bindings in item order; drive the payload + projection.
        let bindings: Vec<(String, usize)> = from
            .iter()
            .map(|f| (f.var.clone(), f.csr.expect("from item should be bound") as usize))
            .collect();
        let n = bindings.len();

        // Register the from-cursors, then allocate the phase-2 payload cursor.
        for (_, csr) in &bindings {
            self.use_cursor(*csr);
        }
        let payload_csr = self.alloc_cursor();

        // The collector array lives at the bottom of the stack across phase 1.
        self.emit_arr();

        // Open table sources once before the loop; value sources need no open.
        for f in &from {
            if let Source::Table(_) = &f.src {
                let csr = f.csr.expect("from item should be bound") as usize;
                let oid = f.oid.expect("bind pass must set oid for Table");
                self.emit_open(csr, oid);
            }
        }

        // Begin one iteration per source, outer to inner (mirrors cc_select).
        let mut entry = vec![0usize; n];
        let mut begin = vec![0usize; n];
        for (i, f) in from.into_iter().enumerate() {
            let csr = f.csr.expect("from item should be bound") as usize;
            match f.src {
                Source::Table(_) => {
                    self.emit_scan(csr, 0);
                    entry[i] = self.pc();
                }
                Source::Value(expr) => {
                    entry[i] = self.pc() + 1;
                    self.cc_expr(*expr)?;
                    self.emit_iter(csr, 0);
                }
            }
            begin[i] = self.pc();
        }

        // Innermost body: residual filter, then tag + collect.
        let body = self.code.len();
        let mut where_fail = None;
        if let Some(where_) = where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            where_fail = Some(self.pc());
        }

        // Build the tagged element [order_key_bytes, payload].
        self.emit_arr();
        for k in order.keys {
            self.cc_expr(k.expr)?;
        }
        self.emit_order_key(dirs);
        self.emit_arr_push();
        // The payload is the binding tuple, exactly what `select .` builds.
        self.cc_select_constructor(Constructor::None, &bindings)?;
        self.emit_arr_push();
        self.emit_arr_push();

        // Close the loops inner to outer (mirrors cc_select).
        let mut next_pc = vec![0usize; n];
        for i in (0..n).rev() {
            let resume = if i + 1 < n { entry[i + 1] } else { body };
            self.emit_next(bindings[i].1, resume);
            next_pc[i] = self.pc();
        }

        // Patch the phase-1 exhaust edges. The outermost source exhausting ends
        // phase 1 and falls into the sort; an inner source advances its enclosing one.
        let sort_pc = self.pc() + 1;
        self.patch(begin[0], sort_pc)?;
        for i in 1..n {
            self.patch(begin[i], next_pc[i - 1])?;
        }
        let inner = next_pc[n - 1];
        if let Some(pc) = where_fail {
            self.patch(pc, inner)?;
        }

        // Phase 2: drop the read iterators, then sort the collector by key bytes.
        for (_, csr) in &bindings {
            self.emit_close(*csr);
        }
        self.emit_sort();

        // Limit counters apply to the sorted stream (order then limit, §4.9).
        let (cnt_skip, cnt_take) = self.emit_limit_counters(limit.as_ref());

        // Iterate the sorted collector on the payload cursor.
        self.emit_iter(payload_csr, 0);
        let begin_payload = self.pc();
        let loop_top = self.code.len();

        // Limit: skip drops the row, take exhausted ends the scan.
        let (offset, limit_pc) = self.emit_limit_checks(cnt_skip, cnt_take);

        // Re-seed each from-binding from the payload (element[1]) so the select
        // constructor reads it via the same LoadVal as a live scan.
        for (alias, csr) in &bindings {
            self.emit_load(payload_csr);
            self.emit_jpi(1);
            self.emit_jpk(alias.clone());
            self.emit_set_val(*csr);
        }

        // Project and yield.
        self.cc_select_constructor(constructor, &bindings)?;
        self.emit_yield();

        // Advance; patch the exhaust/skip/take edges now that targets are known.
        self.emit_next(payload_csr, loop_top);
        let next_payload = self.pc();
        let exit = self.pc() + 1;
        self.patch(begin_payload, exit)?;
        if let Some(pc) = offset {
            self.patch(pc, next_payload)?;
        }
        if let Some(pc) = limit_pc {
            self.patch(pc, exit)?;
        }
        Ok(())
    }

    /// Allocates and initializes the skip/take counters for a `limit`, returning
    /// their slots. `Limit N..M` is half-open `[N, M)`: skip N rows, then take
    /// `M - N` (saturating, so `M <= N` yields nothing). The `CntSet`s emit at
    /// the current pc, so the caller controls placement — before the streaming
    /// loop (`cc_select`) or after the sort (`cc_order`).
    fn emit_limit_counters(&mut self, limit: Option<&Limit>) -> (Option<usize>, Option<usize>) {
        let mut cnt_skip = None;
        let mut cnt_take = None;
        if let Some(limit) = limit {
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
        (cnt_skip, cnt_take)
    }

    /// Emits the per-row offset/limit guards from counters made by
    /// [`Self::emit_limit_counters`], returning the `(offset, limit)` jump sites
    /// to back-patch: `offset` drops the row (skip not yet exhausted), `limit`
    /// exits the loop (take exhausted).
    fn emit_limit_checks(
        &mut self,
        cnt_skip: Option<usize>,
        cnt_take: Option<usize>,
    ) -> (Option<usize>, Option<usize>) {
        let mut offset = None;
        if let Some(c) = cnt_skip {
            self.emit_cnt_if_pos(c, 0);
            offset = Some(self.pc());
        }
        let mut limit_pc = None;
        if let Some(c) = cnt_take {
            self.emit_cnt_if_zero(c, 0);
            limit_pc = Some(self.pc());
        }
        (offset, limit_pc)
    }

    /// Compiles the projection onto the stack: `.` builds a tuple of the
    /// bindings, `*` merges them into one object, an expr/list builds a value.
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

    /// Patches the jump target of the control-flow instruction at `src` to `dst`.
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

    /// Raises the program's transaction mode to at least `txn`.
    fn ensure_txn(&mut self, txn: TransactionMode) {
        self.txm = Some(txn.coalesce(self.txm));
    }

    //------------------------------
    // EXPRESSIONS
    //------------------------------

    /// Compiles an expression, leaving its value on top of the stack.
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
            // Binding already lowered a full-key table subscript to this node;
            // we encode the literal key and emit the point lookup.
            Expr::Get(get) => self.cc_expr_get(&get),
            // A multi-element subscript that survived binding is a value
            // multi-selector — deferred (path-receiver multi-selectors are not v1).
            Expr::Subscript(_) => {
                unsupported!("multi-element subscript on a value is not supported")
            }
        }
    }

    /// Keyed-table access `table[key, ...]`. Literal keys are encoded at COMPILE
    /// time (v1: literal keys only); a type mismatch surfaces here as an
    /// `Error::Schema` (e.g. `t["a"]` on an int key). A full key (arity == key
    /// count) is a point lookup (`Get` → the one row or null); a leading prefix
    /// (arity < key count) is a range lookup (`GetRange` → the matching rows as
    /// an array, in key order). The surrounding `select` has already emitted
    /// `Transaction(Read)`, so the cursor ops run under it.
    fn cc_expr_get(&mut self, get: &Get) -> Result<()> {
        let key = schema::encode_key_tuple(&get.args, &get.keys)?;
        self.emit_open(get.csr as usize, get.oid);
        self.emit_push(Value::Bytes(key.into()));
        if get.args.len() == get.keys.len() {
            self.emit_get(get.csr as usize);
        } else {
            self.emit_get_range(get.csr as usize);
        }
        Ok(())
    }

    /// Compiles a builtin call to its operator: arithmetic, comparison, 3VL
    /// logic, `between`, or `in_list`. An unknown name or bad arity errors.
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

    /// Compiles a computed path step `input[expr]`.
    fn cc_expr_jpe(&mut self, jpe: Jpe) -> Result<()> {
        self.cc_expr(*jpe.inp)?;
        self.cc_expr(*jpe.exp)?;
        self.emit_jpe();
        Ok(())
    }

    /// Compiles a path index `input[i]`.
    fn cc_expr_jpi(&mut self, jpi: Jpi) -> Result<()> {
        self.cc_expr(*jpi.inp)?;
        self.emit_jpi(jpi.idx);
        Ok(())
    }

    /// Compiles a path key `input.key`.
    fn cc_expr_jpk(&mut self, jpk: Jpk) -> Result<()> {
        self.cc_expr(*jpk.inp)?;
        self.emit_jpk(jpk.key);
        Ok(())
    }

    /// Compiles a literal: push it onto the stack.
    fn cc_expr_lit(&mut self, value: Value) {
        self.emit_push(value);
    }

    /// Compiles an object constructor, assigning or spreading each member.
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

    /// Compiles an array constructor, pushing then appending each element.
    fn cc_expr_array(&mut self, items: Vec<Expr>) -> Result<()> {
        self.emit_arr();
        for item in items {
            self.cc_expr(item)?;
            self.emit_arr_push();
        }
        Ok(())
    }

    /// Compiles a variable reference: load its bound cursor's current value.
    fn cc_expr_var(&mut self, var: &Var) {
        let csr = var.bind.expect("all variables should be bound") as usize;
        self.emit_load(csr);
    }

    //------------------------------
    // HELPERS
    //------------------------------

    /// Returns the index of the last emitted instruction.
    fn pc(&self) -> usize {
        self.code.len() - 1
    }

    /// Records that cursor slot `csr` is in use, growing the slot count.
    fn use_cursor(&mut self, csr: usize) {
        self.cursor_slots = self.cursor_slots.max(csr + 1);
    }

    /// Allocates and returns the next cursor slot.
    fn alloc_cursor(&mut self) -> usize {
        let csr = self.cursor_slots;
        self.cursor_slots += 1;
        csr
    }

    /// Allocates and returns the next counter slot.
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

    fn emit_new_key(&mut self, keys: Vec<Key>) {
        self.code.push(Vop::NewKey { keys });
    }

    fn emit_delete(&mut self, csr: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::Delete { csr });
    }

    fn emit_close(&mut self, csr: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::Close { csr });
    }

    fn emit_load_key(&mut self, csr: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::LoadKey { csr });
    }

    fn emit_clear(&mut self, tbl: u32) {
        self.code.push(Vop::Clear { tbl });
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
        self.code.push(Vop::LoadVal { csr });
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

    fn emit_order_key(&mut self, dirs: Vec<bool>) {
        self.code.push(Vop::OrderKey { dirs });
    }

    fn emit_sort(&mut self) {
        self.code.push(Vop::Sort);
    }

    fn emit_set_val(&mut self, csr: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::SetVal { csr });
    }

    fn emit_open(&mut self, csr: usize, tbl: u32) {
        self.use_cursor(csr);
        self.code.push(Vop::Open { csr, tbl });
    }

    fn emit_get(&mut self, csr: usize) {
        self.code.push(Vop::Get { csr });
    }

    fn emit_get_range(&mut self, csr: usize) {
        self.code.push(Vop::GetRange { csr });
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
        assert!(matches!(code[3], Vop::LoadVal { csr: 0 }));
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
        assert!(matches!(code[5], Vop::LoadVal { csr: 0 }));
        assert!(matches!(code[6], Vop::Yield));
        assert!(matches!(code[7], Vop::Next { csr: 0, jmp: 3 }));
    }

    #[test]
    fn order_by_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program =
            compile_sql(&storage, &catalog, "select * from catalog order by catalog.name;");
        // Two cursors: the table scan (0) and the sorted-payload iterator (1).
        assert_eq!(program.cursors, 2);
        let code = program.instructions;
        assert_eq!(code.len(), 28);
        assert!(matches!(code[0], Vop::Init { jmp: 26 }));
        assert!(matches!(code[1], Vop::Arr)); // collector
        assert!(matches!(code[2], Vop::Open { csr: 0, tbl: 0 }));
        // Phase 1: scan, build [sortkey, payload], collect; empty table -> Close.
        assert!(matches!(code[3], Vop::Scan { csr: 0, jmp: 15 }));
        assert!(matches!(code[4], Vop::Arr)); // element
        assert!(matches!(code[5], Vop::LoadVal { csr: 0 }));
        assert!(matches!(code[6], Vop::Jpk(_))); // catalog.name
        assert!(matches!(code[7], Vop::OrderKey { .. }));
        assert!(matches!(code[8], Vop::ArrPush)); // elem[0] = key bytes
        assert!(matches!(code[9], Vop::Obj)); // payload {catalog: row}
        assert!(matches!(code[10], Vop::LoadVal { csr: 0 }));
        assert!(matches!(code[11], Vop::ObjAssign(_)));
        assert!(matches!(code[12], Vop::ArrPush)); // elem[1] = payload
        assert!(matches!(code[13], Vop::ArrPush)); // collector += elem
        assert!(matches!(code[14], Vop::Next { csr: 0, jmp: 4 }));
        // Phase 2: release the read iterator, sort, then drain the sorted array.
        assert!(matches!(code[15], Vop::Close { csr: 0 }));
        assert!(matches!(code[16], Vop::Sort));
        assert!(matches!(code[17], Vop::Iter { csr: 1, jmp: 25 }));
        // Re-seed the `catalog` binding from the payload, then project + yield.
        assert!(matches!(code[18], Vop::LoadVal { csr: 1 }));
        assert!(matches!(code[19], Vop::Jpi(1)));
        assert!(matches!(code[20], Vop::Jpk(_)));
        assert!(matches!(code[21], Vop::SetVal { csr: 0 }));
        assert!(matches!(code[22], Vop::LoadVal { csr: 0 }));
        assert!(matches!(code[23], Vop::Yield));
        assert!(matches!(code[24], Vop::Next { csr: 1, jmp: 18 }));
        assert!(matches!(code[25], Vop::Halt));
        assert!(matches!(code[26], Vop::Transaction { txm: TransactionMode::Read }));
        assert!(matches!(code[27], Vop::Jump { jmp: 1 }));
    }

    #[test]
    fn delete_all_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "delete from catalog;");
        assert_eq!(program.cursors, 2);
        let code = program.instructions;
        assert_eq!(code.len(), 15);
        assert!(matches!(code[0], Vop::Init { jmp: 13 }));
        assert!(matches!(code[1], Vop::Arr));
        assert!(matches!(code[2], Vop::Open { csr: 0, tbl: 0 }));
        // Phase 1: collect matching keys; an empty table jumps straight to Close.
        assert!(matches!(code[3], Vop::Scan { csr: 0, jmp: 7 }));
        assert!(matches!(code[4], Vop::LoadKey { csr: 0 }));
        assert!(matches!(code[5], Vop::ArrPush));
        assert!(matches!(code[6], Vop::Next { csr: 0, jmp: 4 }));
        // Release the table's read iterator before any delete.
        assert!(matches!(code[7], Vop::Close { csr: 0 }));
        // Phase 2: `select delete(key) from keys` over a value cursor.
        assert!(matches!(code[8], Vop::Iter { csr: 1, jmp: 12 }));
        assert!(matches!(code[9], Vop::LoadVal { csr: 1 }));
        assert!(matches!(code[10], Vop::Delete { csr: 0 }));
        assert!(matches!(code[11], Vop::Next { csr: 1, jmp: 9 }));
        assert!(matches!(code[12], Vop::Halt));
        assert!(matches!(code[13], Vop::Transaction { txm: TransactionMode::Write }));
        assert!(matches!(code[14], Vop::Jump { jmp: 1 }));
    }

    #[test]
    fn delete_where_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program =
            compile_sql(&storage, &catalog, "delete from catalog where catalog.name = 'x';");
        let code = program.instructions;
        // Phase 1: scan exits to Close; a false predicate skips the collect.
        assert!(matches!(code[3], Vop::Scan { csr: 0, jmp: 12 }));
        assert!(matches!(code[4], Vop::LoadVal { csr: 0 }));   // predicate reads the row
        assert!(matches!(code[8], Vop::IfNot(11)));
        assert!(matches!(code[9], Vop::LoadKey { csr: 0 }));
        assert!(matches!(code[10], Vop::ArrPush));
        assert!(matches!(code[11], Vop::Next { csr: 0, jmp: 4 }));
        assert!(matches!(code[12], Vop::Close { csr: 0 }));
        // Phase 2: delete each collected key.
        assert!(matches!(code[13], Vop::Iter { csr: 1, jmp: 17 }));
        assert!(matches!(code[14], Vop::LoadVal { csr: 1 }));
        assert!(matches!(code[15], Vop::Delete { csr: 0 }));
        assert!(matches!(code[16], Vop::Next { csr: 1, jmp: 14 }));
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
                | Vop::LoadVal { csr }
                | Vop::Next { csr, .. } => assert_eq!(*csr, 0),
                _ => {}
            }
        }
    }

    #[test]
    fn keyed_insert_emits_encode_key() {
        // A keyed insert derives the composite key with a single EncodeKey, which
        // gathers, validates, and encodes the declared columns. The binder's
        // catalog lookup is simulated.
        let mut stmt = SqlParser::new()
            .parse(SqlLexer::new("insert into t ({a: 1, b: \"x\"});"))
            .unwrap();
        let Statement::Insert(ins) = &mut stmt else {
            panic!("expected insert");
        };
        ins.target.oid = Some(1);
        ins.target.keys = vec![
            Key { name: "a".into(), ty: Type::Int },
            Key { name: "b".into(), ty: Type::String },
        ];
        let members = ins.target.keys.clone();

        let code = Compiler::new().compile(stmt).unwrap().instructions;
        // EncodeKey carries the declared key columns in order, and precedes Insert.
        let encode = code
            .iter()
            .position(|op| matches!(op, Vop::NewKey { keys } if *keys == members))
            .expect("keyed insert must emit EncodeKey for its key columns");
        let insert = code
            .iter()
            .position(|op| matches!(op, Vop::Insert { .. }))
            .expect("insert must emit Insert");
        assert!(encode < insert, "EncodeKey must precede Insert");
    }

}
