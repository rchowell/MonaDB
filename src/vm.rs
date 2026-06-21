//! Stack-based bytecode interpreter.
//!
//! [`VM::next`] drives a fetch-decode-execute loop over a [`Program`]'s [`Vop`]
//! stream, operating on a value stack plus fixed banks of cursors and counters.
//! Each `Yield` pauses the loop with one result row; `Halt` commits and stops.

use std::cell::Cell;
use std::cmp::Ordering;
use std::mem::take;
use std::rc::Rc;
use std::vec;

use crate::Result;
use crate::cursor::Cursor;
use crate::error::Error;
use crate::functions;
use crate::ir::{AggKind, CmpOp, Key, Param};
use crate::schema;
use crate::storage::Storage;
use crate::transaction::{Transaction, TransactionMode};
use crate::value::{Params, Value};
use heed::byteorder::BigEndian;
use heed::byteorder::ByteOrder;

/// Program is a sequence of virtual machine instructions and relevant state.
#[derive(Debug, Clone)]
pub struct Program {
    /// The number of cursor slots needed
    pub cursors: usize,
    /// The number of counter slots needed
    pub counters: usize,
    /// The number of aggregate-accumulator slots needed
    pub aggs: usize,
    /// Whether a successful run changes catalog membership (CREATE/DROP).
    pub mutates_catalog: bool,
    /// The program's instruction set
    pub instructions: Vec<Vop>,
}

/// A virtual machine instruction.
///
/// Operands follow a strict 3-char naming convention:
///
///   csr  cursor slot
///   tbl  a btree table oid
///   cst  index into vm constants (does not exist yet)
///   jmp  jump target (absolute PC)
///   cnt  count (arity, column count, …)
///   key  (does not exist yet)
///   val  inline (for now) value
///
/// Stack effects are written `… before ─▶ … after`, top-of-stack on the right.
///
/// TODO: Make this Copy in the near future.
#[derive(Debug, Clone)]
pub enum Vop {
    /// Initialize the virtual machine, then jump to jmp.
    Init { jmp: usize },
    /// Insert a value into the given cursor (csr) where key=stack[0] val=stack[1].
    Insert { csr: usize },
    /// Delete the key (top-of-stack) from cursor (csr)'s btree immediately.
    Delete { csr: usize },
    /// End cursor (csr)'s active scan, releasing its read iterator. The cursor
    /// stays open on its btree (unpositioned) so it can be written through.
    Close { csr: usize },
    /// Clear all rows from the btree at the given table oid.
    Clear { tbl: u32 },
    /// Jump to the instruction at jmp.
    Jump { jmp: usize },
    /// JSON Path Index
    Jpi(usize),
    /// JSON Path Key
    Jpk(String),
    /// JSON Path Expression
    Jpe,
    /// Load the cursor's (csr) current key onto the stack.
    LoadKey { csr: usize },
    /// Load the cursor's (csr) current val onto the stack.
    LoadVal { csr: usize },
    /// Pop a value and bind it as cursor (csr)'s single current value, so a
    /// later `LoadVal { csr }` returns it. Used to re-seed a from-binding from a
    /// materialized payload after a sort (ORDER BY).
    SetVal { csr: usize },
    /// Next from the cursor, else jump.
    Next { csr: usize, jmp: usize },
    /// Pushes a new OID for the given cursor (csr) onto the stack.
    NewOid { csr: usize },
    /// Encodes the row's declared key columns into a composite key and pushes it onto the stack.
    EncodeKey { keys: Vec<Key> },
    /// Encodes a key tuple from the top `cnt` stack values (pushed in key
    /// column order) into a composite key, replacing them with the key bytes.
    /// The runtime counterpart of compile-time key encoding, used when a keyed
    /// access has a parameter key.
    ///
    ///   stack:  … v0 v1 … v{cnt-1}  ─▶  … key
    EncodeKeyTuple { keys: Vec<Key>, cnt: usize },
    /// Pushes the value bound to parameter `p` (`?`/`$N`/`$name`). A missing
    /// binding is a `BindError` (raised here, at run time).
    ///
    ///   stack:  …  ─▶  … param
    LoadParam(Param),
    /// Creates a new btree named by the stack[0] oid.
    NewBtree,
    /// Opens the table at the given table oid and binds to cursors[csr].
    Open { csr: usize, tbl: u32 },
    /// Pops a table oid and opens that btree on cursors[csr].
    OpenOid { csr: usize },
    /// Duplicates the top-of-stack value.
    Dup,
    /// Point lookup: pop an encoded key, fetch the row from cursor (csr)'s
    /// btree, and push the decoded row value (or null on a miss). The cursor
    /// must already be `Open`.
    Get { csr: usize },
    /// Range lookup: pop an encoded key prefix, prefix-scan cursor (csr)'s
    /// btree, and push the matching rows as a `Value::Array` in key order
    /// (empty on no match). The cursor must already be `Open`.
    GetRange { csr: usize },
    /// Create a new object on the stack.
    Obj,
    /// Assign a value to an object member.
    ObjAssign(String),
    /// Spread a value into an object.
    ObjSpread,
    /// Merge a value into an object: spread its fields if it is an object,
    /// else set it under the given name.
    ObjMerge(String),
    /// Dynamic-key set: the name is computed, not baked in (the dual of
    /// [`Vop::ObjAssign`]). A non-string name is skipped, so `pivot` over a
    /// non-string `at` value contributes nothing.
    ///
    ///   stack:  … obj name val  ─▶  … obj
    ObjSet,
    /// Expand an object into its attribute-value pairs (the dual of
    /// [`Vop::ObjSet`], used to lower `unpivot`). A non-object yields an empty
    /// array, so `unpivot` of a non-tuple produces no rows.
    ///
    ///   stack:  … obj  ─▶  … [[name, val], …]
    Entries,
    /// Create a new array on the stack.
    Arr,
    /// Append the top value to the array beneath it.
    ArrPush,
    /// Pop the order-key values for one row and push their order-preserving
    /// `Bytes` encoding (one per `dirs` entry; `true` = descending). The bytes
    /// sort in ORDER BY order, so a later `Sort` clusters rows correctly.
    OrderKey { dirs: Vec<bool> },
    /// Sort the top-of-stack array in place by each element's `[0]` key bytes
    /// (the tag pushed by `OrderKey`). Unstable, per the spec.
    Sort,
    /// Reset the accumulator at slot to `kind`'s identity (SQLite's
    /// `resetAccumulator`). Runs once before the scan: count → 0, sum/min/max →
    /// null, avg → `[sum, count]`.
    AggInit { slot: usize, kind: AggKind },
    /// Fold one argument into the accumulator at slot (SQLite's `xStep`):
    ///
    ///   stack:  … v  ─▶  …
    ///
    /// Pops `v` and updates `aggs[slot]`, skipping a null `v` (for `count(*)` the
    /// compiler pushes a non-null constant, so every row still counts).
    AggStep { slot: usize, kind: AggKind },
    /// Push the finalized aggregate at slot (SQLite's `xFinalize`):
    ///
    ///   stack:  …  ─▶  … result
    ///
    /// Non-mutating, so it is idempotent: avg divides sum by count, the rest read
    /// the accumulator straight out.
    AggFinal { slot: usize, kind: AggKind },
    /// Detect a GROUP BY group boundary against the previous key at `slot`:
    ///
    ///   stack:  … key_bytes  ─▶  …
    ///
    /// Pops the current row's encoded group key. If `aggs[slot]` is null (the
    /// first row) or equals the key (same group), record it and jump to `jmp`
    /// (the step block — no flush). Otherwise it is a new group: record the new
    /// key and fall through to the flush block, which finalizes the group that
    /// just ended. The previous key lives in the aggregate bank like every other
    /// per-group cell, so nested grouping just allocates more slots.
    GroupBreak { slot: usize, jmp: usize },
    /// Set a counter to a value.
    CntSet(usize, u64),
    /// If the counter is greater than 0, decrement it and jump to p1.
    CntIfPos(usize, usize),
    /// If the counter is 0, jump to p1.
    CntIfZero(usize, usize),
    /// Adds the top two values:  … a b ─▶ … (a + b).
    Add,
    /// Subtracts the top two values:  … a b ─▶ … (a - b).
    Sub,
    /// Multiplies the top two values:  … a b ─▶ … (a * b).
    Mul,
    /// Divides the top two values:  … a b ─▶ … (a / b).
    Div,
    /// Remainder of the top two values:  … a b ─▶ … (a % b).
    Rem,
    /// Tests the top two values:  … a b ─▶ … (a < b).
    Lt,
    /// Tests the top two values:  … a b ─▶ … (a <= b).
    Le,
    /// Tests the top two values:  … a b ─▶ … (a > b).
    Gt,
    /// Tests the top two values:  … a b ─▶ … (a >= b).
    Ge,
    /// Tests the top two values:  … a b ─▶ … (a == b).
    Eq,
    /// Tests the top two values:  … a b ─▶ … (a != b).
    Ne,
    /// Logical NOT with 3VL semantics.
    Not,
    /// Logical AND with 3VL semantics (false-dominant).
    And,
    /// Logical OR with 3VL semantics (true-dominant).
    Or,
    /// Definite-bool: true iff top is null.
    IsNull,
    /// Definite-bool: true iff top is the boolean true.
    IsTrue,
    /// Definite-bool: true iff top is the boolean false.
    IsFalse,
    /// Definite-bool: true iff top is null.
    IsUnknown,
    /// Ternary range check: pop b, a, x; push x >= a && x <= b.
    Between,
    /// Variadic membership: pop n list values then target; push target in list.
    InList(usize),
    /// Scalar-subquery coercion: pop an array; push its single element, or null
    /// if empty. More than one element is a runtime error.
    ///
    ///   stack:  … [v]      ─▶  … v
    ///   stack:  … []       ─▶  … null
    Scalar,
    /// Existence test: pop an array; push true iff it is non-empty.
    ///
    ///   stack:  … arr      ─▶  … (len(arr) > 0)
    Exists,
    /// Quantified comparison: pop an array then a left value; push the
    /// three-valued result of `lhs <op> any/all (array)`. `all` chooses ALL
    /// (else ANY); an empty array gives `true` for ALL and `false` for ANY.
    ///
    ///   stack:  … lhs arr  ─▶  … bool|null
    Quantify { op: CmpOp, all: bool },
    /// Call builtin `fun` (a `functions` registry index) on its `cnt` arguments:
    ///
    ///   stack:  … a b c  ─▶  … fun(a, b, c)
    Call { fun: usize, cnt: usize },
    /// If the value on the stack is true, jump to p1.
    If(usize),
    /// If the value on the stack is false, jump to p1.
    IfNot(usize),
    /// Pop the top value from the stack.
    Pop,
    /// Push a literal onto the stack.
    Push { val: Value },
    /// Yields the top value from the stack, pausing the VM step
    Yield,
    /// Initializes a cursor's forward scan, jumping to `jmp` if empty. With a
    /// `prefix`, the scan is restricted to the contiguous run of keys sharing
    /// it (a keyed-table leading-key range); without one it scans the whole
    /// table.
    Scan {
        csr: usize,
        jmp: usize,
        prefix: Option<Rc<[u8]>>,
    },
    /// Pops a value off the stack and iterates its array elements on cursors[csr], jumping to jmp if empty.
    Iter { csr: usize, jmp: usize },
    /// Open a transaction with the given mode
    Transaction { txm: TransactionMode },
    /// Halt the virtual machine.
    Halt,
}

// TODO: Uncomment this when ready
// /// This ensures that every variant is <= 4 bytes.
// const _: () = assert!(std::mem::size_of::<Vop>() <= 4);

/// VM holds the state of the virtual machine.
pub struct VM {
    /// The storage environment is an owned Arc clone.
    storage: Storage,
    /// The program counter.
    pc: usize,
    /// The program being run, shared by `Rc` so a prepared plan is reused across
    /// executions without deep-copying its instruction stream.
    program: Rc<Program>,
    /// The stack.
    stack: Vec<Value>,
    /// The open cursors, addressed by index; dropped before the transaction.
    cursors: Vec<Cursor>,
    /// The limit counters, addressed by index.
    counters: Vec<u64>,
    /// The aggregate accumulators, addressed by index. One `Value` cell per
    /// aggregate term; avg's cell is a `[sum, count]` array.
    aggs: Vec<Value>,
    /// Rows changed by mutations (inserts, updates, deletes). Reported by
    /// `Rows::finish`, so `execute` returns a real affected-row count.
    affected: u64,
    /// Shared catalog generation counter; bumped on Halt when `mutates_catalog`.
    catalog_generation: Rc<Cell<u64>>,
    /// Whether this program changes catalog membership on success.
    mutates_catalog: bool,
    /// Bound query parameters, resolved by `Vop::LoadParam` at run time.
    params: Params,
    /// The open transaction handle; dropped last.
    txn: Option<Transaction>,
}

impl VM {
    /// Builds a VM primed to run `program` against `storage` with bound `params`.
    pub fn init(
        storage: Storage,
        catalog_generation: Rc<Cell<u64>>,
        program: Rc<Program>,
        params: Params,
    ) -> VM {
        // Allocate an unopened cursor for each slot
        let mut cursors = Vec::with_capacity(program.cursors);
        cursors.resize_with(program.cursors, Cursor::new);
        let mutates_catalog = program.mutates_catalog;
        VM {
            storage,
            pc: 0,
            stack: vec![],
            txn: None,
            cursors,
            counters: vec![0; program.counters],
            aggs: vec![Value::Null; program.aggs],
            affected: 0,
            catalog_generation,
            mutates_catalog,
            params,
            program,
        }
    }

    /// Pushes a value onto the stack.
    fn push<V: Into<Value>>(&mut self, value: V) {
        self.stack.push(value.into());
    }

    /// Pushes a boolean onto the stack.
    fn push_bool(&mut self, value: bool) {
        self.stack.push(Value::bool(value));
    }

    /// Pops and returns the top value.
    fn pop(&mut self) -> Value {
        self.stack.pop().expect("Stack is empty")
    }

    /// Pops and returns the top `n` values, preserving their stack order.
    fn take(&mut self, n: usize) -> Vec<Value> {
        let i = self.stack.len() - n;
        self.stack.split_off(i)
    }

    /// Returns a mutable reference to the top value.
    fn peek(&mut self) -> &mut Value {
        self.stack.last_mut().unwrap()
    }

    /// Runs the fetch-decode-execute loop until it yields a row or halts.
    ///
    /// Returns `Some(row)` at each `Yield` (the VM stays resumable) and `None`
    /// at `Halt`, after committing the transaction.
    #[allow(clippy::too_many_lines)]
    pub fn next(&mut self) -> Result<Option<Value>> {
        loop {
            // TODO: make vop 'Copy' then deref
            let op = self.program.instructions[self.pc].clone();
            self.pc += 1;
            match &op {
                Vop::Init { jmp } => {
                    // TODO: initialize any state
                    self.pc = *jmp;
                }
                Vop::Insert { csr } => {
                    let key = pop_key(self.pop())?;
                    let val = self.pop();
                    ensure_object(&val)?;
                    let val = val.encode()?;
                    let cursor = &self.cursors[*csr];
                    let txn = self.txn.as_mut().expect("Insert before Transaction");
                    cursor.insert(txn, &key, &val)?;
                    self.affected += 1;
                }
                Vop::Delete { csr } => {
                    let key = pop_key(self.pop())?;
                    let cursor = &self.cursors[*csr];
                    let txn = self.txn.as_mut().expect("Delete before Transaction");
                    cursor.delete(txn, &key)?;
                    self.affected += 1;
                }
                Vop::Close { csr } => {
                    self.cursors[*csr].close();
                }
                Vop::Clear { tbl } => {
                    let txn = self.txn.as_mut().expect("Clear before Transaction");
                    self.storage.clear_btree(txn, *tbl)?;
                }
                Vop::Jpe => {
                    let e = self.pop();
                    let v = self.pop();
                    let v = v.jpe(e).unwrap_or_default();
                    self.push(v);
                }
                Vop::Jpi(idx) => {
                    let v = self.pop();
                    let v = v.jpi(*idx).unwrap_or_default();
                    self.push(v);
                }
                Vop::Jpk(key) => {
                    let v = self.pop();
                    let v = v.jpk(key).unwrap_or_default();
                    self.push(v);
                }
                Vop::NewOid { csr } => {
                    let txn = self.txn.as_ref().unwrap();
                    let csr = &self.cursors[*csr];
                    let oid = match csr.last(txn)? {
                        Some((key, _)) => BigEndian::read_u32(&key) + 1,
                        None => 0,
                    };
                    self.push(oid);
                }
                Vop::EncodeKey { keys } => {
                    let val = self.pop();
                    let key = schema::encode_key(&val, keys)?;
                    self.stack.push(val);
                    self.stack.push(Value::Bytes(key.into()));
                }
                Vop::EncodeKeyTuple { keys, cnt } => {
                    // The arg values were pushed in key-column order, so the
                    // tuple is the top `cnt` of the stack in that order. Encode
                    // straight from the stack slice, then drop them — no temp Vec.
                    let at = self.stack.len() - *cnt;
                    let key = schema::encode_key_tuple(&self.stack[at..], keys)?;
                    self.stack.truncate(at);
                    self.push(Value::Bytes(key.into()));
                }
                Vop::LoadParam(p) => {
                    let v = match p {
                        Param::Numbered(n) => self.params.get_numbered(*n),
                        Param::Named(name) => self.params.get_named(name),
                    };
                    match v {
                        Some(v) => self.push(v.clone()),
                        None => {
                            let name = match p {
                                Param::Numbered(n) => format!("${n}"),
                                Param::Named(name) => format!("${name}"),
                            };
                            return Err(Error::BindError(format!("missing parameter {name}")));
                        }
                    }
                }
                Vop::NewBtree => {
                    let tbl = self.peek().as_oid();
                    let txn = self.txn.as_mut().unwrap();
                    self.storage.create_btree(txn, tbl)?;
                }
                Vop::Obj => {
                    self.stack.push(Value::object());
                }
                Vop::ObjAssign(name) => {
                    let val = self.pop();
                    let obj = self.peek();
                    obj.set(name.clone(), val);
                }
                Vop::ObjSpread => {
                    let val = self.pop();
                    let obj = self.peek();
                    obj.spread(val);
                }
                Vop::ObjMerge(name) => {
                    let val = self.pop();
                    let obj = self.peek();
                    if val.is_object() {
                        obj.spread(val);
                    } else {
                        obj.set(name.clone(), val);
                    }
                }
                Vop::ObjSet => {
                    let val = self.pop();
                    let name = self.pop();
                    if let Some(key) = name.as_str() {
                        self.peek().set(key, val);
                    }
                }
                Vop::Entries => {
                    let val = self.pop();
                    let mut arr = Value::array();
                    if let Some(members) = val.members() {
                        for (name, value) in members {
                            let mut pair = Value::array();
                            pair.push(Value::String(name.into()));
                            pair.push(value);
                            arr.push(pair);
                        }
                    }
                    self.push(arr);
                }
                Vop::Arr => {
                    self.stack.push(Value::array());
                }
                Vop::ArrPush => {
                    let val = self.pop();
                    let arr = self.peek();
                    arr.push(val);
                }
                Vop::OrderKey { dirs } => {
                    let vals = self.take(dirs.len());
                    let key = schema::encode_order_key(&vals, dirs);
                    self.push(Value::Bytes(key.into()));
                }
                Vop::Sort => {
                    let mut arr = self.pop();
                    debug_assert!(
                        matches!(arr, Value::Array(_)),
                        "Sort expects the collector array on top of stack",
                    );
                    if let Value::Array(rc) = &mut arr {
                        std::rc::Rc::make_mut(rc)
                            .sort_unstable_by(|a, b| sort_key(a).cmp(sort_key(b)));
                    }
                    self.push(arr);
                }
                Vop::AggInit { slot, kind } => {
                    self.aggs[*slot] = agg_init(*kind);
                }
                Vop::AggStep { slot, kind } => {
                    let v = self.pop();
                    let cell = take(&mut self.aggs[*slot]);
                    self.aggs[*slot] = agg_step(cell, *kind, v)?;
                }
                Vop::AggFinal { slot, kind } => {
                    let out = agg_final(&self.aggs[*slot], *kind);
                    self.push(out);
                }
                Vop::GroupBreak { slot, jmp } => {
                    let cur = self.pop();
                    let is_first = self.aggs[*slot].is_null();
                    let is_break = !is_first && self.aggs[*slot] != cur;
                    if is_first || is_break {
                        // First row or a new group: record its key.
                        self.aggs[*slot] = cur;
                    }
                    if !is_break {
                        // First row or same group: skip the flush, go to the step.
                        self.pc = *jmp;
                    }
                }
                Vop::Pop => {
                    let _ = self.pop();
                }
                Vop::Push { val } => {
                    // TODO: no clone, we want copy.
                    self.push(val.clone());
                }
                Vop::Yield => {
                    let val = self.pop();
                    return Ok(Some(val));
                }
                Vop::Jump { jmp } => {
                    self.pc = *jmp;
                }
                Vop::Transaction { txm } => {
                    let txn = Transaction::new(&self.storage, *txm)?;
                    self.txn = Some(txn);
                }
                Vop::Halt => {
                    self.cursors.clear();
                    if let Some(txn) = take(&mut self.txn) {
                        txn.commit()?;
                    }
                    if self.mutates_catalog {
                        let generation = self.catalog_generation.get();
                        self.catalog_generation.set(generation + 1);
                    }
                    return Ok(None);
                }
                Vop::Add => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l.add(r)?);
                }
                Vop::Sub => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l.sub(r)?);
                }
                Vop::Mul => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l.mul(r)?);
                }
                Vop::Div => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l.div(r)?);
                }
                Vop::Rem => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l.rem(r)?);
                }
                Vop::Lt => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push_bool(l < r);
                }
                Vop::Le => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push_bool(l <= r);
                }
                Vop::Gt => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push_bool(l > r);
                }
                Vop::Ge => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push_bool(l >= r);
                }
                Vop::Eq => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push_bool(l.eq(&r));
                }
                Vop::Ne => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push_bool(l.ne(&r));
                }
                Vop::Not => {
                    let v = self.pop();
                    match to_bool(&v) {
                        Some(true) => self.push_bool(false),
                        Some(false) => self.push_bool(true),
                        None => self.push(Value::null()),
                    }
                }
                Vop::And => {
                    let r = self.pop();
                    let l = self.pop();
                    let lv = to_bool(&l);
                    let rv = to_bool(&r);
                    if lv == Some(false) || rv == Some(false) {
                        self.push_bool(false);
                    } else if lv == Some(true) && rv == Some(true) {
                        self.push_bool(true);
                    } else {
                        self.push(Value::null());
                    }
                }
                Vop::Or => {
                    let r = self.pop();
                    let l = self.pop();
                    let lv = to_bool(&l);
                    let rv = to_bool(&r);
                    if lv == Some(true) || rv == Some(true) {
                        self.push_bool(true);
                    } else if lv == Some(false) && rv == Some(false) {
                        self.push_bool(false);
                    } else {
                        self.push(Value::null());
                    }
                }
                Vop::IsNull => {
                    let v = self.pop();
                    self.push_bool(v.is_null());
                }
                Vop::IsTrue => {
                    let v = self.pop();
                    self.push_bool(to_bool(&v) == Some(true));
                }
                Vop::IsFalse => {
                    let v = self.pop();
                    self.push_bool(to_bool(&v) == Some(false));
                }
                Vop::IsUnknown => {
                    let v = self.pop();
                    self.push_bool(v.is_null());
                }
                Vop::Between => {
                    let b = self.pop();
                    let a = self.pop();
                    let x = self.pop();
                    self.push_bool(x.ge(&a) && x.le(&b));
                }
                Vop::InList(n) => {
                    let items = self.take(*n);
                    let target = self.pop();
                    let mut hit = false;
                    for item in &items {
                        if target.eq(item) {
                            hit = true;
                            break;
                        }
                    }
                    self.push_bool(hit);
                }
                Vop::Scalar => {
                    let arr = self.pop();
                    let v = match &arr {
                        Value::Array(items) => match items.as_slice() {
                            [] => Value::null(),
                            [v] => v.clone(),
                            _ => crate::error!("scalar subquery returned more than one row"),
                        },
                        // A subquery always materializes an array; tolerate any
                        // other value as itself rather than failing.
                        other => other.clone(),
                    };
                    self.push(v);
                }
                Vop::Exists => {
                    let arr = self.pop();
                    self.push_bool(arr.len().unwrap_or(0) > 0);
                }
                Vop::Quantify { op, all } => {
                    let arr = self.pop();
                    let lhs = self.pop();
                    self.push(quantify(*op, *all, &lhs, &arr));
                }
                Vop::Call { fun, cnt } => {
                    let args = self.take(*cnt);
                    self.push(functions::call(*fun, &args)?);
                }
                Vop::If(jmp) => {
                    if self.pop().is_truthy() {
                        self.pc = *jmp;
                    }
                }
                Vop::IfNot(jmp) => {
                    if !self.pop().is_truthy() {
                        self.pc = *jmp;
                    }
                }
                //
                // Cursor Instructions
                //
                Vop::Open { csr, tbl } => {
                    if !self.cursors[*csr].is_open() {
                        let txn = self.txn.as_ref().expect("Open before Transaction");
                        let btree = self.storage.open_btree(txn, *tbl)?;
                        self.cursors[*csr].open(btree);
                    }
                }
                Vop::OpenOid { csr } => {
                    let oid = self.pop().as_oid();
                    if !self.cursors[*csr].is_open() {
                        let txn = self.txn.as_ref().expect("Open before Transaction");
                        let btree = self.storage.open_btree(txn, oid)?;
                        self.cursors[*csr].open(btree);
                    }
                }
                Vop::Dup => {
                    let val = self.stack.last().expect("Dup on empty stack").clone();
                    self.push(val);
                }
                Vop::Get { csr } => {
                    let key = pop_key(self.pop())?;
                    let txn = self.txn.as_ref().expect("Get before Transaction");
                    let val = self.cursors[*csr].get(txn, &key)?;
                    self.push(val);
                }
                Vop::GetRange { csr } => {
                    let prefix = pop_key(self.pop())?;
                    let txn = self.txn.as_ref().expect("GetRange before Transaction");
                    let cursor = &mut self.cursors[*csr];
                    let mut arr = Value::array();
                    let mut more = cursor.scan(txn, Some(&prefix))?;
                    while more {
                        arr.push(cursor.load()?);
                        more = cursor.next()?;
                    }
                    self.push(arr);
                }
                Vop::Scan { csr, jmp, prefix } => {
                    let txn = self.txn.as_ref().expect("Scan before Transaction");
                    if !self.cursors[*csr].scan(txn, prefix.as_deref())? {
                        self.pc = *jmp;
                    }
                }
                Vop::Iter { csr, jmp } => {
                    let val = self.pop();
                    if !self.cursors[*csr].iter(val)? {
                        self.pc = *jmp;
                    }
                }
                Vop::Next { csr, jmp } => {
                    if self.cursors[*csr].next()? {
                        self.pc = *jmp;
                    }
                }
                Vop::LoadVal { csr } => {
                    let val = self.cursors[*csr].load()?;
                    self.push(val);
                }
                Vop::SetVal { csr } => {
                    let val = self.pop();
                    self.cursors[*csr].set(val);
                }
                Vop::LoadKey { csr } => {
                    let key = self.cursors[*csr]
                        .current()
                        .map(|(key, _)| key.to_vec())
                        .unwrap_or_default();
                    self.push(Value::Bytes(key.into()));
                }
                //
                // Counter Instructions
                //
                Vop::CntSet(c, v) => {
                    self.counters[*c] = *v;
                }
                Vop::CntIfPos(c, jmp) => {
                    if self.counters[*c] > 0 {
                        self.counters[*c] -= 1;
                        self.pc = *jmp;
                    }
                }
                Vop::CntIfZero(c, jmp) => {
                    if self.counters[*c] == 0 {
                        self.pc = *jmp;
                    } else {
                        self.counters[*c] -= 1;
                    }
                }
            }
        }
    }
}

/// The sort key of a materialized ORDER BY element `[key_bytes, payload]`: its
/// `[0]` component as raw bytes. `Value` has no total `Ord` (and `Bytes` isn't
/// in its `PartialOrd`), so `Sort` compares these byte slices directly. The key
/// is borrowed out of the element — no clone per comparison. A well-formed
/// element always has `Bytes` at `[0]`; anything else sorts as empty.
fn sort_key(elem: &Value) -> &[u8] {
    match elem {
        Value::Array(items) => match items.first() {
            Some(Value::Bytes(bytes)) => bytes,
            _ => {
                debug_assert!(false, "ORDER BY element[0] must be the key bytes");
                &[]
            }
        },
        _ => {
            debug_assert!(
                false,
                "ORDER BY element must be a [key_bytes, payload] array"
            );
            &[]
        }
    }
}

/// The reset value of an aggregate accumulator (SQLite's `resetAccumulator`):
/// count starts at zero, sum/min/max start null (the "no rows yet" sentinel),
/// and avg starts as a `[sum, count]` pair.
fn agg_init(kind: AggKind) -> Value {
    match kind {
        AggKind::Count => Value::Int(0),
        AggKind::Sum | AggKind::Min | AggKind::Max | AggKind::First => Value::Null,
        AggKind::Avg => {
            let mut acc = Value::array();
            acc.push(Value::Float(0.0));
            acc.push(Value::Int(0));
            acc
        }
    }
}

/// Folds one (already-popped) argument into an accumulator (SQLite's `xStep`). A
/// null argument is skipped for every kind; `count(*)` still counts because the
/// compiler pushes a non-null constant for it.
fn agg_step(cell: Value, kind: AggKind, v: Value) -> Result<Value> {
    if v.is_null() {
        return Ok(cell);
    }
    match kind {
        // `cell` starts at `Int(0)` and stays an int.
        AggKind::Count => cell.add(Value::Int(1)),
        AggKind::Sum => sum_add(cell, v),
        AggKind::Min => extremum(cell, v, Ordering::Less, "min"),
        AggKind::Max => extremum(cell, v, Ordering::Greater, "max"),
        AggKind::Avg => avg_step(cell, v),
        // Keep the first folded value; within a group every key is equal, so
        // this captures the group's representative (used by GROUP BY).
        AggKind::First => Ok(if cell.is_null() { v } else { cell }),
    }
}

/// Produces the finalized aggregate value (SQLite's `xFinalize`). Non-mutating,
/// so calling it more than once is harmless: count/sum/min/max read the cell
/// straight out; avg divides the running sum by the count (null on no rows).
fn agg_final(cell: &Value, kind: AggKind) -> Value {
    match kind {
        AggKind::Count | AggKind::Sum | AggKind::Min | AggKind::Max | AggKind::First => {
            cell.clone()
        }
        AggKind::Avg => {
            let (sum, count) = avg_parts(cell);
            if count == 0 {
                Value::Null
            } else {
                Value::Float(sum / count as f64)
            }
        }
    }
}

/// Adds a non-null numeric `v` into a running sum. `Int + Int` stays an `Int`,
/// but **promotes to `Float` on i64 overflow** (SQLite-faithful) and once either
/// side is a float; a non-number `v` is a runtime type error.
#[allow(clippy::cast_precision_loss)]
fn sum_add(cell: Value, v: Value) -> Result<Value> {
    if !v.is_number() {
        return Err(Error::InternalError(format!(
            "sum() requires numbers, got {v}"
        )));
    }
    if cell.is_null() {
        return Ok(v); // first non-null value seeds the sum
    }
    if let (Value::Int(a), Value::Int(b)) = (&cell, &v) {
        // The promotion `(a + b) as f64` is bounded by 2·i64::MAX, always finite.
        return Ok(a
            .checked_add(*b)
            .map_or_else(|| Value::Float(*a as f64 + *b as f64), Value::Int));
    }
    // Either operand is a float; both are numbers, so `as_f64` is total here.
    // Reject a non-finite total: `Value::Float` forbids NaN/∞ (a JSON number
    // can't encode them — they'd silently serialize to `null`).
    let sum = cell.as_f64().unwrap_or(0.0) + v.as_f64().unwrap_or(0.0);
    if !sum.is_finite() {
        return Err(Error::InternalError(
            "sum() overflowed to a non-finite value".into(),
        ));
    }
    Ok(Value::Float(sum))
}

/// Keeps whichever of `cell`/`v` compares `want` (min keeps the lesser, max the
/// greater). Incomparable operands (e.g. int vs string) are a runtime error —
/// `Value`'s ordering defines no cross-type collation.
fn extremum(cell: Value, v: Value, want: Ordering, name: &str) -> Result<Value> {
    if cell.is_null() {
        return Ok(v); // first non-null value
    }
    match v.partial_cmp(&cell) {
        Some(ord) if ord == want => Ok(v),
        Some(_) => Ok(cell),
        None => Err(Error::InternalError(format!(
            "{name}() arguments are not comparable"
        ))),
    }
}

/// Folds a non-null numeric `v` into an avg accumulator `[sum, count]`,
/// accumulating the sum in `f64`; a non-number `v` is a runtime type error.
fn avg_step(cell: Value, v: Value) -> Result<Value> {
    let Some(x) = v.as_f64() else {
        return Err(Error::InternalError(format!(
            "avg() requires numbers, got {v}"
        )));
    };
    let (sum, count) = avg_parts(&cell);
    // Reject a non-finite running sum, as `sum_add` does (the no-NaN/∞ invariant).
    let sum = sum + x;
    if !sum.is_finite() {
        return Err(Error::InternalError(
            "avg() overflowed to a non-finite value".into(),
        ));
    }
    let mut acc = Value::array();
    acc.push(Value::Float(sum));
    acc.push(Value::Int(count + 1));
    Ok(acc)
}

/// Reads an avg accumulator's `[sum, count]` pair, defaulting a malformed cell
/// to `(0.0, 0)`.
fn avg_parts(cell: &Value) -> (f64, i64) {
    match cell {
        Value::Array(items) => {
            let sum = items.first().and_then(Value::as_f64).unwrap_or(0.0);
            let count = match items.get(1) {
                Some(Value::Int(c)) => *c,
                _ => 0,
            };
            (sum, count)
        }
        _ => (0.0, 0),
    }
}

/// Take an encoded key off the stack. `LoadKey`/`EncodeKey` already push encoded
/// key bytes, so move them out rather than re-cloning them through `encode()`.
fn pop_key(val: Value) -> Result<Vec<u8>> {
    match val {
        Value::Bytes(bytes) => Ok(bytes.to_vec()),
        // A catalog key is a bare oid: 4 raw big-endian bytes (`get_table` reads
        // it back with `BigEndian::read_u32`), not the tagged flat value form.
        Value::Oid(oid) => Ok(oid.to_be_bytes().to_vec()),
        val => val.encode(),
    }
}

/// Enforce the stored-row invariant: every row value is an object. Surfaces a
/// scalar (or other non-object) row instead of writing it, which would later
/// no-op on field assignment.
fn ensure_object(val: &Value) -> Result<()> {
    if val.is_object() {
        Ok(())
    } else {
        Err(Error::Schema(format!(
            "row value must be an object, got {val}"
        )))
    }
}

/// Coerces a value to a 3VL truth value: `null` → unknown (`None`), else its
/// truthiness as `Some(bool)`.
fn to_bool(v: &Value) -> Option<bool> {
    if v.is_null() {
        None
    } else {
        Some(v.is_truthy())
    }
}

/// The three-valued comparison `lhs <op> rhs` of one element: `None` is unknown
/// (either side null, or the two values are incomparable across types).
fn cmp3(op: CmpOp, lhs: &Value, rhs: &Value) -> Option<bool> {
    if lhs.is_null() || rhs.is_null() {
        return None;
    }
    match op {
        // Both sides are non-null here, so `eq`/`ne` are definite.
        CmpOp::Eq => Some(lhs.eq(rhs)),
        CmpOp::Ne => Some(lhs.ne(rhs)),
        CmpOp::Lt => lhs.partial_cmp(rhs).map(Ordering::is_lt),
        CmpOp::Le => lhs.partial_cmp(rhs).map(Ordering::is_le),
        CmpOp::Gt => lhs.partial_cmp(rhs).map(Ordering::is_gt),
        CmpOp::Ge => lhs.partial_cmp(rhs).map(Ordering::is_ge),
    }
}

/// Folds a quantified comparison `lhs <op> any/all (array)` under three-valued
/// logic. ANY is true on the first true element, false only if every element is
/// false, else unknown; ALL is false on the first false element, true only if
/// every element is true, else unknown. An empty array gives ALL true, ANY
/// false. A non-array `arr` (never produced by a subquery) is treated as empty.
fn quantify(op: CmpOp, all: bool, lhs: &Value, arr: &Value) -> Value {
    let items: &[Value] = match arr {
        Value::Array(items) => items,
        _ => &[],
    };
    let mut saw_unknown = false;
    for e in items {
        match cmp3(op, lhs, e) {
            Some(true) if !all => return Value::bool(true),
            Some(false) if all => return Value::bool(false),
            None => saw_unknown = true,
            _ => {}
        }
    }
    if saw_unknown {
        Value::null()
    } else {
        Value::bool(all)
    }
}

/// Pull-based iterator over a running VM.
pub struct Rows {
    vm: VM,
}

impl Rows {
    /// Wraps a primed VM as a pull-based row iterator.
    pub fn new(vm: VM) -> Self {
        Self { vm }
    }

    /// Returns the next 'row' i.e. a JSON value.
    pub fn next(&mut self) -> Result<Option<Value>> {
        self.vm.next()
    }

    /// Rows changed by INSERT/UPDATE/DELETE. Meaningful after [`Self::next`]
    /// returns `None` (the transaction has committed at `Halt`).
    pub fn mutations(&self) -> u64 {
        self.vm.affected
    }

    /// Completes the statement and commits its transaction, returning the row
    /// count: rows yielded for a query, rows changed for a mutation (one is
    /// always zero).
    pub fn finish(mut self) -> Result<u64> {
        let mut n = 0;
        while self.next()?.is_some() {
            n += 1;
        }
        Ok(n + self.vm.affected)
    }
}

#[cfg(test)]
mod agg_tests {
    //! Direct tests of the aggregate fold/finalize helpers. These pin the
    //! Int-vs-Float result type, which the conformance harness can't see (it
    //! compares JSON numbers by `f64`, so `Int(6)` and `Float(6.0)` look equal).

    use super::{AggKind, agg_final, agg_init, agg_step};
    use crate::value::Value;

    /// Folds a sequence of values into a fresh accumulator and finalizes it.
    fn run(kind: AggKind, vals: &[Value]) -> Value {
        let mut cell = agg_init(kind);
        for v in vals {
            cell = agg_step(cell, kind, v.clone()).expect("step");
        }
        agg_final(&cell, kind)
    }

    #[test]
    fn count_counts_nonnull_steps() {
        // count(expr): a null step is skipped, a non-null one increments.
        let out = run(
            AggKind::Count,
            &[Value::Int(7), Value::Null, Value::Int(9)],
        );
        assert!(matches!(out, Value::Int(2)));
    }

    #[test]
    fn sum_of_ints_stays_int() {
        let out = run(AggKind::Sum, &[Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert!(matches!(out, Value::Int(6)), "got {out:?}");
    }

    #[test]
    fn sum_with_a_float_promotes() {
        let out = run(AggKind::Sum, &[Value::Int(1), Value::Float(0.5)]);
        match out {
            Value::Float(f) => assert_eq!(f, 1.5),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn sum_int_overflow_promotes_to_float() {
        // i64::MAX + i64::MAX overflows, so the running sum promotes to f64.
        let out = run(AggKind::Sum, &[Value::Int(i64::MAX), Value::Int(i64::MAX)]);
        assert!(matches!(out, Value::Float(_)), "got {out:?}");
    }

    #[test]
    fn sum_over_no_rows_is_null() {
        assert!(matches!(run(AggKind::Sum, &[]), Value::Null));
    }

    #[test]
    fn first_keeps_the_earliest_value() {
        // First holds the first folded value — a GROUP BY group's representative.
        let out = run(
            AggKind::First,
            &[Value::Int(10), Value::Int(20), Value::Int(30)],
        );
        assert!(matches!(out, Value::Int(10)), "got {out:?}");
    }

    #[test]
    fn first_over_no_rows_is_null() {
        assert!(matches!(run(AggKind::First, &[]), Value::Null));
    }

    #[test]
    fn sum_non_finite_overflow_errors() {
        // Summing past f64::MAX must error, not store a forbidden Float(inf).
        let mut cell = agg_init(AggKind::Sum);
        cell = agg_step(cell, AggKind::Sum, Value::Float(1e308)).expect("first");
        assert!(agg_step(cell, AggKind::Sum, Value::Float(1e308)).is_err());
    }

    #[test]
    fn min_and_max_pick_extremes() {
        let vals = [Value::Int(3), Value::Int(1), Value::Int(2)];
        assert!(matches!(run(AggKind::Min, &vals), Value::Int(1)));
        assert!(matches!(run(AggKind::Max, &vals), Value::Int(3)));
    }

    #[test]
    fn min_incomparable_types_errors() {
        let mut cell = agg_init(AggKind::Min);
        cell = agg_step(cell, AggKind::Min, Value::Int(1)).expect("first");
        let err = agg_step(cell, AggKind::Min, Value::from("a".to_string()));
        assert!(err.is_err(), "comparing int and string should error");
    }

    #[test]
    fn avg_is_float_mean_and_null_when_empty() {
        match run(AggKind::Avg, &[Value::Int(1), Value::Int(2)]) {
            Value::Float(f) => assert_eq!(f, 1.5),
            other => panic!("expected Float, got {other:?}"),
        }
        assert!(matches!(run(AggKind::Avg, &[]), Value::Null));
    }
}
