use std::mem::take;
use std::vec;
use std::fmt::Write;

use crate::cursor::Cursor;
use crate::storage::BTree;
use crate::storage::Storage;
use heed::byteorder::BigEndian;
use heed::byteorder::ByteOrder;
use crate::transaction::{Transaction, TransactionMode};
use crate::value::Value;
use crate::{Result, unsupported};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Vop is a virtual machine instruction code.
///
/// Operand naming conventions are strict 3-chars.
///   csr  – cursor slot
///   tbl  – index into vm tables
///   cst  – index into vm constants
///   jmp  – jump target (absolute PC)
///   cnt  – count (arity, column count, …)
///   key  – secondary-key discriminant (u8 tag, variant selector)
///
/// TODO: Make this Copy in the near future.
///
#[derive(Debug, Clone)]
pub enum Vop {
    /// Initialize the virtual machine, then jump to jmp.
    Init { jmp: usize },
    /// Consider replacing with a `Delete` instruction on the catalog table.
    Drop { table: String },
    /// Insert a value into the given cursor (csr) where key=stack[0] value=stack[1].
    Insert { csr: usize },
    /// Jump to the instruction at jmp.
    Jump { jmp: usize },
    /// JSON Path Index
    Jpi(usize),
    /// JSON Path Key
    Jpk(String),
    /// JSON Path Expression
    Jpe,
    /// Load the current value from the cursor onto to the stack.
    Load { csr: usize },
    /// Next from the cursor, else jump.
    Next { csr: usize, jmp: usize },
    /// Pushes a new OID for the given cursor (csr) onto the stack.
    NewOid { csr: usize },
    /// Creates a new btree named by the stack[0] oid.
    NewBtree,
    /// Opens the table 'tbl' and binds to cursors[csr].
    Open { csr: usize, tbl: String },
    /// Create a new object on the stack.
    Obj,
    /// Assign a value to an object member.
    ObjAssign(String),
    /// Spread a value into an object.
    ObjSpread,
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
    /// If the value on the stack is true, jump to p1.
    If(usize),
    /// If the value on the stack is false, jump to p1.
    IfNot(usize),
    /// Pop the top value from the stack.
    Pop,
    /// Push a literal onto the stack.
    Push { val: Value },
    /// Returns the top value from the stack.
    Return,
    /// Initializes a cursor's scan state
    Scan { csr: usize, jmp: usize },
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
    /// The program.
    program: Program,
    /// The stack.
    stack: Vec<Value>,
    /// The open cursors, addressed by index; dropped before the transaction.
    cursors: Vec<Cursor>,
    /// The open transaction handle; dropped last.
    txn: Option<Transaction>,
}

impl VM {
    pub fn init(storage: Storage, program: Program) -> VM {
        VM {
            storage,
            pc: 0,
            program,
            stack: vec![],
            txn: None,
            cursors: vec![],
            // counters: vec![0; 10],
        }
    }

    fn push<V: Into<Value>>(&mut self, value: V) {
        self.stack.push(value.into());
    }

    fn push_bool(&mut self, value: bool) {
        self.stack.push(Value::bool(value));
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
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
            let op = self.program[self.pc].clone();
            self.pc += 1;
            match &op {
                Vop::Init { jmp } => {
                    // TODO: initialize any state
                    self.pc = *jmp;
                }
                Vop::Drop { table } => {
                    unsupported!("drop table {table:?} (Phase 1: not yet wired through storage)");
                }
                Vop::Insert { csr } => {
                    let val = self.pop().encode()?;
                    let key = self.pop().encode()?;
                    let csr = &self.cursors[*csr];
                    let txn = self.txn.as_mut().expect("Insert before Transaction");
                    csr.insert(txn, &key, &val)?;
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
                Vop::NewBtree => {
                    // The btree name is the hex of the big-endian oid.
                    let oid = self.peek().as_oid();
                    let txn = self.txn.as_mut().unwrap();
                    let mut name = String::with_capacity(8);
                    for b in oid.to_be_bytes() {
                        write!(&mut name, "{b:02x}")?;
                    }
                    self.storage.create_btree(txn, name.as_str())?;
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
                Vop::Pop => {
                    let _ = self.pop();
                }
                Vop::Push { val } => {
                    // TODO: no clone, we want copy.
                    self.push(val.clone());
                }
                Vop::Return => {
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
                    self.push(l + r);
                }
                Vop::Sub => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l - r);
                }
                Vop::Mul => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l * r);
                }
                Vop::Div => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l / r);
                }
                Vop::Rem => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(l % r);
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
                    self.push_bool(l == r);
                }
                Vop::Ne => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push_bool(l != r);
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
                    // TODO: assign new cursor to cursors[csr], push is only ok right now
                    let txn = self.txn.as_ref().expect("Open before Transaction");
                    let btree = self.storage.open_btree(txn, tbl)?;
                    let cursor = Cursor::new(btree);
                    self.cursors.push(cursor);
                }
                Vop::Scan { csr, jmp} => {
                    let txn = self.txn.as_ref().expect("Scan before Transaction");
                    if !self.cursors[*csr].scan(txn, None)? {
                        self.pc = *jmp;
                    }
                }
                Vop::Next { csr, jmp} => {
                    if self.cursors[*csr].next()? {
                        self.pc = *jmp;
                    }
                }
                Vop::Load { csr } => {
                    let (_, val) = self.cursors[*csr].current().expect("Load on unpositioned cursor");
                    let val = Value::decode(val)?;
                    self.push(val);
                }
                //
                // Counter Instructions
                //
                Vop::CntSet(c, v) => {
                    // self.counters[*c] = *v;
                }
                Vop::CntIfPos(c, jmp) => {
                    // if self.counters[*c] > 0 {
                    //     self.counters[*c] -= 1;
                    //     self.pc = *jmp;
                    // }
                }
                Vop::CntIfZero(c, jmp) => {
                    // if self.counters[*c] == 0 {
                    //     self.pc = *jmp;
                    // } else {
                    //     self.counters[*c] -= 1;
                    // }
                }
            }
        }
    }
}

/// Pull-based result iterator over a running VM. Produced by `MonaDB::exec`.
pub struct Rows {
    vm: VM,
}

impl Rows {
    pub fn new(vm: VM) -> Self {
        Self { vm }
    }

    pub fn next(&mut self) -> Result<Option<Value>> {
        self.vm.next()
    }
}
