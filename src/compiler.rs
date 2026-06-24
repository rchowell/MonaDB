use serde_json::json;

use crate::Result;
use crate::catalog::CATALOG_OID;
use crate::error::Error;
use crate::unsupported;
use crate::functions;
use crate::ir::{
    AggKind, Call, Clear, CmpOp, Constructor, Copy, CopySource, Create, Delete, Drop, Expr, Get,
    Insert, Jpe, Jpi, Jpk, Key, Limit, Member, Obj, Select, Source, Statement, TableDefinition,
    ToSql, Type, Var,
};
use crate::read::{self, FileFormat};
use crate::schema;
use crate::transaction::TransactionMode;
use crate::value::Value;
use crate::visitor::visit::{self, Visit};
use crate::visitor::visit_mut::{self, VisitMut};
use crate::vm::{Program, ScanPrefix, Vop};

/// The open back-patch sites a nested loop leaves for its caller: `begin0`
/// (the outermost source's exhaust edge — its target is variant-specific)
/// and `inner` (the innermost `Next`, where a dropped row resumes).
struct LoopExits {
    begin0: usize,
    inner: usize,
}

/// The state threaded from [`Compiler::cc_loop_open`] to [`Compiler::cc_loop_close`].
struct LoopFrame {
    loop_csr: Vec<usize>,      // cursor each source advances (outer→inner)
    entry: Vec<usize>,         // re-entry target per source (for the enclosing Next)
    begin: Vec<usize>,         // exhaust-edge patch site per source
    body: usize,               // first instruction of the per-iteration body
    where_fail: Option<usize>, // IfNot patch site of the WHERE guard, if any
}

/// The per-source analysis of a from clause, computed in one pass: the cursor
/// each source advances, the flattened projection bindings, and the unpivot
/// reseed triples.
struct FromPlan {
    /// Cursor each source advances (outer→inner).
    loop_csr: Vec<usize>,
    /// Flattened projection environment: `(alias, cursor)` in binding order.
    bindings: Vec<(String, usize)>,
    /// `(pair, value, attr)` per unpivot source; empty when no unpivots.
    seeds: Vec<(usize, usize, Option<usize>)>,
}

/// Where a SELECT plan sends each output row: `Yield`ed to the caller (a
/// top-level statement) or `Collect`ed onto a collector array (a subquery
/// materializing its bag of rows on the stack).
#[derive(Default, Clone, Copy, PartialEq)]
enum Sink {
    #[default]
    Yield,
    Collect,
}

/// Translates a bound SQL statement into a `Program` of `Vop` bytecode.
#[derive(Default)]
pub struct Compiler {
    code: Vec<Vop>,
    /// Number of cursor slots required (max index + 1).
    cursor_slots: usize,
    /// Number of counter slots required (one per allocated counter).
    counter_slots: usize,
    /// Number of aggregate-accumulator slots required (one per aggregate term).
    agg_slots: usize,
    /// Where each SELECT plan sends its output rows (see [`Sink`]). A subquery
    /// flips this to `Collect` while compiling, then restores it.
    sink: Sink,
    /// The transaction mode this program requires
    txm: Option<TransactionMode>,
    /// Set when the statement changes catalog membership (CREATE/DROP).
    mutates_catalog: bool,
}

impl Compiler {
    /// Creates an empty compiler.
    pub fn new() -> Self {
        Self::default()
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
            Statement::Copy(copy) => self.cc_copy(copy)?,
            Statement::Create(create) => {
                self.mutates_catalog = true;
                self.cc_create(create)?;
            }
            Statement::Delete(delete) => self.cc_delete(delete)?,
            Statement::Drop(drop) => {
                self.mutates_catalog = true;
                self.cc_drop(drop);
            }
            Statement::Clear(clear) => self.cc_clear(clear),
            Statement::Insert(insert) => self.cc_insert(insert)?,
            Statement::Select(select) => self.cc_select(select)?,
            Statement::Begin | Statement::Commit | Statement::Rollback => {
                crate::error!("transaction control is handled before compilation");
            }
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
            mutates_catalog: self.mutates_catalog,
            instructions: self.code,
            // Resolved at prepare time; `compile` has no transaction to open them.
            tables: Vec::new(),
        })
    }

    /// Compiles CREATE TABLE or CREATE TABLE AS SELECT.
    fn cc_create(&mut self, create: Create) -> Result<()> {
        match create {
            Create::Table(def) => self.cc_create_table(def),
            Create::TableAs { def, query } => self.cc_create_table_as(def, query),
        }
    }

    /// Compiles a CREATE TABLE: insert the table's definition into the catalog.
    fn cc_create_table(&mut self, table_definition: TableDefinition) -> Result<()> {
        self.txm = Some(TransactionMode::Write);

        let sql = Create::Table(table_definition.clone()).sql();

        for member in &table_definition.keys {
            if !matches!(member.ty, Type::Int | Type::String) {
                unsupported!("key column '{}' must be int or string", member.name);
            }
        }

        let object = json!({
            "name": table_definition.name,
            "type": "table",
            "sql": sql,
        });

        let csr = self.alloc_cursor();
        self.emit_open(csr, CATALOG_OID);
        self.emit_push(object);
        self.emit_new_oid(csr);
        self.emit_new_btree();
        self.emit_insert(csr);
        Ok(())
    }

    /// Compiles CREATE TABLE AS SELECT: catalog entry, then bulk-insert query rows.
    fn cc_create_table_as(&mut self, def: TableDefinition, query: Select) -> Result<()> {
        self.txm = Some(TransactionMode::Write);
        for member in &def.keys {
            if !matches!(member.ty, Type::Int | Type::String) {
                unsupported!("key column '{}' must be int or string", member.name);
            }
        }

        let arr_csr = self.alloc_cursor();
        let oid_csr = self.alloc_cursor();
        let data_csr = self.alloc_cursor();
        let row_csr = self.alloc_cursor();

        self.cc_subquery_array(query)?;
        self.emit_set_val(arr_csr);

        let csr = self.alloc_cursor();
        let catalog_sql = Create::Table(def.clone()).sql();
        let object = json!({
            "name": def.name,
            "type": "table",
            "sql": catalog_sql,
        });
        self.emit_open(csr, CATALOG_OID);
        self.emit_push(object);
        self.emit_new_oid(csr);
        self.emit_dup();
        self.emit_set_val(oid_csr);
        self.emit_new_btree();
        self.emit_insert(csr);

        self.emit_load(oid_csr);
        self.emit_open_oid(data_csr);
        self.emit_load(arr_csr);
        self.cc_insert_from_cursor(data_csr, row_csr, &def.keys)?;

        Ok(())
    }

    /// Compiles COPY import or export.
    fn cc_copy(&mut self, copy: Copy) -> Result<()> {
        match copy {
            Copy::From {
                target,
                path,
                options,
            } => self.cc_copy_from(target, path, options),
            Copy::To {
                source,
                path,
                options,
            } => self.cc_copy_to(source, path, options),
        }
    }

    /// Compiles `COPY <table> FROM <file>`.
    fn cc_copy_from(
        &mut self,
        target: TableDefinition,
        path: String,
        options: Obj,
    ) -> Result<()> {
        self.ensure_txn(TransactionMode::Write);
        let tbl = target
            .oid
            .expect("copy target should be resolved to an oid");
        let keys = target.keys;

        let format = read::infer_format(&path)
            .ok_or_else(|| Error::Unsupported(format!("unsupported file format: {path}")))?;
        self.emit_read_call(path, format, options)?;

        let data_csr = self.alloc_cursor();
        let row_csr = self.alloc_cursor();
        self.emit_open(data_csr, tbl);
        self.cc_insert_from_cursor(data_csr, row_csr, &keys)?;
        Ok(())
    }

    /// Compiles `COPY <source> TO <file>`.
    fn cc_copy_to(
        &mut self,
        source: CopySource,
        path: String,
        options: Obj,
    ) -> Result<()> {
        self.ensure_txn(TransactionMode::Read);
        let hold_csr = self.alloc_cursor();
        let format = read::infer_format(&path)
            .ok_or_else(|| Error::Unsupported(format!("unsupported file format: {path}")))?;

        match source {
            CopySource::Table { oid, .. } => {
                let tbl = oid.expect("copy table source should be resolved to an oid");
                self.emit_arr();
                let tcsr = self.alloc_cursor();
                self.emit_open(tcsr, tbl);
                self.emit_scan(tcsr, 0);
                let scan = self.pc();
                let loop_top = self.code.len();
                self.emit_load(tcsr);
                self.emit_arr_push();
                self.emit_next(tcsr, loop_top);
                self.emit_close(tcsr);
                let _ = self.patch(scan, self.pc());
            }
            CopySource::Query(select) => {
                self.cc_subquery_array(select)?;
            }
        }

        self.emit_set_val(hold_csr);
        self.emit_push(Value::String(std::rc::Rc::from(path.as_str())));
        self.emit_load(hold_csr);
        self.cc_obj(options)?;
        self.emit_write_call(format)?;
        Ok(())
    }

    /// Emits a read builtin call leaving a row array on the stack.
    fn emit_read_call(&mut self, path: String, format: FileFormat, options: Obj) -> Result<()> {
        let builtin = read::read_builtin(format);
        let fun =
            functions::lookup(builtin).ok_or_else(|| Error::UnknownFunction(builtin.into()))?;
        self.emit_push(Value::String(std::rc::Rc::from(path)));
        self.cc_obj(options)?;
        self.code.push(Vop::Call { fun, cnt: 2 });
        Ok(())
    }

    /// Emits a write builtin call consuming path, rows, and options on the stack.
    fn emit_write_call(&mut self, format: FileFormat) -> Result<()> {
        let builtin = read::write_builtin(format);
        let fun =
            functions::lookup(builtin).ok_or_else(|| Error::UnknownFunction(builtin.into()))?;
        self.code.push(Vop::Call { fun, cnt: 3 });
        Ok(())
    }

    /// Compiles an object literal onto the stack.
    fn cc_obj(&mut self, obj: Obj) -> Result<()> {
        self.cc_expr(Expr::Obj(obj))
    }

    /// Inserts each row from `row_csr`'s array iteration into `data_csr`'s table.
    fn cc_insert_from_cursor(
        &mut self,
        data_csr: usize,
        row_csr: usize,
        keys: &[Key],
    ) -> Result<()> {
        self.emit_iter(row_csr, 0);
        let iter = self.pc();
        let loop_top = self.code.len();
        self.emit_load(row_csr);
        if keys.is_empty() {
            self.emit_new_oid(data_csr);
        } else {
            self.emit_encode_key(keys.to_vec());
        }
        self.emit_insert(data_csr);
        self.emit_next(row_csr, loop_top);
        let exit = self.pc() + 1;
        self.patch(iter, exit)?;
        self.emit_close(row_csr);
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
                self.emit_encode_key(members.clone());
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
    #[expect(
        clippy::needless_pass_by_value,
        reason = "cc_* take their IR node by ownership per convention; only the Copy oid is read"
    )]
    fn cc_drop(&mut self, drop: Drop) {
        self.ensure_txn(TransactionMode::Write);
        let Drop { oid, name: _ } = drop;
        let oid = oid.expect("drop target should be bound to table oid");

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
    #[expect(
        clippy::needless_pass_by_value,
        reason = "cc_* take their IR node by ownership per convention; only the Copy oid is read"
    )]
    fn cc_clear(&mut self, clear: Clear) {
        self.ensure_txn(TransactionMode::Write);
        let Clear { oid, name: _ } = clear;
        let oid = oid.expect("clear target should be bound to table oid");
        // Clearing a table empties its data btree but leaves the catalog row.
        self.emit_clear(oid);
    }

    //--- NESTED LOOP SCAFFOLD ---

    /// Emits a nested loop's prologue over `from`: opens table sources, emits
    /// each source's begin block (outer→inner), marks the body, seeds unpivot
    /// bindings, and emits the optional WHERE guard. The caller emits the body,
    /// then calls [`Self::cc_loop_close`].
    ///
    /// `fp` is the pre-computed [`FromPlan`] for `from` (computed once by the
    /// caller via [`analyze_from`] so it can also read `fp.bindings`/`fp.loop_csr`).
    fn cc_loop_open(
        &mut self,
        from: Vec<crate::ir::From>,
        fp: &FromPlan,
        where_: Option<Expr>,
    ) -> Result<LoopFrame> {
        let n = from.len();
        self.open_tables(&from);
        let mut entry = vec![0usize; n];
        let mut begin = vec![0usize; n];
        for (i, f) in from.into_iter().enumerate() {
            entry[i] = self.cc_source_begin(fp.loop_csr[i], f)?;
            begin[i] = self.pc();
        }
        let body = self.code.len();
        self.cc_seed(&fp.seeds);
        let mut where_fail = None;
        if let Some(where_) = where_ {
            self.cc_expr(where_)?;
            self.emit_if_not(0);
            where_fail = Some(self.pc());
        }
        Ok(LoopFrame { loop_csr: fp.loop_csr.clone(), entry, begin, body, where_fail })
    }

    /// Emits the inner→outer `Next` chain and patches the loop's inner exhaust
    /// edges: each inner source advances its enclosing source, and a failed
    /// WHERE drops the row back to the innermost `Next`. Returns the sites the
    /// caller still owns — `begin0` (the outermost exhaust, whose target depends
    /// on the query form) and `inner` (the innermost Next, for WHERE/offset drops).
    fn cc_loop_close(&mut self, frame: &LoopFrame) -> Result<LoopExits> {
        let n = frame.loop_csr.len();
        let mut next_pc = vec![0usize; n];
        for i in (0..n).rev() {
            let resume = if i + 1 < n { frame.entry[i + 1] } else { frame.body };
            self.emit_next(frame.loop_csr[i], resume);
            next_pc[i] = self.pc();
        }
        for i in 1..n {
            self.patch(frame.begin[i], next_pc[i - 1])?;
        }
        let inner = next_pc[n - 1];
        if let Some(pc) = frame.where_fail {
            self.patch(pc, inner)?;
        }
        Ok(LoopExits { begin0: frame.begin[0], inner })
    }

    //--- END NESTED LOOP SCAFFOLD ---

    /// Compiles a SELECT, streaming the nested-loop from/where/limit/project
    /// path. An `ORDER BY` can't stream, so it detours to [`Self::cc_order`].
    fn cc_select(&mut self, select: Select) -> Result<()> {
        self.ensure_txn(TransactionMode::Read);
        match plan(&select) {
            Plan::Group => return self.cc_group(select),
            Plan::Pivot => return self.cc_pivot(select),
            Plan::Aggregate => return self.cc_aggregate(select),
            Plan::Order => return self.cc_order(select),
            Plan::Stream => {}
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
            self.emit_row();
            return Ok(());
        }

        let fp = analyze_from(&from);
        let frame = self.cc_loop_open(from, &fp, where_)?;
        // --- body ---
        let (offset, limit_pc) = self.emit_limit_checks(cnt_skip, cnt_take);
        self.cc_select_constructor(constructor, &fp.bindings)?;
        self.emit_row();
        // --- close ---
        let exits = self.cc_loop_close(&frame)?;
        let exit = self.pc() + 1;
        self.patch(exits.begin0, exit)?;
        if let Some(pc) = offset {
            self.patch(pc, exits.inner)?;
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

        let fp = analyze_from(&from);
        let frame = self.cc_loop_open(from, &fp, where_)?;
        // --- body: set obj[name] = value (ObjSet wants `obj name value`) ---
        self.cc_expr(*pivot.name)?;
        self.cc_expr(*pivot.value)?;
        self.emit_obj_set();
        // --- close, then yield the accumulated object ---
        let exits = self.cc_loop_close(&frame)?;
        // After the outermost source exhausts (or was empty), yield the one
        // accumulated object. Both the initial-empty edge (begin0) and the
        // exhausted edge (Next falling through) land on this row emission.
        self.emit_row();
        self.patch(exits.begin0, self.pc())?;
        Ok(())
    }

    /// Opens every table source's btree once before the loop; value and unpivot
    /// sources need no open.
    fn open_tables(&mut self, from: &[crate::ir::From]) {
        for f in from {
            if let Source::Table(_) | Source::Range(_) = &f.src {
                let csr = f.csr.expect("from item should be bound") as usize;
                let oid = f.oid.expect("bind pass must set oid for Table/Range");
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
            Source::Range(g) => {
                // A partial-key keyed source: scan the btree prefix directly,
                // streaming rows like a table scan — no `GetRange` array. The
                // encoded prefix is left on the stack (literal keys at compile
                // time, parameter keys at run time, via the shared `emit_key_tuple`),
                // and the scan pops it.
                let Get { keys, args, .. } = g;
                self.emit_key_tuple(keys, args)?;
                self.emit_scan_from_stack(csr, 0);
                self.pc()
            }
            Source::Value(expr) => {
                let entry = self.pc() + 1;
                // A derived table iterates the subquery's whole bag, so emit the
                // array directly — not the scalar coercion `cc_expr` would apply.
                match *expr {
                    Expr::Subquery(select) => self.cc_subquery_array(*select)?,
                    other => self.cc_expr(other)?,
                }
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
    /// the spec, select runs after limit (4.9), so projection is post-sort.
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

        // Pre-compute the from plan before consuming `from`, so we can register
        // their cursors and allocate the phase-2 payload cursor without colliding
        // with any from-cursor.
        let fp = analyze_from(&from);
        for &csr in &fp.loop_csr {
            self.use_cursor(csr);
        }
        for (_, csr) in &fp.bindings {
            self.use_cursor(*csr);
        }
        let payload_csr = self.alloc_cursor();

        // The collector array lives at the bottom of the stack across phase 1.
        self.emit_arr();

        let frame = self.cc_loop_open(from, &fp, where_)?;
        // --- body: build the tagged element [order_key_bytes, payload] ---
        self.emit_arr();
        for k in order.keys {
            self.cc_expr(k.expr)?;
        }
        self.emit_order_key(dirs);
        self.emit_arr_push();
        // The payload is the binding tuple, exactly what `select .` builds.
        self.cc_select_constructor(Constructor::None, &fp.bindings)?;
        self.emit_arr_push();
        self.emit_arr_push();
        // --- close, then phase 2 ---
        let exits = self.cc_loop_close(&frame)?;
        let sort_pc = self.pc() + 1;
        self.patch(exits.begin0, sort_pc)?;
        // Phase 2: drop the read iterators, then sort the collector by key bytes.
        for &csr in &fp.loop_csr {
            self.emit_close(csr);
        }
        self.emit_sort();

        // Limit counters apply to the sorted stream (order then limit, 4.9).
        let (cnt_skip, cnt_take) = self.emit_limit_counters(limit.as_ref());

        // Iterate the sorted collector on the payload cursor.
        self.emit_iter(payload_csr, 0);
        let begin_payload = self.pc();
        let loop_top = self.code.len();

        // Limit: skip drops the row, take exhausted ends the scan.
        let (offset, limit_pc) = self.emit_limit_checks(cnt_skip, cnt_take);

        // Re-seed each from-binding from the payload (element[1]) so the select
        // constructor reads it via the same LoadVal as a live scan.
        self.emit_reseed_from_payload(payload_csr, &fp.bindings);

        // Project and emit.
        self.cc_select_constructor(constructor, &fp.bindings)?;
        self.emit_row();

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

        let fp = analyze_from(&from);

        // Reset the accumulators and init the limit counters before the loop.
        for t in &terms {
            self.emit_agg_init(t.slot, t.kind);
        }
        let (cnt_skip, cnt_take) = self.emit_limit_counters(limit.as_ref());

        let frame = self.cc_loop_open(from, &fp, where_)?;
        // --- body: fold each aggregate (consuming the terms — their last use) ---
        for t in terms {
            match t.arg {
                // count(*): push a non-null constant so AggStep counts the row.
                None => self.emit_push(Value::bool(true)),
                Some(arg) => self.cc_expr(arg)?,
            }
            self.emit_agg_step(t.slot, t.kind);
        }
        // --- close; outermost exhaust falls into the finalize block ---
        let exits = self.cc_loop_close(&frame)?;
        // The finalize block begins right after the Next instructions. An empty
        // source exhausting before any row still reaches AggFinal and yields one row.
        let fin = self.pc() + 1;
        self.patch(exits.begin0, fin)?;

        // Finalize: HAVING (whole input as one group), the limit, then project.
        // A failed HAVING, spent skip, or exhausted take drops the one row.
        let (cont, stop) = self.cc_emit_group_yield(
            &constructor,
            having.as_ref(),
            &fp.bindings,
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
    // The nested-loop scaffold is extracted (cc_loop_open/close); the residual
    // length is the irreducible 3-phase grouped-stream emission. `expect` (not
    // `allow`) so this fails the build once Phase 4 decomposes it under the limit.
    #[expect(clippy::too_many_lines, reason = "irreducible 3-phase grouped-stream emission; the nested-loop scaffold is already extracted")]
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

        // Pre-compute the from plan before consuming `from`, so we can register
        // their cursors and allocate the phase-3 payload/repr cursors without
        // colliding with any from-cursor.
        let fp = analyze_from(&from);
        for &csr in &fp.loop_csr {
            self.use_cursor(csr);
        }
        for (_, csr) in &fp.bindings {
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

        let frame = self.cc_loop_open(from, &fp, where_)?;
        // --- body: build the tagged element [group_key_bytes, payload] ---
        let dirs = vec![false; group_keys.len()];
        self.emit_arr();
        for k in group_keys {
            self.cc_expr(k)?;
        }
        self.emit_order_key(dirs);
        self.emit_arr_push();
        self.cc_select_constructor(Constructor::None, &fp.bindings)?;
        self.emit_arr_push();
        self.emit_arr_push();
        // --- close, then close the read iterators and sort ---
        let exits = self.cc_loop_close(&frame)?;
        let sort_pc = self.pc() + 1;
        self.patch(exits.begin0, sort_pc)?;
        for &csr in &fp.loop_csr {
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
            &fp.bindings,
            (cnt_skip, cnt_take),
        )?;
        // Reset the representative row and accumulators for the new group; the
        // transition slot already holds its key.
        let reset_pc = self.code.len();
        self.emit_group_reset(repr_slot, &terms);

        // Step block: fold the current row into the group.
        let step_pc = self.code.len();
        self.emit_reseed_from_payload(payload_csr, &fp.bindings);
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
            &fp.bindings,
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
        self.emit_row();
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
        let len = self.code.len();
        let op = self.code.get_mut(src)
            .ok_or_else(|| crate::error::Error::InternalError(
                format!("patch: pc[{src}] is out of range (code len={len})")
            ))?;
        match op {
            Vop::Init { jmp }
            | Vop::Next { csr: _, jmp }
            | Vop::Scan { csr: _, jmp, .. }
            | Vop::Iter { csr: _, jmp }
            | Vop::If(jmp)
            | Vop::IfNot(jmp)
            | Vop::CntIfPos(_, jmp)
            | Vop::CntIfZero(_, jmp)
            | Vop::GroupBreak { slot: _, jmp } => *jmp = dst,
            _ => crate::error!("patch: instruction at pc[{src}] is not a control-flow op"),
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
            Expr::Get(get) => self.cc_expr_get(get),
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
            // A parameter compiles to a runtime slot load — the VM resolves it
            // from the bound params, so one program serves every bound value.
            Expr::Param(p) => {
                self.code.push(Vop::LoadParam(p));
                Ok(())
            }
            // A subquery in scalar position: materialize its bag, then coerce
            // the array to a single value (empty → null, >1 row → runtime error).
            Expr::Subquery(select) => {
                self.cc_subquery_array(*select)?;
                self.emit_scalar();
                Ok(())
            }
            // `exists (sub)`: materialize the bag, then test it is non-empty.
            Expr::Exists(select) => {
                self.cc_subquery_array(*select)?;
                self.emit_exists();
                Ok(())
            }
            // `lhs op any/all (sub)`: push lhs then the bag, then fold the
            // comparison over the elements under three-valued logic.
            Expr::Quantify(q) => {
                self.cc_expr(*q.lhs)?;
                self.cc_subquery_array(*q.sub)?;
                self.emit_quantify(q.op, q.all);
                Ok(())
            }
        }
    }

    /// Compiles a subquery so its rows materialize as a `Value::Array` on top of
    /// the stack. A fresh collector array sits at the bottom of the subquery's
    /// frame and the row sink is flipped to `Collect`, so every `Select` plan
    /// `ArrPush`es its output instead of `Yield`ing it; the array is left behind
    /// when the loop exhausts. Correlation is automatic — the subquery compiles
    /// inline in the enclosing loop body and reads the live outer cursors.
    fn cc_subquery_array(&mut self, select: Select) -> Result<()> {
        let saved = self.sink;
        self.emit_arr();
        self.sink = Sink::Collect;
        let result = self.cc_select(select);
        self.sink = saved;
        result
    }

    /// Keyed-table access `table[key, ...]`. An all-literal key is encoded at
    /// COMPILE time (a type mismatch surfaces here as an `Error::Schema`, e.g.
    /// `t["a"]` on an int key); a key with any parameter is encoded at RUN time
    /// (`EncodeKeyTuple`) from the evaluated arg values, so one program serves
    /// every bound value. A full key (arity == key count) is a point lookup
    /// (`Get` → the one row or null); a leading prefix (arity < key count) is a
    /// range lookup (`GetRange` → the matching rows as an array, in key order).
    /// The surrounding `select` has already emitted `Transaction(Read)`, so the
    /// cursor ops run under it.
    fn cc_expr_get(&mut self, get: Get) -> Result<()> {
        let Get { csr, oid, keys, args } = get;
        // Full key (arity == key count) → point lookup; a leading prefix → range.
        let full = args.len() == keys.len();
        self.emit_open(csr as usize, oid);
        self.emit_key_tuple(keys, args)?;
        if full {
            self.emit_get(csr as usize);
        } else {
            self.emit_get_range(csr as usize);
        }
        Ok(())
    }

    /// Emits the encoded composite key for `args` onto the stack, shared by the
    /// keyed-access sites (point/range lookup and the keyed `FROM` prefix scan).
    /// An all-literal key is encoded at COMPILE time (a type mismatch surfaces
    /// here as an `Error::Schema`, e.g. `t["a"]` on an int key); a key with any
    /// parameter pushes its arg values and encodes at RUN time (`EncodeKeyTuple`),
    /// so one program serves every bound value.
    fn emit_key_tuple(&mut self, keys: Vec<Key>, args: Vec<Expr>) -> Result<()> {
        let cnt = args.len();
        if let Some(lits) = Self::all_literal_keys(&args) {
            let key = schema::encode_key_tuple(&lits, &keys)?;
            self.emit_push(Value::Bytes(key.into()));
        } else {
            for arg in args {
                self.cc_expr(arg)?;
            }
            self.emit_encode_key_tuple(keys, cnt);
        }
        Ok(())
    }

    /// Returns the key argument values iff every arg is a literal (`Expr::Lit`),
    /// enabling compile-time key encoding; `None` if any arg is a parameter.
    fn all_literal_keys(args: &[Expr]) -> Option<Vec<Value>> {
        args.iter()
            .map(|a| match a {
                Expr::Lit(v) => Some(v.clone()),
                _ => None,
            })
            .collect()
    }

    /// Compiles a builtin call. A built-in operator (arithmetic, comparison, 3VL
    /// logic, `between`, `in_list`) compiles to its dedicated opcode; any other
    /// name resolves against the `functions` standard-library registry and
    /// compiles to a generic `Vop::Call`. An unknown name or bad arity errors.
    fn cc_expr_call(&mut self, call: Call) -> Result<()> {
        let Call { name, args } = call;
        // Built-in operators compile to dedicated opcodes (hot path, special
        // promotion/3VL semantics live in the VM).
        match operator_op(&name, args.len()) {
            OpLookup::Op(op) => {
                for arg in args {
                    self.cc_expr(arg)?;
                }
                self.code.push(op);
                return Ok(());
            }
            OpLookup::BadArity => return Err(Error::UnknownFunction(name)),
            OpLookup::NotAnOperator => {}
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

    fn emit_encode_key(&mut self, keys: Vec<Key>) {
        self.code.push(Vop::EncodeKey { keys });
    }

    fn emit_encode_key_tuple(&mut self, keys: Vec<Key>, cnt: usize) {
        self.code.push(Vop::EncodeKeyTuple { keys, cnt });
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

    fn emit_scalar(&mut self) {
        self.code.push(Vop::Scalar);
    }

    fn emit_exists(&mut self) {
        self.code.push(Vop::Exists);
    }

    fn emit_quantify(&mut self, op: CmpOp, all: bool) {
        self.code.push(Vop::Quantify { op, all });
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

    /// Re-seeds each from-binding from the sorted payload element (`element[1]`),
    /// so the projection reads it via the same `LoadVal` as a live scan.
    ///
    /// For each `(alias, csr)` in `bindings`: `LoadVal` → `Jpi(1)` → `Jpk(alias)` → `SetVal`.
    fn emit_reseed_from_payload(&mut self, payload_csr: usize, bindings: &[(String, usize)]) {
        for (alias, csr) in bindings {
            self.emit_load(payload_csr);
            self.emit_jpi(1);
            self.emit_jpk(alias.clone());
            self.emit_set_val(*csr);
        }
    }

    fn emit_open(&mut self, csr: usize, tbl: u32) {
        self.use_cursor(csr);
        self.code.push(Vop::Open { csr, tbl });
    }

    fn emit_open_oid(&mut self, csr: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::OpenOid { csr });
    }

    fn emit_dup(&mut self) {
        self.code.push(Vop::Dup);
    }

    fn emit_get(&mut self, csr: usize) {
        self.code.push(Vop::Get { csr });
    }

    fn emit_get_range(&mut self, csr: usize) {
        self.code.push(Vop::GetRange { csr });
    }

    fn emit_scan(&mut self, csr: usize, jmp: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::Scan { csr, jmp, prefix: ScanPrefix::None });
    }

    /// Emits a forward scan of cursor `csr` restricted to the encoded key prefix
    /// on top of the stack (left there by [`Self::emit_key_tuple`]).
    fn emit_scan_from_stack(&mut self, csr: usize, jmp: usize) {
        self.use_cursor(csr);
        self.code.push(Vop::Scan { csr, jmp, prefix: ScanPrefix::Stack });
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

    /// Emits a plan's terminal output of one row: `Yield` it to the caller, or —
    /// inside a subquery — `ArrPush` it onto the collector array that
    /// [`Self::cc_subquery_array`] left at the bottom of the stack frame.
    fn emit_row(&mut self) {
        match self.sink {
            Sink::Yield => self.emit_yield(),
            Sink::Collect => self.emit_arr_push(),
        }
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
        // A subquery is its own query level: aggregates inside it belong to that
        // query, not this one. Don't cross the boundary (a quantifier's left
        // operand is still part of this level, so scan it).
        match e {
            Expr::Subquery(_) | Expr::Exists(_) => return,
            Expr::Quantify(q) => {
                self.visit_expr(&q.lhs);
                return;
            }
            _ => {}
        }
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

/// Which compilation strategy a SELECT takes. The order of checks is significant:
/// GROUP BY and PIVOT are decided before the aggregate path.
enum Plan {
    /// Sort the from/where stream by the group key, then stream with accumulator resets.
    Group,
    /// Fold the entire stream into one accumulator object and yield it once.
    Pivot,
    /// Fold the entire stream into aggregate accumulators and yield one finalized row.
    Aggregate,
    /// Materialize the from/where stream, sort it, then project and yield.
    Order,
    /// Stream each row from the from/where loop, apply limit, project, and yield.
    Stream,
}

/// Classifies a bound SELECT into the appropriate compilation strategy.
///
/// The check order is load-bearing: GROUP BY is tested before aggregates
/// (a grouped aggregate query must reach `cc_group`, not `cc_aggregate`), and
/// PIVOT is tested before aggregates for the same reason.
fn plan(select: &Select) -> Plan {
    if select.group.is_some() {
        Plan::Group
    } else if matches!(select.select, Constructor::Pivot(_)) {
        Plan::Pivot
    } else if has_aggregate(&select.select) || select.having.is_some() {
        Plan::Aggregate
    } else if select.order.is_some() {
        Plan::Order
    } else {
        Plan::Stream
    }
}

/// Analyzes a from clause in a single pass, producing the cursor list, the
/// flattened projection bindings, and the unpivot reseed triples needed by
/// [`Compiler::cc_loop_open`] and its callers.
fn analyze_from(from: &[crate::ir::From]) -> FromPlan {
    let mut loop_csr = Vec::with_capacity(from.len());
    let mut bindings = Vec::new();
    let mut seeds = Vec::new();
    for f in from {
        let csr = f.csr.expect("from item should be bound") as usize;
        loop_csr.push(csr);
        if let Source::Unpivot(u) = &f.src {
            let val = u.val_csr.expect("unpivot value cursor") as usize;
            bindings.push((f.var.clone(), val));
            let att = u.att.as_ref().map(|att| {
                let ac = u.att_csr.expect("unpivot attribute cursor") as usize;
                bindings.push((att.clone(), ac));
                ac
            });
            seeds.push((csr, val, att));
        } else {
            bindings.push((f.var.clone(), csr));
        }
    }
    FromPlan { loop_csr, bindings, seeds }
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
        // A subquery is a separate query level — its aggregates and column
        // references are collected when it is compiled, not here. Skip its body
        // (but a quantifier's left operand belongs to this level).
        match e {
            Expr::Subquery(_) | Expr::Exists(_) => return,
            Expr::Quantify(q) => {
                self.visit_expr_mut(&mut q.lhs);
                return;
            }
            _ => {}
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

/// The result of resolving a name against the built-in operator table.
enum OpLookup {
    /// Not an operator — resolve as a standard-library function instead.
    NotAnOperator,
    /// A known operator applied with the wrong number of arguments.
    BadArity,
    /// A known operator at a valid arity, with its opcode.
    Op(Vop),
}

/// Maps a built-in operator name to an [`OpLookup`], distinguishing unknown
/// names from known operators at the wrong arity.
#[allow(clippy::len_zero)]
fn operator_op(name: &str, argc: usize) -> OpLookup {
    match name {
        "*" => if argc == 2 { OpLookup::Op(Vop::Mul) } else { OpLookup::BadArity },
        "/" => if argc == 2 { OpLookup::Op(Vop::Div) } else { OpLookup::BadArity },
        "%" => if argc == 2 { OpLookup::Op(Vop::Rem) } else { OpLookup::BadArity },
        "+" => if argc == 2 { OpLookup::Op(Vop::Add) } else { OpLookup::BadArity },
        "-" => if argc == 2 { OpLookup::Op(Vop::Sub) } else { OpLookup::BadArity },
        "<" => if argc == 2 { OpLookup::Op(Vop::Lt) } else { OpLookup::BadArity },
        "<=" => if argc == 2 { OpLookup::Op(Vop::Le) } else { OpLookup::BadArity },
        "=" => if argc == 2 { OpLookup::Op(Vop::Eq) } else { OpLookup::BadArity },
        ">=" => if argc == 2 { OpLookup::Op(Vop::Ge) } else { OpLookup::BadArity },
        ">" => if argc == 2 { OpLookup::Op(Vop::Gt) } else { OpLookup::BadArity },
        "!=" => if argc == 2 { OpLookup::Op(Vop::Ne) } else { OpLookup::BadArity },
        "and" => if argc == 2 { OpLookup::Op(Vop::And) } else { OpLookup::BadArity },
        "or" => if argc == 2 { OpLookup::Op(Vop::Or) } else { OpLookup::BadArity },
        "not" => if argc == 1 { OpLookup::Op(Vop::Not) } else { OpLookup::BadArity },
        "is_null" => if argc == 1 { OpLookup::Op(Vop::IsNull) } else { OpLookup::BadArity },
        "is_true" => if argc == 1 { OpLookup::Op(Vop::IsTrue) } else { OpLookup::BadArity },
        "is_false" => if argc == 1 { OpLookup::Op(Vop::IsFalse) } else { OpLookup::BadArity },
        "is_unknown" => if argc == 1 { OpLookup::Op(Vop::IsUnknown) } else { OpLookup::BadArity },
        "between" => if argc == 3 { OpLookup::Op(Vop::Between) } else { OpLookup::BadArity },
        "in_list" => if argc >= 1 { OpLookup::Op(Vop::InList(argc.saturating_sub(1))) } else { OpLookup::BadArity },
        _ => OpLookup::NotAnOperator,
    }
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
        let mut stmt = SqlParser::new()
            .parse(&std::cell::Cell::new(0), SqlLexer::new(sql))
            .unwrap();
        let mut binder = Binder::new(
            catalog.clone(),
            storage.clone(),
            0,
            std::rc::Rc::new(std::cell::RefCell::new(None)),
        );
        binder
            .bind(&mut stmt)
            .unwrap();
        Compiler::new().compile(stmt).unwrap()
    }

    /// Returns the index of the first instruction satisfying `pred`; panics with context if none.
    fn find(code: &[Vop], pred: impl Fn(&Vop) -> bool) -> usize {
        code.iter()
            .position(pred)
            .unwrap_or_else(|| panic!("no instruction matched in {code:#?}"))
    }

    /// Returns the count of instructions satisfying `pred`.
    fn count(code: &[Vop], pred: impl Fn(&Vop) -> bool) -> usize {
        code.iter().filter(|op| pred(op)).count()
    }

    // -------------------------------------------------------------------------
    // select * from catalog
    // -------------------------------------------------------------------------

    #[test]
    fn select_star_from_catalog_scan_exhausts_into_halt() {
        // Load-bearing: an empty table's Scan must exhaust into Halt (loop exit),
        // not into a Yield or any body instruction. Following the jump survives
        // any address shift introduced by later refactors.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select * from catalog;");
        assert_eq!(program.cursors, 1);
        let code = &program.instructions;
        let scan = find(code, |op| matches!(op, Vop::Scan { .. }));
        let Vop::Scan { jmp, .. } = code[scan] else {
            unreachable!()
        };
        assert!(
            matches!(code[jmp], Vop::Halt),
            "empty Scan must exhaust into Halt, found {:?}",
            code[jmp]
        );
    }

    #[test]
    fn select_star_from_catalog_control_flow_shape() {
        // Open precedes Scan; body emits LoadVal then Yield; Next loops back
        // into the body (before Scan); Halt follows the scan's exhaust target.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select * from catalog;");
        assert_eq!(program.cursors, 1);
        let code = &program.instructions;
        let open = find(code, |op| matches!(op, Vop::Open { .. }));
        let scan = find(code, |op| matches!(op, Vop::Scan { .. }));
        let load = find(code, |op| matches!(op, Vop::LoadVal { .. }));
        let yld = find(code, |op| matches!(op, Vop::Yield));
        let next = find(code, |op| matches!(op, Vop::Next { .. }));
        let halt = find(code, |op| matches!(op, Vop::Halt));
        // Structural order.
        assert!(open < scan, "Open precedes Scan");
        assert!(scan < load, "LoadVal is inside the loop body");
        assert!(load < yld, "Yield follows LoadVal");
        assert!(yld < next, "Next follows Yield");
        assert!(next < halt, "Halt follows the loop");
        // Next must jump back into the body (before or at LoadVal, after Scan).
        let Vop::Next { jmp: next_jmp, .. } = code[next] else {
            unreachable!()
        };
        assert!(
            next_jmp > scan && next_jmp <= load,
            "Next must loop back into the body"
        );
        // Exactly one Yield.
        assert_eq!(count(code, |op| matches!(op, Vop::Yield)), 1);
    }

    // -------------------------------------------------------------------------
    // select * from catalog where true
    // -------------------------------------------------------------------------

    #[test]
    fn select_where_true_predicate_skips_to_next() {
        // The IfNot (predicate guard) must jump to the Next instruction, not past it.
        // A false predicate drops the row by jumping to Next which then loops.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select * from catalog where true;");
        let code = &program.instructions;
        let scan = find(code, |op| matches!(op, Vop::Scan { .. }));
        let ifnot = find(code, |op| matches!(op, Vop::IfNot(..)));
        let yld = find(code, |op| matches!(op, Vop::Yield));
        // Predicate sits between Scan and Yield.
        assert!(scan < ifnot, "IfNot is inside the scan loop");
        assert!(ifnot < yld, "IfNot precedes Yield");
        // IfNot jumps to Next (drops the row), not past it.
        let Vop::IfNot(ifnot_jmp) = code[ifnot] else {
            unreachable!()
        };
        assert!(
            matches!(code[ifnot_jmp], Vop::Next { .. }),
            "IfNot must jump to Next to drop the row, found {:?}",
            code[ifnot_jmp]
        );
    }

    #[test]
    fn select_where_true_scan_exhausts_into_halt() {
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select * from catalog where true;");
        let code = &program.instructions;
        let scan = find(code, |op| matches!(op, Vop::Scan { .. }));
        let Vop::Scan { jmp, .. } = code[scan] else {
            unreachable!()
        };
        assert!(
            matches!(code[jmp], Vop::Halt),
            "empty Scan must exhaust into Halt, found {:?}",
            code[jmp]
        );
    }

    // -------------------------------------------------------------------------
    // select * from catalog order by catalog.name
    // -------------------------------------------------------------------------

    #[test]
    fn order_by_phase_one_collects_sort_keys_into_array() {
        // Phase 1: an Arr collector precedes Open; the body builds [OrderKey,
        // payload] pairs and ArrPushes them; Scan exhausts into Close (not Halt).
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(
            &storage,
            &catalog,
            "select * from catalog order by catalog.name;",
        );
        assert_eq!(program.cursors, 2);
        let code = &program.instructions;
        // The collector Arr comes before Open.
        let collector_arr = find(code, |op| matches!(op, Vop::Arr));
        let open = find(code, |op| matches!(op, Vop::Open { .. }));
        assert!(collector_arr < open, "collector Arr precedes Open");
        // OrderKey and ObjAssign (payload build) are inside the scan body.
        let scan = find(code, |op| matches!(op, Vop::Scan { csr: 0, .. }));
        let order_key = find(code, |op| matches!(op, Vop::OrderKey { .. }));
        let obj_assign = find(code, |op| matches!(op, Vop::ObjAssign(..)));
        assert!(scan < order_key, "OrderKey is inside the scan body");
        assert!(scan < obj_assign, "ObjAssign (payload) is inside the scan body");
        // Scan exhausts into Close (releases the read iterator before Sort).
        let Vop::Scan { jmp, .. } = code[scan] else {
            unreachable!()
        };
        assert!(
            matches!(code[jmp], Vop::Close { .. }),
            "empty Scan must exhaust into Close before Sort, found {:?}",
            code[jmp]
        );
    }

    #[test]
    fn order_by_phase_two_sorts_then_drains_into_halt() {
        // Phase 2: Close → Sort → Iter; the Iter exhaust jumps to Halt; the
        // re-seed block (SetVal) and Yield are inside the Iter loop body.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(
            &storage,
            &catalog,
            "select * from catalog order by catalog.name;",
        );
        assert_eq!(program.cursors, 2);
        let code = &program.instructions;
        let close = find(code, |op| matches!(op, Vop::Close { .. }));
        let sort = find(code, |op| matches!(op, Vop::Sort));
        let iter = find(code, |op| matches!(op, Vop::Iter { csr: 1, .. }));
        let set_val = find(code, |op| matches!(op, Vop::SetVal { .. }));
        let yld = find(code, |op| matches!(op, Vop::Yield));
        // Phase 2 structural order.
        assert!(close < sort, "Close precedes Sort");
        assert!(sort < iter, "Sort precedes Iter");
        assert!(iter < set_val, "SetVal (re-seed) is inside the Iter body");
        assert!(set_val < yld, "Yield follows SetVal");
        // Iter exhaust jumps to Halt.
        let Vop::Iter { jmp, .. } = code[iter] else {
            unreachable!()
        };
        assert!(
            matches!(code[jmp], Vop::Halt),
            "empty Iter must exhaust into Halt, found {:?}",
            code[jmp]
        );
        // Exactly one Yield in the whole program.
        assert_eq!(count(code, |op| matches!(op, Vop::Yield)), 1);
    }

    // -------------------------------------------------------------------------
    // select count(*) from catalog
    // -------------------------------------------------------------------------

    #[test]
    fn count_star_empty_table_exhausts_into_finalize() {
        // Load-bearing: an empty table's Scan jumps INTO the finalize block
        // (AggFinal), not past the Yield — so `count(*)` of nothing still
        // yields one row. Following the Scan's exhaust jump to its target opcode
        // survives any address shift.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select count(*) from catalog;");
        assert_eq!(program.aggs, 1);
        let code = &program.instructions;
        let scan = find(code, |op| matches!(op, Vop::Scan { .. }));
        let Vop::Scan { jmp, .. } = code[scan] else {
            unreachable!()
        };
        assert!(
            matches!(code[jmp], Vop::AggFinal { kind: AggKind::Count, .. }),
            "empty Scan must exhaust into AggFinal, found {:?}",
            code[jmp]
        );
    }

    #[test]
    fn count_star_folds_each_row_then_yields_once() {
        // AggInit resets before the loop; AggStep folds inside; AggFinal then
        // Yield happen exactly once after the loop completes.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select count(*) from catalog;");
        assert_eq!(program.cursors, 1);
        assert_eq!(program.aggs, 1);
        let code = &program.instructions;
        let init = find(code, |op| matches!(op, Vop::AggInit { .. }));
        let scan = find(code, |op| matches!(op, Vop::Scan { .. }));
        let step = find(code, |op| matches!(op, Vop::AggStep { .. }));
        let fin = find(code, |op| matches!(op, Vop::AggFinal { .. }));
        let yld = find(code, |op| matches!(op, Vop::Yield));
        assert!(init < scan, "accumulator reset before the scan loop");
        assert!(scan < step, "AggStep is inside the scan body");
        assert!(step < fin, "AggFinal follows the loop");
        assert!(fin < yld, "finalize precedes the single Yield");
        assert_eq!(count(code, |op| matches!(op, Vop::Yield)), 1);
    }

    // -------------------------------------------------------------------------
    // select sum(catalog.name) from catalog
    // -------------------------------------------------------------------------

    #[test]
    fn sum_expr_arg_compiled_before_step_and_scan_exhausts_into_finalize() {
        // Body compiles the arg (LoadVal + Jpk) then AggStep{Sum}; AggInit
        // precedes Scan; Scan exhaust lands on AggFinal{Sum}; aggs == 1.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "select sum(catalog.name) from catalog;");
        assert_eq!(program.aggs, 1);
        let code = &program.instructions;
        let init = find(code, |op| matches!(op, Vop::AggInit { kind: AggKind::Sum, .. }));
        let scan = find(code, |op| matches!(op, Vop::Scan { .. }));
        let load = find(code, |op| matches!(op, Vop::LoadVal { .. }));
        let step = find(code, |op| matches!(op, Vop::AggStep { kind: AggKind::Sum, .. }));
        let fin = find(code, |op| matches!(op, Vop::AggFinal { kind: AggKind::Sum, .. }));
        let yld = find(code, |op| matches!(op, Vop::Yield));
        // Order: AggInit → Scan → LoadVal → AggStep → AggFinal → Yield.
        assert!(init < scan, "AggInit precedes Scan");
        assert!(scan < load, "arg eval (LoadVal) is inside the loop body");
        assert!(load < step, "AggStep follows arg eval");
        assert!(step < fin, "AggFinal is after the loop");
        assert!(fin < yld, "Yield follows AggFinal");
        // Scan exhaust jumps to AggFinal (not past Yield).
        let Vop::Scan { jmp, .. } = code[scan] else {
            unreachable!()
        };
        assert!(
            matches!(code[jmp], Vop::AggFinal { kind: AggKind::Sum, .. }),
            "empty Scan must exhaust into AggFinal{{Sum}}, found {:?}",
            code[jmp]
        );
    }

    // -------------------------------------------------------------------------
    // delete from catalog
    // -------------------------------------------------------------------------

    #[test]
    fn delete_all_phase_one_scan_exhausts_into_close() {
        // Phase 1: collect-then-delete. An empty table's Scan must exhaust into
        // Close (releasing the read iterator) — not into the delete phase Iter.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "delete from catalog;");
        assert_eq!(program.cursors, 2);
        let code = &program.instructions;
        let scan = find(code, |op| matches!(op, Vop::Scan { csr: 0, .. }));
        let Vop::Scan { jmp, .. } = code[scan] else {
            unreachable!()
        };
        assert!(
            matches!(code[jmp], Vop::Close { csr: 0 }),
            "empty Scan must exhaust into Close, found {:?}",
            code[jmp]
        );
    }

    #[test]
    fn delete_all_two_phase_structure() {
        // Phase 1: Arr → Open → Scan → LoadKey → ArrPush → Next(loop).
        // Phase 2: Close → Iter(key csr) → LoadVal → Delete → Next(loop) → Halt.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "delete from catalog;");
        assert_eq!(program.cursors, 2);
        let code = &program.instructions;
        // Phase 1 structural order.
        let arr = find(code, |op| matches!(op, Vop::Arr));
        let open = find(code, |op| matches!(op, Vop::Open { .. }));
        let scan = find(code, |op| matches!(op, Vop::Scan { csr: 0, .. }));
        let load_key = find(code, |op| matches!(op, Vop::LoadKey { .. }));
        let arr_push = find(code, |op| matches!(op, Vop::ArrPush));
        let next_scan = find(code, |op| matches!(op, Vop::Next { csr: 0, .. }));
        assert!(arr < open, "key-collector Arr precedes Open");
        assert!(open < scan, "Open precedes Scan");
        assert!(scan < load_key, "LoadKey is inside the scan body");
        assert!(load_key < arr_push, "ArrPush follows LoadKey");
        assert!(arr_push < next_scan, "Next loops back after ArrPush");
        // Phase 2 structural order.
        let close = find(code, |op| matches!(op, Vop::Close { .. }));
        let iter = find(code, |op| matches!(op, Vop::Iter { csr: 1, .. }));
        let load_val = find(code, |op| matches!(op, Vop::LoadVal { .. }));
        let delete = find(code, |op| matches!(op, Vop::Delete { .. }));
        let next_iter = find(code, |op| matches!(op, Vop::Next { csr: 1, .. }));
        let halt = find(code, |op| matches!(op, Vop::Halt));
        assert!(close < iter, "Close (releases read lock) precedes Iter");
        assert!(iter < load_val, "LoadVal is inside the Iter body");
        assert!(load_val < delete, "Delete follows LoadVal");
        assert!(delete < next_iter, "Next loops back after Delete");
        assert!(next_iter < halt, "Halt follows the delete loop");
    }

    // -------------------------------------------------------------------------
    // delete from catalog where catalog.name = 'x'
    // -------------------------------------------------------------------------

    #[test]
    fn delete_where_predicate_skips_to_next() {
        // A false predicate's IfNot must jump to Next (skipping LoadKey/ArrPush),
        // not to Close — so only matching rows are collected.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(
            &storage,
            &catalog,
            "delete from catalog where catalog.name = 'x';",
        );
        let code = &program.instructions;
        let ifnot = find(code, |op| matches!(op, Vop::IfNot(..)));
        let next_scan = find(code, |op| matches!(op, Vop::Next { csr: 0, .. }));
        // IfNot drops the row by jumping to Next.
        let Vop::IfNot(ifnot_jmp) = code[ifnot] else {
            unreachable!()
        };
        assert!(
            matches!(code[ifnot_jmp], Vop::Next { csr: 0, .. }),
            "IfNot must jump to Next to drop the row, found {:?}",
            code[ifnot_jmp]
        );
        // LoadKey and ArrPush sit between IfNot and Next (only for matching rows).
        let load_key = find(code, |op| matches!(op, Vop::LoadKey { .. }));
        let arr_push = find(code, |op| matches!(op, Vop::ArrPush));
        assert!(ifnot < load_key, "LoadKey follows the predicate guard");
        assert!(load_key < arr_push, "ArrPush follows LoadKey");
        assert!(arr_push < next_scan, "Next comes after ArrPush");
    }

    #[test]
    fn delete_where_two_phase_structure() {
        // Phase 1 Scan exhausts into Close; Phase 2 Iter follows Close.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(
            &storage,
            &catalog,
            "delete from catalog where catalog.name = 'x';",
        );
        let code = &program.instructions;
        let scan = find(code, |op| matches!(op, Vop::Scan { csr: 0, .. }));
        let Vop::Scan { jmp, .. } = code[scan] else {
            unreachable!()
        };
        assert!(
            matches!(code[jmp], Vop::Close { csr: 0 }),
            "Scan must exhaust into Close, found {:?}",
            code[jmp]
        );
        let close = find(code, |op| matches!(op, Vop::Close { .. }));
        let iter = find(code, |op| matches!(op, Vop::Iter { csr: 1, .. }));
        // Phase 2 loads from the key-array cursor (csr=1), not the table cursor (csr=0).
        let load_val = find(code, |op| matches!(op, Vop::LoadVal { csr: 1 }));
        let delete = find(code, |op| matches!(op, Vop::Delete { .. }));
        assert!(close < iter, "Close precedes Iter");
        assert!(iter < load_val, "LoadVal{{csr:1}} is inside Iter body");
        assert!(load_val < delete, "Delete follows LoadVal");
    }

    // -------------------------------------------------------------------------
    // create table t (id int)
    // -------------------------------------------------------------------------

    #[test]
    fn create_table_emits_open_then_oid_btree_insert() {
        // Open → Push(schema) → NewOid → NewBtree → Insert, all in relative order.
        // cursors == 1 (the catalog cursor).
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(&storage, &catalog, "create table t (id int);");
        assert_eq!(program.cursors, 1);
        let code = &program.instructions;
        let open = find(code, |op| matches!(op, Vop::Open { .. }));
        let push = find(code, |op| matches!(op, Vop::Push { .. }));
        let new_oid = find(code, |op| matches!(op, Vop::NewOid { .. }));
        let new_btree = find(code, |op| matches!(op, Vop::NewBtree));
        let insert = find(code, |op| matches!(op, Vop::Insert { .. }));
        assert!(open < push, "Open precedes Push (schema)");
        assert!(push < new_oid, "NewOid follows Push");
        assert!(new_oid < new_btree, "NewBtree follows NewOid");
        assert!(new_btree < insert, "Insert follows NewBtree");
    }

    // -------------------------------------------------------------------------
    // Already-robust tests — left unchanged per spec.
    // -------------------------------------------------------------------------

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
            .parse(
                &std::cell::Cell::new(0),
                SqlLexer::new("insert into t ({\"a\": 1, \"b\": \"x\"});"),
            )
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
            .position(|op| matches!(op, Vop::EncodeKey { keys } if *keys == members))
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
            "select price from unpivot {\"a\": 1, \"b\": 2} as price at sym;",
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
        let code = &program.instructions;
        // Obj accumulator appears before any Scan or Iter.
        let obj = find(code, |op| matches!(op, Vop::Obj));
        let scan_or_iter = code
            .iter()
            .position(|op| matches!(op, Vop::Scan { .. } | Vop::Iter { .. }))
            .unwrap_or(code.len());
        assert!(obj < scan_or_iter, "Obj accumulator precedes the scan/iter loop");
        // At least one ObjSet (dynamic member assignment).
        let obj_set = find(code, |op| matches!(op, Vop::ObjSet));
        assert!(obj < obj_set, "ObjSet follows the Obj accumulator");
        // Exactly one Yield.
        assert_eq!(count(code, |op| matches!(op, Vop::Yield)), 1, "pivot yields exactly one object");
    }

    // -------------------------------------------------------------------------
    // Item 1 — patch error reclassification
    // -------------------------------------------------------------------------

    #[test]
    fn patch_non_jump_instruction_is_internal_error() {
        let mut c = Compiler::new();
        c.code.push(Vop::Halt);                       // not a jump-bearing op
        assert!(matches!(c.patch(0, 3), Err(Error::InternalError(_))));
    }

    #[test]
    fn patch_out_of_range_pc_is_internal_error() {
        let mut c = Compiler::new();                  // empty code
        assert!(matches!(c.patch(0, 3), Err(Error::InternalError(_))));
    }

    // -------------------------------------------------------------------------
    // Item 2 — OpLookup enum
    // -------------------------------------------------------------------------

    // -------------------------------------------------------------------------
    // Phase 3 — multi-source nested-loop characterization test
    // -------------------------------------------------------------------------

    #[test]
    fn nested_loop_two_value_sources_exhaust_topology() {
        // Query: `select x from [1, 2] as x, [3, 4] as y;`
        // Two value sources (outer = x, inner = y) produce a cross-product.
        //
        // The loop structure the helper centralises:
        //   - inner Iter exhausts into the outer Next (advances x, re-enters y)
        //   - outer Iter exhausts into exit (Halt)
        //
        // Using value-scan Iters so no catalog setup is needed.
        let (_dir, storage, catalog) = fixture();
        let program = compile_sql(
            &storage,
            &catalog,
            "select x from [1, 2] as x, [3, 4] as y;",
        );
        let code = &program.instructions;

        // There must be exactly two Iter instructions — outer (csr=0) and inner (csr=1).
        let outer_iter = code
            .iter()
            .position(|op| matches!(op, Vop::Iter { csr: 0, .. }))
            .expect("outer Iter{csr=0} must exist");
        let inner_iter = code
            .iter()
            .position(|op| matches!(op, Vop::Iter { csr: 1, .. }))
            .expect("inner Iter{csr=1} must exist");
        assert!(outer_iter < inner_iter, "outer Iter precedes inner Iter");

        // The outer Next advances the outer cursor; the inner Next advances the inner.
        let outer_next = code
            .iter()
            .position(|op| matches!(op, Vop::Next { csr: 0, .. }))
            .expect("outer Next{csr=0} must exist");
        let inner_next = code
            .iter()
            .position(|op| matches!(op, Vop::Next { csr: 1, .. }))
            .expect("inner Next{csr=1} must exist");

        // Topology: inner Iter's exhaust must jump to the outer Next.
        let Vop::Iter { jmp: inner_iter_jmp, .. } = code[inner_iter] else { unreachable!() };
        assert!(
            matches!(code[inner_iter_jmp], Vop::Next { csr: 0, .. }),
            "inner Iter exhaust must jump to outer Next, found {:?}",
            code[inner_iter_jmp]
        );

        // Topology: outer Iter's exhaust must jump to exit (Halt).
        let Vop::Iter { jmp: outer_iter_jmp, .. } = code[outer_iter] else { unreachable!() };
        assert!(
            matches!(code[outer_iter_jmp], Vop::Halt),
            "outer Iter exhaust must jump to Halt (loop exit), found {:?}",
            code[outer_iter_jmp]
        );

        // Inner Next loops back to the loop body (past both Iters).
        let Vop::Next { jmp: inner_next_jmp, .. } = code[inner_next] else { unreachable!() };
        assert!(
            inner_next_jmp > inner_iter,
            "inner Next must loop back to the body (after inner Iter)"
        );

        // Outer Next loops back to re-enter the inner source (its expression, between the two Iters).
        let Vop::Next { jmp: outer_next_jmp, .. } = code[outer_next] else { unreachable!() };
        assert!(
            outer_next_jmp > outer_iter && outer_next_jmp <= inner_iter,
            "outer Next must loop back to inner source re-entry (between outer and inner Iter)"
        );

        // Exactly one Yield.
        assert_eq!(count(code, |op| matches!(op, Vop::Yield)), 1);
    }

    #[test]
    fn operator_op_resolves_binary_plus() {
        assert!(matches!(operator_op("+", 2), OpLookup::Op(Vop::Add)));
    }

    #[test]
    fn operator_op_rejects_plus_with_wrong_arity() {
        assert!(matches!(operator_op("+", 3), OpLookup::BadArity));
    }

    #[test]
    fn operator_op_in_list_needs_at_least_one_arg() {
        assert!(matches!(operator_op("in_list", 0), OpLookup::BadArity));
    }

    #[test]
    fn operator_op_unknown_name_is_not_an_operator() {
        assert!(matches!(operator_op("hypot", 2), OpLookup::NotAnOperator));
    }

    // -------------------------------------------------------------------------
    // Item 1 — `plan()` classifier precedence tests
    // -------------------------------------------------------------------------

    /// Parse + bind a SQL string and return the bound `Select`, stopping before
    /// `.compile`. Used by the `plan()` precedence tests to exercise the pure
    /// classifier without running the VM.
    fn bound_select(storage: &Storage, catalog: &Catalog, sql: &str) -> Select {
        let mut stmt = SqlParser::new()
            .parse(&std::cell::Cell::new(0), SqlLexer::new(sql))
            .unwrap();
        let mut binder = Binder::new(
            catalog.clone(),
            storage.clone(),
            0,
            std::rc::Rc::new(std::cell::RefCell::new(None)),
        );
        binder
            .bind(&mut stmt)
            .unwrap();
        let Statement::Select(select) = stmt else {
            panic!("expected a SELECT statement");
        };
        select
    }

    #[test]
    fn plan_streams_a_plain_select() {
        let (_dir, storage, catalog) = fixture();
        let select = bound_select(&storage, &catalog, "select * from catalog;");
        assert!(matches!(plan(&select), Plan::Stream));
    }

    #[test]
    fn plan_detects_order_by() {
        let (_dir, storage, catalog) = fixture();
        let select = bound_select(&storage, &catalog, "select * from catalog order by catalog.name;");
        assert!(matches!(plan(&select), Plan::Order));
    }

    #[test]
    fn plan_detects_aggregate_projection() {
        let (_dir, storage, catalog) = fixture();
        let select = bound_select(&storage, &catalog, "select count(*) from catalog;");
        assert!(matches!(plan(&select), Plan::Aggregate));
    }

    #[test]
    fn plan_having_without_aggregate_is_aggregate() {
        let (_dir, storage, catalog) = fixture();
        // HAVING with an aggregate — routes to cc_aggregate (whole input as one group).
        let select = bound_select(&storage, &catalog, "select count(*) from catalog having count(*) > 0;");
        assert!(matches!(plan(&select), Plan::Aggregate));
    }

    #[test]
    fn plan_group_by_beats_aggregate() {
        // A grouped aggregate must resolve to Plan::Group, not Plan::Aggregate —
        // the group check is first in the ladder, locking this precedence.
        let (_dir, storage, catalog) = fixture();
        let select = bound_select(
            &storage,
            &catalog,
            "select count(*) from catalog group by catalog.name;",
        );
        assert!(matches!(plan(&select), Plan::Group));
    }

    #[test]
    fn plan_pivot_beats_aggregate() {
        // A pivot must resolve to Plan::Pivot, not Plan::Aggregate even though
        // pivot aggregates — the pivot check precedes the aggregate check.
        let (_dir, storage, catalog) = fixture();
        let select = bound_select(
            &storage,
            &catalog,
            "pivot catalog.name at catalog.name from catalog;",
        );
        assert!(matches!(plan(&select), Plan::Pivot));
    }

    #[test]
    fn ctas_from_csv_runs() {
        // Run through the real prepare→execute path: `prepare` resolves the
        // program's table handles (a compiled-but-unprepared program isn't
        // runnable, since its `tables` are unresolved).
        let mut db = crate::MonaDB::memory().expect("open db");
        db.execute("create table people as select * from 'tests/fixtures/people.csv';")
            .expect("ctas should complete");
    }

}
