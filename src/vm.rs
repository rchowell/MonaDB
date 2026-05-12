use std::vec;

use crate::ir::Table;
use crate::storage::Storage;
use crate::transaction::{Transaction, TransactionMode};
use crate::value::Value;
use crate::{Result, unsupported};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Vop is a virtual machine instruction code.
#[derive(Debug, Clone)]
pub enum Vop {
    /// Initialize the virtual machine, then jump to jmp.
    Init { jmp: usize },
    /// Consider replacing with an `Insert` instruction on the catalog table.
    CreateTable { table: Table },
    /// Consider replacing with a `Delete` instruction on the catalog table.
    Drop { table: String },
    /// Insert a value into the table.
    Insert(String),
    /// Jump to the instruction at jmp.
    Jump { jmp: usize },
    /// JSON Path Index
    Jpi(usize),
    /// JSON Path Key
    Jpk(String),
    /// JSON Path Expression
    Jpe,
    /// Load a value from cursors[p0] to the stack.
    Load(usize),
    /// Next from cursors[p0], else jump to p1.
    Next(usize, usize),
    /// Opens a table for reading with the cursor positioned at the first row.
    Open(String),
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
    /// Push a value onto the stack.
    Push(Value),
    /// Rewind the cursor to its initial position.
    Rewind(usize, usize),
    /// Return a value from the stack.
    Return(usize),
    /// Open a transaction
    Transaction { txn: TransactionMode },
    /// Halt the virtual machine.
    Halt,
}

// TODO: Uncomment this when ready
// /// This ensures that every variant is <= 4 bytes.
// const _: () = assert!(std::mem::size_of::<Vop>() <= 4);

/// VM holds the state of the virtual machine.
pub struct VM<'s> {
    /// The storage environment reference.
    storage: &'s Storage,
    /// The program counter.
    pc: usize,
    /// The program.
    program: Program,
    /// The stack.
    stack: Vec<Value>,
    /// The open transaction handle, if any.
    txn: Option<Transaction<'s>>,
    // /// The cursors.
    // cursors: Vec<Cursor<'s>>,
    // /// The counters.
    // counters: Vec<u64>,
}

impl<'s> VM<'s> {
    pub fn init(storage: &'s Storage, program: Program) -> VM<'s> {
        VM {
            storage,
            pc: 0,
            program,
            stack: vec![],
            txn: None,
            // cursors: vec![],
            // counters: vec![0; 10],
        }
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn push_bool(&mut self, value: bool) {
        self.stack.push(Value::bool(value));
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
    }

    fn drop(&mut self, tofs: usize) {
        self.stack.truncate(tofs);
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
                    self.pc = *jmp;
                }
                Vop::CreateTable { table } => {
                    unsupported!("create table")
                }

                Vop::Drop { table } => {
                    unsupported!("drop table {table:?} (Phase 1: not yet wired through storage)");
                }
                Vop::Insert(table) => {
                    let value = self.pop();
                    todo!()
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
                Vop::Load(cursor) => {
                    // let row = self.cursors[*cursor].curr();
                    // let val = row.val.clone();
                    // self.push(val);
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
                Vop::Push(v) => {
                    self.push(v.clone());
                }
                Vop::Return(tofs) => {
                    // let v = self.pop();
                    // self.drop(*tofs);
                    // return Ok(Some(v));
                    todo!()
                }
                Vop::Jump { jmp } => {
                    self.pc = *jmp;
                }
                Vop::Transaction { txn } => {
                    let txn = match txn {
                        TransactionMode::Ro => self.storage.read(),
                        TransactionMode::Rw => self.storage.write(),
                    }?;
                    self.txn = Some(txn);
                }
                Vop::Halt => {
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
                Vop::Open(table) => {
                    // let cursor = self.txn.open_cursor(table)?;
                    // let cursor: Cursor<'a> = unsafe { std::mem::transmute(cursor) };
                    // self.cursors.push(cursor);
                }
                Vop::Next(cursor, jmp) => {
                    // if self.cursors[*cursor].next()? {
                    //     self.pc = *jmp;
                    // }
                }
                Vop::Rewind(cursor, jmp) => {
                    // if !self.cursors[*cursor].rewind()? {
                    //     self.pc = *jmp;
                    // }
                }
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
pub struct Rows<'vm> {
    vm: VM<'vm>,
}

impl<'vm> Rows<'vm> {
    pub fn new(vm: VM<'vm>) -> Self {
        Self { vm }
    }

    pub fn next(&mut self) -> Result<Option<Value>> {
        self.vm.next()
    }
}
