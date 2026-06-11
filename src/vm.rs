use std::mem::take;
use std::vec;

use crate::Result;
use crate::cursor::Cursor;
use crate::error::Error;
use crate::schema;
use crate::ir::Key;
use crate::storage::Storage;
use crate::transaction::{Transaction, TransactionMode};
use crate::value::Value;
use heed::byteorder::BigEndian;
use heed::byteorder::ByteOrder;

/// Program is a sequence of virtual machine instructions and relevant state.
pub struct Program {
    /// The number of cursor slots needed
    pub cursors: usize,
    /// The number of counter slots needed
    pub counters: usize,
    /// The program's instruction set
    pub instructions: Vec<Vop>,
}

/// Vop is a virtual machine instruction code.
///
/// Operand naming conventions are strict 3-chars
///   csr  – cursor slot
///   tbl  – a btree table oid
///   cst  – index into vm constants (does not exist yet)
///   jmp  – jump target (absolute PC)
///   cnt  – count (arity, column count, …)
///   key  – (does not exist yet)
///   val  - inline (for now) values
///
/// TODO: Make this Copy in the near future.
///
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
    /// Next from the cursor, else jump.
    Next { csr: usize, jmp: usize },
    /// Pushes a new OID for the given cursor (csr) onto the stack.
    NewOid { csr: usize },
    /// Pushes a new key onto the stack, built by encoding the row's fields in the given order.
    NewKey { keys: Vec<Key> },
    /// Creates a new btree named by the stack[0] oid.
    NewBtree,
    /// Opens the table at the given table oid and binds to cursors[csr].
    Open { csr: usize, tbl: u32 },
    /// Create a new object on the stack.
    Obj,
    /// Assign a value to an object member.
    ObjAssign(String),
    /// Spread a value into an object.
    ObjSpread,
    /// Merge a value into an object: spread its fields if it is an object,
    /// else set it under the given name.
    ObjMerge(String),
    /// Create a new array on the stack.
    Arr,
    /// Append the top value to the array beneath it.
    ArrPush,
    /// Set a counter to a value.
    CntSet(usize, u64),
    /// If the counter is greater than 0, decrement it and jump to p1.
    CntIfPos(usize, usize),
    /// If the counter is 0, jump to p1.
    CntIfZero(usize, usize),
    /// Add two values on the stack.
    Add,
    /// Subtract two values on the stack.
    Sub,
    /// Multiply two values on the stack.
    Mul,
    /// Divide two values on the stack.
    Div,
    /// Remainder of two values on the stack.
    Rem,
    /// Less than two values on the stack.
    Lt,
    /// Less than or equal to two values on the stack.
    Le,
    /// Greater than two values on the stack.
    Gt,
    /// Greater than or equal to two values on the stack.
    Ge,
    /// Equal to two values on the stack.
    Eq,
    /// Not equal to two values on the stack.
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
    /// Initializes a cursor's scan state
    Scan { csr: usize, jmp: usize },
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
    /// The program instructions
    instructions: Vec<Vop>,
    /// The stack.
    stack: Vec<Value>,
    /// The open cursors, addressed by index; dropped before the transaction.
    cursors: Vec<Cursor>,
    /// The limit counters, addressed by index.
    counters: Vec<u64>,
    /// Rows changed by mutations (inserts, updates, deletes). Reported by
    /// `Rows::finish`, so `execute` returns a real affected-row count.
    affected: u64,
    /// The open transaction handle; dropped last.
    txn: Option<Transaction>,
}

impl VM {
    pub fn init(storage: Storage, program: Program) -> VM {
        // Allocate an unopened cursor for each slot
        let mut cursors = Vec::with_capacity(program.cursors);
        cursors.resize_with(program.cursors, Cursor::new);
        VM {
            storage,
            pc: 0,
            instructions: program.instructions,
            stack: vec![],
            txn: None,
            cursors,
            counters: vec![0; program.counters],
            affected: 0,
        }
    }

    fn push<V: Into<Value>>(&mut self, value: V) {
        self.stack.push(value.into());
    }

    fn push_bool(&mut self, value: bool) {
        self.stack.push(Value::bool(value));
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().expect("Stack is empty")
    }

    fn take(&mut self, n: usize) -> Vec<Value> {
        let i = self.stack.len() - n;
        self.stack.split_off(i)
    }

    fn peek(&mut self) -> &mut Value {
        self.stack.last_mut().unwrap()
    }

    #[allow(clippy::too_many_lines)]
    pub fn next(&mut self) -> Result<Option<Value>> {
        loop {
            // TODO: make vop 'Copy' then deref
            let op = self.instructions[self.pc].clone();
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
                Vop::NewKey { keys } => {
                    let val = self.pop();
                    let key = schema::encode_key(&val, keys)?;
                    self.stack.push(val);
                    self.stack.push(Value::Bytes(key.into()));
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
                Vop::Arr => {
                    self.stack.push(Value::array());
                }
                Vop::ArrPush => {
                    let val = self.pop();
                    let arr = self.peek();
                    arr.push(val);
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
                    let txn = self.txn.as_ref().expect("Open before Transaction");
                    let btree = self.storage.open_btree(txn, *tbl)?;
                    self.cursors[*csr].open(btree);
                }
                Vop::Scan { csr, jmp } => {
                    let txn = self.txn.as_ref().expect("Scan before Transaction");
                    if !self.cursors[*csr].scan(txn, None)? {
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

/// Take an encoded key off the stack. `LoadKey`/`NewKey` already push encoded
/// key bytes, so move them out rather than re-cloning them through `encode()`.
fn pop_key(val: Value) -> Result<Vec<u8>> {
    match val {
        Value::Bytes(bytes) => Ok(bytes.to_vec()),
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
        Err(Error::Schema(format!("row value must be an object, got {val}")))
    }
}

/// This coerces a value to unknown (None), true, false. It is not precisely
/// what I want, but good enough for now. It's for 3VL.
fn to_bool(v: &Value) -> Option<bool> {
    if v.is_null() {
        None
    } else {
        Some(v.is_truthy())
    }
}

/// Pull-based iterator over a running VM.
pub struct Rows {
    vm: VM,
}

impl Rows {
    pub fn new(vm: VM) -> Self {
        Self { vm }
    }

    /// Returns the next 'row' i.e. a JSON value.
    pub fn next(&mut self) -> Result<Option<Value>> {
        self.vm.next()
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
