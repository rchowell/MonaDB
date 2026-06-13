//! IR → `Vop` bytecode compiler.
//!
//! `cc_*` methods walk a bound [`Statement`] and append instructions through the
//! `emit_*` helpers. Control-flow ops are emitted with placeholder jump targets
//! and back-patched via [`Compiler::patch`] once the loop body's extent is known.

use serde_json::json;

use crate::Result;
use crate::catalog::CATALOG_OID;
use crate::error::Error;
use crate::functions;
use crate::ir::{
    AggKind, Call, Clear, Constructor, Create, Delete, Drop, Expr, Get, Insert, Jpe, Jpi, Jpk, Key,
    Limit, Member, Obj, Select, Source, Statement, ToSql, Type, Var,
};
use crate::schema;
use crate::transaction::TransactionMode;
use crate::value::Value;
use crate::visitor::visit::{self, Visit};
use crate::visitor::visit_mut::{self, VisitMut};
use crate::vm::{Program, Vop};
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
    /// Number of aggregate-accumulator slots required (one per aggregate term).
    agg_slots: usize,
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
            agg_slots: 0,
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
            aggs: self.agg_slots,
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
        let tbl = insert
            .target
            .oid
            .expect("insert target should be resolved to an oid");
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
        let oid = clear
            .oid
            .expect("clear target should be bound to table oid");
        // Clearing a table empties its data btree but leaves the catalog row.
        self.emit_clear(oid);
    }

    /// Compiles a SELECT, streaming the nested-loop from/where/limit/project
    /// path. An `ORDER BY` can't stream, so it detours to [`Self::cc_order`].
    fn cc_select(&mut self, select: Select) -> Result<()> {
        self.ensure_txn(TransactionMode::Read);

        // GROUP BY sorts the post-where stream by the grouping key, then streams
        // it, resetting the accumulators at each group boundary. Checked first so
        // a grouped query reaches cc_group's own projection rules.
        if select.group.is_some() {
            return self.cc_group(select);
        }

        // PIVOT folds the whole stream into one tuple instead of projecting per
        // row, so it has its own accumulate-then-yield path in cc_pivot. Checked
        // before the aggregate path so `pivot … having` reaches cc_pivot's reject.
        if let Constructor::Pivot(_) = &select.select {
            return self.cc_pivot(select);
        }

        // An aggregate projection (or a HAVING) collapses the stream to one row;
        // that path accumulates instead of yielding per row, so it lives in
        // cc_aggregate (which treats the whole input as a single group).
        if has_aggregate(&select.select) || select.having.is_some() {
            return self.cc_aggregate(select);
        }

        // ORDER BY can't stream: it materializes the post-where stream, sorts
        // it, then projects. That two-phase path lives in cc_order.
        if select.order.is_some() {
            return self.cc_order(select);
        }

        // Initialize the limit counters before the loop.
        let (cnt_skip, cnt_take) = self.emit_limit_counters(select.limit.as_ref());

        let Select {
            from,
            where_,
            select: constructor,
            ..
        } = select;

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

        // `loop_csr[i]` is the cursor source i advances (a table scan, a value
        // iterator, or — for unpivot — its attribute-value pair iterator).
        // `bindings` is the flattened projection environment: a table/value
        // source contributes one binding; an unpivot contributes its value (and
        // optional attribute-name) binding. `seeds` re-derives those from the
        // current pair at the top of each body iteration.
        let loop_csr = Self::loop_cursors(&from);
        let bindings = Self::from_bindings(&from);
        let seeds = Self::unpivot_seeds(&from);
        let n = from.len();

        // Open table sources once before the loop; value sources need no open.
        self.open_tables(&from);

        // Begin one iteration per source, outer to inner. The sources are:
        //
        //  1. A table source begin is a Scan.
        //  2. A value source begin is an expression + Iter.
        //  3. An unpivot source begin is an expression + Entries + Iter.
        //
        // We enter a value/unpivot source on the expression so that we evaluate
        // it again. This is critical for correlated sources.
        //
        //  - entry[i] is the entry target for the enclosing Next instruction.
        //  - begin[i] is the exhaust instruction to patch once `exit` is known.
        //
        let mut entry = vec![0usize; n];
        let mut begin = vec![0usize; n];
        for (i, f) in from.into_iter().enumerate() {
            entry[i] = self.cc_source_begin(loop_csr[i], f)?;
            begin[i] = self.pc();
        }

        // Innermost body: seed unpivot bindings, predicate filter, then
        // offset/limit, then projection.
        let body = self.code.len();
        self.cc_seed(&seeds);
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
            self.emit_next(loop_csr[i], resume);
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

    /// Compiles a PIVOT query: fold every surviving binding tuple's `name: value`
    /// into one accumulator object, then yield it once.
    ///
    /// The accumulator lives at the bottom of the stack across the nested loop
    /// (mirroring the ORDER BY collector). Per tuple, `ObjSet` writes the dynamic
    /// `name: value` member; after the outermost source exhausts (or was empty)
    /// the single object is yielded — so an empty stream produces one `{}`.
    /// v1 supports `from` + `where`; `order by`/`limit` are rejected.
    #[allow(clippy::too_many_lines)]
    fn cc_pivot(&mut self, select: Select) -> Result<()> {
        self.ensure_txn(TransactionMode::Read);
        let Select {
            from,
            where_,
            group,
            having,
            order,
            limit,
            select: constructor,
        } = select;
        let Constructor::Pivot(pivot) = constructor else {
            unreachable!("cc_pivot requires a pivot constructor");
        };
        if order.is_some() || limit.is_some() || group.is_some() || having.is_some() {
            unsupported!("pivot does not support group by, having, order by, or limit");
        }
        if from.is_empty() {
            unsupported!("pivot requires a from clause");
        }

        // The accumulator object sits at the bottom of the stack for the whole
        // loop; each tuple sets one dynamic member on it.
        self.emit_obj();

        let loop_csr = Self::loop_cursors(&from);
        let seeds = Self::unpivot_seeds(&from);
        let n = from.len();

        self.open_tables(&from);

        let mut entry = vec![0usize; n];
        let mut begin = vec![0usize; n];
        for (i, f) in from.into_iter().enumerate() {
            entry[i] = self.cc_source_begin(loop_csr[i], f)?;
            begin[i] = self.pc();
        }

        // Body: seed unpivot bindings, filter, then set obj[name] = value. The
        // stack order ObjSet wants is `obj name value`, so push name then value.
        let body = self.code.len();
        self.cc_seed(&seeds);
        let mut where_fail = None;
        if let Some(where_) = where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            where_fail = Some(self.pc());
        }
        self.cc_expr(*pivot.name)?;
        self.cc_expr(*pivot.value)?;
        self.emit_obj_set();

        // Close the loops inner to outer (mirrors cc_select).
        let mut next_pc = vec![0usize; n];
        for i in (0..n).rev() {
            let resume = if i + 1 < n { entry[i + 1] } else { body };
            self.emit_next(loop_csr[i], resume);
            next_pc[i] = self.pc();
        }

        // After the outermost source exhausts (or was empty) we yield the one
        // accumulated object. Both the initial-empty edge (begin[0]) and the
        // exhausted edge (next[0] falling through) land on this Yield.
        self.emit_yield();
        let yield_pc = self.pc();

        self.patch(begin[0], yield_pc)?;
        for i in 1..n {
            self.patch(begin[i], next_pc[i - 1])?;
        }
        let inner = next_pc[n - 1];
        if let Some(pc) = where_fail {
            self.patch(pc, inner)?;
        }
        Ok(())
    }

    /// The cursor each from-source advances: a table scan, a value iterator, or
    /// (for unpivot) the attribute-value pair iterator at [`crate::ir::From::csr`].
    fn loop_cursors(from: &[crate::ir::From]) -> Vec<usize> {
        from.iter()
            .map(|f| f.csr.expect("from item should be bound") as usize)
            .collect()
    }

    /// The flattened projection environment a from clause exposes, in binding
    /// order: one binding per table/value source; for an unpivot, its value
    /// binding plus its optional attribute-name binding.
    fn from_bindings(from: &[crate::ir::From]) -> Vec<(String, usize)> {
        let mut bindings = Vec::new();
        for f in from {
            if let Source::Unpivot(u) = &f.src {
                bindings.push((
                    f.var.clone(),
                    u.val_csr.expect("unpivot value cursor") as usize,
                ));
                if let Some(att) = &u.att {
                    bindings.push((
                        att.clone(),
                        u.att_csr.expect("unpivot attribute cursor") as usize,
                    ));
                }
            } else {
                bindings.push((
                    f.var.clone(),
                    f.csr.expect("from item should be bound") as usize,
                ));
            }
        }
        bindings
    }

    /// The `(pair, value, attr)` cursor triples of the unpivot sources: the pair
    /// iterator, the value binding, and the optional attribute-name binding.
    fn unpivot_seeds(from: &[crate::ir::From]) -> Vec<(usize, usize, Option<usize>)> {
        from.iter()
            .filter_map(|f| {
                let Source::Unpivot(u) = &f.src else {
                    return None;
                };
                Some((
                    f.csr.expect("unpivot pair cursor") as usize,
                    u.val_csr.expect("unpivot value cursor") as usize,
                    u.att_csr.map(|c| c as usize),
                ))
            })
            .collect()
    }

    /// Opens every table source's btree once before the loop; value and unpivot
    /// sources need no open.
    fn open_tables(&mut self, from: &[crate::ir::From]) {
        for f in from {
            if let Source::Table(_) = &f.src {
                let csr = f.csr.expect("from item should be bound") as usize;
                let oid = f.oid.expect("bind pass must set oid for Table");
                self.emit_open(csr, oid);
            }
        }
    }

    /// Emits one from-source's begin block and returns its `entry` target (where
    /// the enclosing Next resumes). A table begins on its Scan; a value or
    /// unpivot source begins on its expression so a correlated source
    /// re-evaluates it. An unpivot expands the tuple to `[name, value]` pairs
    /// with `Entries` before iterating.
    fn cc_source_begin(&mut self, csr: usize, f: crate::ir::From) -> Result<usize> {
        let entry = match f.src {
            Source::Table(_) => {
                self.emit_scan(csr, 0);
                self.pc()
            }
            Source::Value(expr) => {
                let entry = self.pc() + 1;
                self.cc_expr(*expr)?;
                self.emit_iter(csr, 0);
                entry
            }
            Source::Unpivot(u) => {
                let entry = self.pc() + 1;
                self.cc_expr(*u.expr)?;
                self.emit_entries();
                self.emit_iter(csr, 0);
                entry
            }
        };
        Ok(entry)
    }

    /// Emits the seed run at the top of the loop body: re-derive each unpivot
    /// source's value (and attribute-name) binding from its current `[name,
    /// value]` pair, so the projection reads them via the same `LoadVal` as any
    /// scanned binding.
    fn cc_seed(&mut self, seeds: &[(usize, usize, Option<usize>)]) {
        for &(pair, val, att) in seeds {
            self.emit_load(pair);
            self.emit_jpi(1);
            self.emit_set_val(val);
            if let Some(att) = att {
                self.emit_load(pair);
                self.emit_jpi(0);
                self.emit_set_val(att);
            }
        }
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
        let Select {
            from,
            where_,
            order,
            limit,
            select: constructor,
            ..
        } = select;
        let order = order.expect("cc_order requires an order clause");
        let dirs: Vec<bool> = order.keys.iter().map(|k| k.desc).collect();

        // `loop_csr` is the cursor each source advances; `bindings` is the
        // flattened projection environment (an unpivot contributes value + name).
        let loop_csr = Self::loop_cursors(&from);
        let bindings = Self::from_bindings(&from);
        let seeds = Self::unpivot_seeds(&from);
        let n = from.len();

        // Register every from-cursor (iterators and bindings), then allocate the
        // phase-2 payload cursor so it can't collide with them.
        for &csr in &loop_csr {
            self.use_cursor(csr);
        }
        for (_, csr) in &bindings {
            self.use_cursor(*csr);
        }
        let payload_csr = self.alloc_cursor();

        // The collector array lives at the bottom of the stack across phase 1.
        self.emit_arr();

        // Open table sources once before the loop; value/unpivot sources don't.
        self.open_tables(&from);

        // Begin one iteration per source, outer to inner (mirrors cc_select).
        let mut entry = vec![0usize; n];
        let mut begin = vec![0usize; n];
        for (i, f) in from.into_iter().enumerate() {
            entry[i] = self.cc_source_begin(loop_csr[i], f)?;
            begin[i] = self.pc();
        }

        // Innermost body: seed unpivot bindings, residual filter, then tag + collect.
        let body = self.code.len();
        self.cc_seed(&seeds);
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
            self.emit_next(loop_csr[i], resume);
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
        for &csr in &loop_csr {
            self.emit_close(csr);
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

    /// Compiles an ungrouped aggregate query (`count`/`sum`/`min`/`max`/`avg`).
    ///
    /// It scans the same `from`/`where` nested loop as `cc_select`, but the loop
    /// body folds each row into accumulators (`AggStep`) instead of yielding, and
    /// the outermost source's exhaust edge lands on the finalize block — so an
    /// empty input still finalizes and yields exactly one row (SQLite's
    /// `OP_Rewind → AggFinal`). The finalize block applies the limit to that one
    /// row, projects (each `Expr::Agg` → `AggFinal`), and yields.
    ///
    ///   addr  instruction              comment
    ///   ----  -----------              -------
    ///   ...   AggInit{0,..}            reset each accumulator
    ///         [CntSet…]                init limit counters (if any)
    ///         Open / Scan{jmp:FIN}     empty source -> FIN (not past it)
    ///           [where IfNot -> Next]
    ///           [arg]; AggStep{0,..}   fold each aggregate
    ///         Next -> body
    ///   FIN:  [limit checks]           slice the single row
    ///         [AggFinal…] / project
    ///         Yield
    #[allow(clippy::too_many_lines)]
    fn cc_aggregate(&mut self, select: Select) -> Result<()> {
        if select.order.is_some() {
            unsupported!("order by of an aggregate query is not supported");
        }
        let Select {
            from,
            where_,
            mut having,
            limit,
            select: mut constructor,
            ..
        } = select;

        if from.is_empty() {
            unsupported!("aggregate requires a from clause");
        }

        // Collect the aggregate terms from the projection and HAVING: each gets
        // an accumulator slot (written back into its `Expr::Agg`), and any bare
        // column reference mixed in is rejected (undefined without GROUP BY).
        let mut collector = AggCollect {
            compiler: self,
            groups: &[],
            terms: vec![],
            bare: false,
        };
        collector.visit_constructor_mut(&mut constructor);
        if let Some(h) = &mut having {
            collector.visit_expr_mut(h);
        }
        if collector.bare {
            unsupported!("non-aggregate reference mixed with aggregates");
        }
        let terms = collector.terms;

        // Loop cursors / bindings handle table/value/unpivot sources uniformly.
        let loop_csr = Self::loop_cursors(&from);
        let bindings = Self::from_bindings(&from);
        let seeds = Self::unpivot_seeds(&from);
        let n = from.len();

        // Reset the accumulators and init the limit counters before the loop.
        for t in &terms {
            self.emit_agg_init(t.slot, t.kind);
        }
        let (cnt_skip, cnt_take) = self.emit_limit_counters(limit.as_ref());

        // Open table sources once before the loop; value/unpivot need no open.
        self.open_tables(&from);

        // Begin one iteration per source, outer to inner (mirrors cc_select).
        let mut entry = vec![0usize; n];
        let mut begin = vec![0usize; n];
        for (i, f) in from.into_iter().enumerate() {
            entry[i] = self.cc_source_begin(loop_csr[i], f)?;
            begin[i] = self.pc();
        }

        // Body: seed unpivot bindings, residual filter, then fold each aggregate
        // (consuming the terms — their last use).
        let body = self.code.len();
        self.cc_seed(&seeds);
        let mut where_fail = None;
        if let Some(where_) = where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            where_fail = Some(self.pc());
        }
        for t in terms {
            match t.arg {
                // count(*): push a non-null constant so AggStep counts the row.
                None => self.emit_push(Value::bool(true)),
                Some(arg) => self.cc_expr(arg)?,
            }
            self.emit_agg_step(t.slot, t.kind);
        }

        // Close the loops inner to outer (mirrors cc_select).
        let mut next_pc = vec![0usize; n];
        for i in (0..n).rev() {
            let resume = if i + 1 < n { entry[i + 1] } else { body };
            self.emit_next(loop_csr[i], resume);
            next_pc[i] = self.pc();
        }

        // The finalize block begins right after the Next instructions.
        let fin = self.pc() + 1;

        // Patch the exhaust edges: the outermost source exhausting falls into the
        // finalize block (so empty input still yields one row); an inner source
        // exhausting advances its enclosing one.
        self.patch(begin[0], fin)?;
        for i in 1..n {
            self.patch(begin[i], next_pc[i - 1])?;
        }
        let inner = next_pc[n - 1];
        if let Some(pc) = where_fail {
            self.patch(pc, inner)?;
        }

        // Finalize: HAVING (whole input as one group), the limit, then project.
        // A failed HAVING, spent skip, or exhausted take drops the one row.
        let (cont, stop) = self.cc_emit_group_yield(
            &constructor,
            having.as_ref(),
            &bindings,
            (cnt_skip, cnt_take),
        )?;
        let exit = self.pc() + 1;
        for pc in cont.into_iter().chain(stop) {
            self.patch(pc, exit)?;
        }
        Ok(())
    }

    /// Compiles a GROUP BY query: a two-pass sort (like `cc_order`) then a
    /// streaming pass that resets the accumulators at each group boundary
    /// (SQLite's sorter-based GROUP BY).
    ///
    /// Phase 1 scans the from/where stream and collects `[group_key_bytes,
    /// payload]` elements; phase 2 sorts them so each group is contiguous; phase
    /// 3 streams them through one shared accumulator bank, folding each row and
    /// flushing a finished group when the key changes (`GroupBreak`) and once
    /// more at the end. All group state lives in the agg bank: a transition slot
    /// holds the current key, a `First` slot the group's representative row (so
    /// the projection's group-key columns survive the boundary), and one slot
    /// per aggregate term. An empty input yields zero rows.
    ///
    ///   addr  instruction                comment
    ///   ----  -----------                -------
    ///   ...   AggInit prevkey/repr/aggs; CntSet limits
    ///         Arr; <scan builds [key, payload]>; Sort       phases 1-2
    ///         Iter{jmp:DONE}             empty input -> DONE (zero groups)
    ///   LOOP: LoadVal; Jpi 0
    ///         GroupBreak{jmp:STEP}       first/same -> STEP ; new group -> flush
    ///         [output prev group]; AggInit repr/aggs        reset
    ///   STEP: reseed; AggStep repr; fold aggs; Next{jmp:LOOP}
    ///         [output last group]
    ///   DONE: (Halt, appended by `compile`)
    #[allow(clippy::too_many_lines)]
    fn cc_group(&mut self, select: Select) -> Result<()> {
        self.ensure_txn(TransactionMode::Read);
        if select.order.is_some() {
            unsupported!("order by of a grouped query is not supported");
        }
        let Select {
            from,
            where_,
            group,
            mut having,
            limit,
            select: mut constructor,
            ..
        } = select;
        let group = group.expect("cc_group requires a group clause");
        if from.is_empty() {
            unsupported!("group by requires a from clause");
        }
        // A grouped query must project explicit keys/aggregates: `*`/`.` (the
        // whole binding tuple) and `pivot` have no defined value per group.
        if matches!(
            constructor,
            Constructor::None | Constructor::Star | Constructor::Pivot(_)
        ) {
            unsupported!("select * / select . is not supported with group by");
        }

        let group_keys = group.keys;

        // Group state slots in the agg bank: the transition key and the `First`
        // representative row.
        let prevkey_slot = self.alloc_agg();
        let repr_slot = self.alloc_agg();

        // Pull the aggregate terms out of the projection and HAVING; reject a
        // referenced column that is neither a group key nor inside an aggregate.
        let mut collector = AggCollect {
            compiler: self,
            groups: &group_keys,
            terms: vec![],
            bare: false,
        };
        collector.visit_constructor_mut(&mut constructor);
        if let Some(h) = &mut having {
            collector.visit_expr_mut(h);
        }
        if collector.bare {
            unsupported!("a grouped projection may only reference group keys and aggregates");
        }
        let terms = collector.terms;

        // Loop cursors / bindings handle table/value/unpivot sources uniformly.
        let loop_csr = Self::loop_cursors(&from);
        let bindings = Self::from_bindings(&from);
        let seeds = Self::unpivot_seeds(&from);
        let n = from.len();

        // Register from-cursors, then the phase-3 payload (sorted scan) and
        // representative-row cursors so they can't collide.
        for &csr in &loop_csr {
            self.use_cursor(csr);
        }
        for (_, csr) in &bindings {
            self.use_cursor(*csr);
        }
        let payload_csr = self.alloc_cursor();
        let repr_csr = self.alloc_cursor();

        // Reset the bank and init the limit counters before the loop.
        self.emit_agg_init(prevkey_slot, AggKind::First);
        self.emit_group_reset(repr_slot, &terms);
        let (cnt_skip, cnt_take) = self.emit_limit_counters(limit.as_ref());

        // ---- Phase 1: collect [group_key_bytes, payload] (mirrors cc_order) ----
        self.emit_arr();
        self.open_tables(&from);

        let mut entry = vec![0usize; n];
        let mut begin = vec![0usize; n];
        for (i, f) in from.into_iter().enumerate() {
            entry[i] = self.cc_source_begin(loop_csr[i], f)?;
            begin[i] = self.pc();
        }

        let body = self.code.len();
        self.cc_seed(&seeds);
        let mut where_fail = None;
        if let Some(where_) = where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            where_fail = Some(self.pc());
        }

        // Build the tagged element [group_key_bytes, payload]. Group order is
        // ascending; direction is irrelevant to grouping, only to output order.
        let dirs = vec![false; group_keys.len()];
        self.emit_arr();
        for k in group_keys {
            self.cc_expr(k)?;
        }
        self.emit_order_key(dirs);
        self.emit_arr_push();
        self.cc_select_constructor(Constructor::None, &bindings)?;
        self.emit_arr_push();
        self.emit_arr_push();

        let mut next_pc = vec![0usize; n];
        for i in (0..n).rev() {
            let resume = if i + 1 < n { entry[i + 1] } else { body };
            self.emit_next(loop_csr[i], resume);
            next_pc[i] = self.pc();
        }

        // Patch the phase-1 exhaust edges, then close the read iterators and sort.
        let sort_pc = self.pc() + 1;
        self.patch(begin[0], sort_pc)?;
        for i in 1..n {
            self.patch(begin[i], next_pc[i - 1])?;
        }
        let inner = next_pc[n - 1];
        if let Some(pc) = where_fail {
            self.patch(pc, inner)?;
        }
        for &csr in &loop_csr {
            self.emit_close(csr);
        }
        self.emit_sort();

        // ---- Phase 3: grouped stream ----
        self.emit_iter(payload_csr, 0);
        let iter_pc = self.pc();
        let loop_top = self.code.len();

        // Compare the current element's key (element[0]) to the group's key.
        self.emit_load(payload_csr);
        self.emit_jpi(0);
        self.emit_group_break(prevkey_slot, 0);
        let break_pc = self.pc();

        // Flush the group that just ended (reached only when GroupBreak detects a
        // new group and falls through).
        let (cont1, stop1) = self.cc_group_output(
            &constructor,
            having.as_ref(),
            repr_csr,
            repr_slot,
            &bindings,
            (cnt_skip, cnt_take),
        )?;
        // Reset the representative row and accumulators for the new group; the
        // transition slot already holds its key.
        let reset_pc = self.code.len();
        self.emit_group_reset(repr_slot, &terms);

        // Step block: fold the current row into the group.
        let step_pc = self.code.len();
        for (alias, csr) in &bindings {
            self.emit_load(payload_csr);
            self.emit_jpi(1);
            self.emit_jpk(alias.clone());
            self.emit_set_val(*csr);
        }
        // Keep the group's representative row (the first one folded).
        self.emit_load(payload_csr);
        self.emit_jpi(1);
        self.emit_agg_step(repr_slot, AggKind::First);
        // Fold each aggregate's argument (consuming the terms — their last use).
        for t in terms {
            match t.arg {
                None => self.emit_push(Value::bool(true)),
                Some(arg) => self.cc_expr(arg)?,
            }
            self.emit_agg_step(t.slot, t.kind);
        }
        self.emit_next(payload_csr, loop_top);

        // Flush the final group after the scan exhausts.
        let (cont2, stop2) = self.cc_group_output(
            &constructor,
            having.as_ref(),
            repr_csr,
            repr_slot,
            &bindings,
            (cnt_skip, cnt_take),
        )?;
        let done = self.pc() + 1;

        // Patch the control edges now that every target is known.
        self.patch(iter_pc, done)?; // empty collector -> DONE
        self.patch(break_pc, step_pc)?; // first row / same group -> STEP
        // A failed HAVING or spent skip drops the group's row: in the break
        // flush it still resets and steps the new row; in the final flush it ends.
        for pc in cont1 {
            self.patch(pc, reset_pc)?;
        }
        for pc in cont2 {
            self.patch(pc, done)?;
        }
        // An exhausted take ends the whole stream.
        for pc in stop1.into_iter().chain(stop2) {
            self.patch(pc, done)?;
        }
        Ok(())
    }

    /// Emits a HAVING guard: evaluate the predicate and, when it is false, jump
    /// to a caller-patched drop-group target. Returns the `IfNot` patch site (or
    /// `None` when there is no HAVING). Shared by the grouped and ungrouped
    /// aggregate finalize paths.
    fn cc_having_guard(&mut self, having: Option<&Expr>) -> Result<Option<usize>> {
        let Some(h) = having else {
            return Ok(None);
        };
        self.cc_expr(h.clone())?;
        self.emit_if_not(0);
        Ok(Some(self.pc()))
    }

    /// Emits one group's output: re-seed the bindings from the group's
    /// representative row, apply HAVING and the limit, then project and yield.
    /// Returns the jump sites to back-patch — `cont` (HAVING failed or skip not
    /// spent: drop this group's row but keep going) and `stop` (take exhausted:
    /// end the stream). Called at each flush site (group boundary and end), so it
    /// clones the projection/HAVING it emits.
    fn cc_group_output(
        &mut self,
        constructor: &Constructor,
        having: Option<&Expr>,
        repr_csr: usize,
        repr_slot: usize,
        bindings: &[(String, usize)],
        limit: (Option<usize>, Option<usize>),
    ) -> Result<(Vec<usize>, Vec<usize>)> {
        // Re-seed each binding from the group's representative row so the
        // projection and HAVING read its group-key columns via the same LoadVal
        // as a live scan; aggregates read the accumulator bank.
        self.emit_agg_final(repr_slot, AggKind::First);
        self.emit_set_val(repr_csr);
        for (alias, csr) in bindings {
            self.emit_load(repr_csr);
            self.emit_jpk(alias.clone());
            self.emit_set_val(*csr);
        }
        // HAVING + limit + project + yield, shared with the ungrouped path.
        self.cc_emit_group_yield(constructor, having, bindings, limit)
    }

    /// Emits the tail of a finalize block: a HAVING guard, the limit guards, then
    /// the projection and Yield. Returns the jump sites to back-patch — `cont`
    /// (HAVING failed or skip not yet spent: drop this row, keep going) and `stop`
    /// (take exhausted: end output). Shared by `cc_aggregate` (the whole input as
    /// one group) and `cc_group_output` (one row per group). HAVING runs before
    /// the limit, so a dropped group does not consume it.
    fn cc_emit_group_yield(
        &mut self,
        constructor: &Constructor,
        having: Option<&Expr>,
        bindings: &[(String, usize)],
        limit: (Option<usize>, Option<usize>),
    ) -> Result<(Vec<usize>, Vec<usize>)> {
        let mut cont = vec![];
        let mut stop = vec![];
        if let Some(pc) = self.cc_having_guard(having)? {
            cont.push(pc);
        }
        let (offset, limit_pc) = self.emit_limit_checks(limit.0, limit.1);
        if let Some(pc) = offset {
            cont.push(pc);
        }
        if let Some(pc) = limit_pc {
            stop.push(pc);
        }
        self.cc_select_constructor(constructor.clone(), bindings)?;
        self.emit_yield();
        Ok((cont, stop))
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
            // PIVOT is a whole-stream fold, lowered by cc_pivot — it never
            // reaches the per-tuple projection path.
            Constructor::Pivot(_) => unreachable!("pivot is lowered by cc_pivot"),
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
            | Vop::CntIfZero(_, jmp)
            | Vop::GroupBreak { slot: _, jmp } => *jmp = dst,
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
            }
            Expr::Obj(obj) => self.cc_expr_obj(obj),
            Expr::Array(items) => self.cc_expr_array(items),
            Expr::Var(var) => {
                self.cc_expr_var(&var);
                Ok(())
            }
            // Binding already lowered a full-key table subscript to this node;
            // we encode the literal key and emit the point lookup.
            Expr::Get(get) => self.cc_expr_get(&get),
            // cc_aggregate assigned this term's slot; emit its finalized value.
            // (Reaching here outside cc_aggregate is a compiler invariant break.)
            Expr::Agg(agg) => {
                let slot = agg.slot.expect("aggregate slot assigned by cc_aggregate");
                self.emit_agg_final(slot, agg.kind);
                Ok(())
            }
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

    /// Compiles a builtin call. A built-in operator (arithmetic, comparison, 3VL
    /// logic, `between`, `in_list`) compiles to its dedicated opcode; any other
    /// name resolves against the `functions` standard-library registry and
    /// compiles to a generic `Vop::Call`. An unknown name or bad arity errors.
    fn cc_expr_call(&mut self, call: Call) -> Result<()> {
        let Call { name, args } = call;
        // Built-in operators compile to dedicated opcodes (hot path, special
        // promotion/3VL semantics live in the VM).
        if let Some((arity_ok, op)) = operator_op(&name, args.len()) {
            if !arity_ok {
                return Err(Error::UnknownFunction(name));
            }
            for arg in args {
                self.cc_expr(arg)?;
            }
            self.code.push(op);
            return Ok(());
        }
        // Otherwise resolve against the standard-library registry.
        match functions::lookup(&name) {
            Some(fun) if functions::arity_ok(fun, args.len()) => {
                let cnt = args.len();
                for arg in args {
                    self.cc_expr(arg)?;
                }
                self.code.push(Vop::Call { fun, cnt });
                Ok(())
            }
            _ => Err(Error::UnknownFunction(name)),
        }
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

    /// Allocates and returns the next aggregate-accumulator slot.
    fn alloc_agg(&mut self) -> usize {
        let slot = self.agg_slots;
        self.agg_slots += 1;
        slot
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

    fn emit_obj_set(&mut self) {
        self.code.push(Vop::ObjSet);
    }

    fn emit_entries(&mut self) {
        self.code.push(Vop::Entries);
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

    fn emit_agg_init(&mut self, slot: usize, kind: AggKind) {
        self.code.push(Vop::AggInit { slot, kind });
    }

    fn emit_agg_step(&mut self, slot: usize, kind: AggKind) {
        self.code.push(Vop::AggStep { slot, kind });
    }

    fn emit_agg_final(&mut self, slot: usize, kind: AggKind) {
        self.code.push(Vop::AggFinal { slot, kind });
    }

    fn emit_group_break(&mut self, slot: usize, jmp: usize) {
        self.code.push(Vop::GroupBreak { slot, jmp });
    }

    /// Resets the per-group accumulators — the representative-row slot and each
    /// aggregate term — to their identities. Emitted before the loop and again at
    /// every group boundary (the transition-key slot is not reset; it carries the
    /// new group's key).
    fn emit_group_reset(&mut self, repr_slot: usize, terms: &[AggTerm]) {
        self.emit_agg_init(repr_slot, AggKind::First);
        for t in terms {
            self.emit_agg_init(t.slot, t.kind);
        }
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

/// One aggregate term collected from a projection: its accumulator slot, kind,
/// and (taken) argument expression — `None` for `count(*)`.
struct AggTerm {
    slot: usize,
    kind: AggKind,
    arg: Option<Expr>,
}

/// A `Visit` that trips the first time it reaches an aggregate term.
#[derive(Default)]
struct AggScan(bool);

impl<'ast> Visit<'ast> for AggScan {
    fn visit_expr(&mut self, e: &'ast Expr) {
        if matches!(e, Expr::Agg(_)) {
            self.0 = true;
        }
        visit::visit_expr(self, e);
    }
}

/// Whether a projection contains any aggregate term. Drives the `cc_select`
/// dispatch to `cc_aggregate`. Reuses the visitor so it can never disagree with
/// [`AggCollect`] about which expressions hold an aggregate.
fn has_aggregate(constructor: &Constructor) -> bool {
    let mut scan = AggScan::default();
    scan.visit_constructor(constructor);
    scan.0
}

/// A `VisitMut` over a projection (and HAVING) that pulls out each aggregate
/// term — assigning its accumulator slot and taking its argument — and flags a
/// bare from-binding reference (a per-row value, undefined in an aggregate
/// query). Under GROUP BY, `groups` holds the grouping key expressions: a
/// subexpression equal to a group key is accepted and left as-is (it reads the
/// group's representative row at finalize time), so it neither needs an
/// accumulator nor trips the bare flag. With an empty `groups` (ungrouped
/// aggregation) any column reference is bare. Reusing the visitor keeps it in
/// lockstep with [`has_aggregate`] and every other `Expr` walk.
struct AggCollect<'c, 'g> {
    compiler: &'c mut Compiler,
    groups: &'g [Expr],
    terms: Vec<AggTerm>,
    bare: bool,
}

impl VisitMut for AggCollect<'_, '_> {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        // A whole subexpression that is a group key projects as-is — don't
        // recurse (its inner from-bindings are part of the key, not bare). The
        // match is structural `Expr` equality, which ignores binder-assigned
        // slots (`Get.csr`, `Var` resolves to a shared per-alias cursor) but not
        // source structure, so it relies on the binder not normalizing
        // expressions; revisit if expression rewriting is ever added.
        if self.groups.contains(&*e) {
            return;
        }
        match e {
            Expr::Agg(agg) => {
                // Reuse an identical aggregate's slot (same kind + argument) so
                // e.g. count(*) in both the projection and HAVING folds once.
                let existing = self
                    .terms
                    .iter()
                    .find(|t| t.kind == agg.kind && t.arg.as_ref() == agg.arg.as_deref())
                    .map(|t| t.slot);
                let slot = existing.unwrap_or_else(|| {
                    let slot = self.compiler.alloc_agg();
                    // The argument folds in the loop body, not the projection, so
                    // it is taken (and never recursed into — a from-binding inside
                    // an aggregate is fine).
                    self.terms.push(AggTerm {
                        slot,
                        kind: agg.kind,
                        arg: agg.arg.take().map(|a| *a),
                    });
                    slot
                });
                agg.slot = Some(slot);
            }
            // A from-binding reference has no per-row value at finalize time.
            Expr::Var(_) => self.bare = true,
            other => visit_mut::visit_expr_mut(self, other),
        }
    }
}

/// Maps a built-in operator name to its opcode, or `None` if `name` is not an
/// operator (in which case it is resolved as a standard-library function). The
/// returned bool reports whether `argc` is a valid arity for that operator.
#[allow(clippy::len_zero)]
fn operator_op(name: &str, argc: usize) -> Option<(bool, Vop)> {
    Some(match name {
        "*" => (argc == 2, Vop::Mul),
        "/" => (argc == 2, Vop::Div),
        "%" => (argc == 2, Vop::Rem),
        "+" => (argc == 2, Vop::Add),
        "-" => (argc == 2, Vop::Sub),
        "<" => (argc == 2, Vop::Lt),
        "<=" => (argc == 2, Vop::Le),
        "=" => (argc == 2, Vop::Eq),
        ">=" => (argc == 2, Vop::Ge),
        ">" => (argc == 2, Vop::Gt),
        "!=" => (argc == 2, Vop::Ne),
        "and" => (argc == 2, Vop::And),
        "or" => (argc == 2, Vop::Or),
        "not" => (argc == 1, Vop::Not),
        "is_null" => (argc == 1, Vop::IsNull),
        "is_true" => (argc == 1, Vop::IsTrue),
        "is_false" => (argc == 1, Vop::IsFalse),
        "is_unknown" => (argc == 1, Vop::IsUnknown),
        "between" => (argc == 3, Vop::Between),
        "in_list" => (argc >= 1, Vop::InList(argc.saturating_sub(1))),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binder::Binder;
    use crate::catalog::Catalog;
    use crate::lexer::SqlLexer;
    use crate::parser::SqlParser;
    use crate::storage::Storage;
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
        assert!(matches!(
            code[7],
            Vop::Transaction {
                txm: TransactionMode::Read
            }
        ));
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
        let program = compile_sql(
            &storage,
            &catalog,
            "select * from catalog order by catalog.name;",
        );
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
        assert!(matches!(
            code[26],
            Vop::Transaction {
                txm: TransactionMode::Read
            }
        ));
        assert!(matches!(code[27], Vop::Jump { jmp: 1 }));
    }

    #[test]
    fn count_star_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select count(*) from catalog;");
        // One scan cursor and one accumulator slot.
        assert_eq!(program.cursors, 1);
        assert_eq!(program.aggs, 1);
        let code = program.instructions;
        assert_eq!(code.len(), 12);
        assert!(matches!(code[0], Vop::Init { jmp: 10 }));
        assert!(matches!(
            code[1],
            Vop::AggInit {
                slot: 0,
                kind: AggKind::Count
            }
        ));
        assert!(matches!(code[2], Vop::Open { csr: 0, tbl: 0 }));
        // The load-bearing invariant: an empty table jumps to the FINALIZE block
        // (so one row still comes out), NOT past the Yield.
        assert!(matches!(code[3], Vop::Scan { csr: 0, jmp: 7 }));
        assert!(matches!(code[4], Vop::Push { .. })); // count(*) non-null constant
        assert!(matches!(
            code[5],
            Vop::AggStep {
                slot: 0,
                kind: AggKind::Count
            }
        ));
        assert!(matches!(code[6], Vop::Next { csr: 0, jmp: 4 }));
        assert!(matches!(
            code[7],
            Vop::AggFinal {
                slot: 0,
                kind: AggKind::Count
            }
        ));
        assert!(matches!(code[8], Vop::Yield));
        assert!(matches!(code[9], Vop::Halt));
        // Prove the scan's exhaust target really is the finalize block.
        let Vop::Scan { jmp, .. } = code[3] else {
            panic!("expected Scan")
        };
        assert!(matches!(code[jmp], Vop::AggFinal { .. }));
    }

    #[test]
    fn sum_expr_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select sum(catalog.name) from catalog;");
        assert_eq!(program.aggs, 1);
        let code = program.instructions;
        assert!(matches!(
            code[1],
            Vop::AggInit {
                slot: 0,
                kind: AggKind::Sum
            }
        ));
        assert!(matches!(code[3], Vop::Scan { csr: 0, jmp: 8 }));
        // The body compiles the argument (catalog.name), then folds it.
        assert!(matches!(code[4], Vop::LoadVal { csr: 0 }));
        assert!(matches!(code[5], Vop::Jpk(_)));
        assert!(matches!(
            code[6],
            Vop::AggStep {
                slot: 0,
                kind: AggKind::Sum
            }
        ));
        assert!(matches!(code[7], Vop::Next { csr: 0, jmp: 4 }));
        assert!(matches!(
            code[8],
            Vop::AggFinal {
                slot: 0,
                kind: AggKind::Sum
            }
        ));
        assert!(matches!(code[9], Vop::Yield));
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
        assert!(matches!(
            code[13],
            Vop::Transaction {
                txm: TransactionMode::Write
            }
        ));
        assert!(matches!(code[14], Vop::Jump { jmp: 1 }));
    }

    #[test]
    fn delete_where_bytecode_shape() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(
            &storage,
            &catalog,
            "delete from catalog where catalog.name = 'x';",
        );
        let code = program.instructions;
        // Phase 1: scan exits to Close; a false predicate skips the collect.
        assert!(matches!(code[3], Vop::Scan { csr: 0, jmp: 12 }));
        assert!(matches!(code[4], Vop::LoadVal { csr: 0 })); // predicate reads the row
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
        assert!(matches!(
            code[7],
            Vop::Transaction {
                txm: TransactionMode::Write
            }
        ));
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
            Key {
                name: "a".into(),
                ty: Type::Int,
            },
            Key {
                name: "b".into(),
                ty: Type::String,
            },
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

    #[test]
    fn unpivot_lowers_to_entries_iter_and_seed() {
        // `unpivot E as v at a` expands the tuple with Entries, iterates the
        // pairs, and seeds the value + attribute bindings (two SetVals) from the
        // current `[name, value]` pair at the top of the body.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(
            &storage,
            &catalog,
            "select price from unpivot {a: 1, b: 2} as price at sym;",
        );
        let code = program.instructions;
        let entries = code
            .iter()
            .position(|op| matches!(op, Vop::Entries))
            .expect("unpivot must emit Entries");
        let iter = code
            .iter()
            .position(|op| matches!(op, Vop::Iter { .. }))
            .expect("unpivot must iterate the pair array");
        assert!(entries < iter, "Entries must precede Iter");
        let setvals = code
            .iter()
            .filter(|op| matches!(op, Vop::SetVal { .. }))
            .count();
        assert_eq!(setvals, 2, "value and attribute bindings are each seeded");
    }

    #[test]
    fn pivot_folds_into_a_single_object() {
        // `pivot v at a` opens one accumulator object before the loop, sets a
        // dynamic member per row (ObjSet), and yields exactly once.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "pivot c.name at c.name from catalog as c;");
        let code = program.instructions;
        // The accumulator is the first body instruction (after Init).
        assert!(matches!(code[1], Vop::Obj));
        assert!(
            code.iter().any(|op| matches!(op, Vop::ObjSet)),
            "pivot must set members dynamically"
        );
        let yields = code.iter().filter(|op| matches!(op, Vop::Yield)).count();
        assert_eq!(yields, 1, "pivot yields exactly one object");
    }
}
