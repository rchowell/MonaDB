use std::vec;

use crate::error::Error;
use crate::ir::{ColumnType as IrColumnType, Table};
use crate::storage::{ColumnSchema, ColumnType as StorageColumnType, Cursor, Storage, ReadTxn, WriteTxn};
use crate::value::Value;
use crate::{unsupported, Result};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Whether a program needs a read or write transaction. Patched onto `Vop::Init`
/// by the compiler after walking the emitted code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnMode {
    Read,
    Write,
}

/// Vop is a virtual machine instruction code.
#[derive(Debug, Clone)]
pub enum Vop {
    /// Open a fresh transaction in the given mode.
    Init(TxnMode),
    /// Delete all rows of the table.
    Clear { table: String },
    /// Commit an open transaction.
    Commit,
    /// Consider replacing with an `Insert` instruction on the catalog table.
    CreateTable { table: Table },
    /// Consider replacing with a `Delete` instruction on the catalog table.
    Drop { table: String },
    /// Insert 1 value into the table.
    Insert(String),
    /// Insert n values into the table.
    InsertBatch(String, usize),
    /// Jump to the instruction, pc = p0.
    Jump(usize),
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
    Obj,
    ObjAssign(String),
    ObjSpread,
    CntSet(usize, u64),
    CntIfPos(usize, usize),
    CntIfZero(usize, usize),
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    If(usize),
    IfNot(usize),
    Pop,
    Push(Value),
    Rewind(usize, usize),
    Return(usize),
    /// Begin a transaction. No-op in Phase 1 (single-statement implicit txns).
    Transaction,
    Exit,
}

/// Active transaction held by a running VM. `None` between `Init` and program start,
/// and after `Commit` consumes it.
enum TxnHandle<'a> {
    None,
    Read(ReadTxn<'a>),
    Write(WriteTxn<'a>),
}

impl<'a> TxnHandle<'a> {
    fn open_cursor(&self, table: &str) -> Result<Cursor<'_>> {
        match self {
            TxnHandle::None => Err(Error::InternalError(
                "vm: open_cursor before Init".to_string(),
            )),
            TxnHandle::Read(t) => t.open_cursor(table),
            TxnHandle::Write(t) => t.open_cursor(table),
        }
    }

    fn write_mut(&mut self) -> Result<&mut WriteTxn<'a>> {
        match self {
            TxnHandle::Write(t) => Ok(t),
            _ => Err(Error::InternalError(
                "vm: write opcode needs an active write txn".to_string(),
            )),
        }
    }
}

/// VM holds the state of the virtual machine.
pub struct VM<'a> {
    engine: &'a Storage,
    txn: TxnHandle<'a>,
    pc: usize,
    program: Program,
    stack: Vec<Value>,
    cursors: Vec<Cursor<'a>>,
    counters: Vec<u64>,
}

impl<'a> VM<'a> {
    pub fn init(engine: &'a Storage, program: Program) -> VM<'a> {
        VM {
            engine,
            txn: TxnHandle::None,
            pc: 0,
            program,
            stack: vec![],
            cursors: vec![],
            counters: vec![0; 10],
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

    /// The VM loop.
    #[allow(clippy::too_many_lines)]
    pub fn next(&mut self) -> Result<Option<Value>> {
        loop {
            let op = self.program[self.pc].clone();
            self.pc += 1;
            match &op {
                Vop::Init(mode) => {
                    // SAFETY (lifetimes): Cursor<'a> borrows from the txn, which borrows
                    // from Engine via Arc<EngineInner>. Engine outlives the VM via the
                    // 'a bound on VM<'a>; the txn lives inside `self.txn` for the rest
                    // of the program. See storage/reference §10.2.
                    let engine: &'a Storage = self.engine;
                    self.txn = match mode {
                        TxnMode::Read => TxnHandle::Read(engine.begin_read()?),
                        TxnMode::Write => TxnHandle::Write(engine.begin_write()?),
                    };
                }
                Vop::Clear { table } => {
                    unsupported!("clear table {table:?} (Phase 1: not yet wired through storage)");
                }
                Vop::CreateTable { table } => {
                    let columns = table_to_columns(table);
                    self.txn.write_mut()?.create_table(&table.name, columns)?;
                }
                Vop::Commit => {
                    // Cursors borrow from the txn — drop them before consuming it.
                    self.cursors.clear();
                    let taken = std::mem::replace(&mut self.txn, TxnHandle::None);
                    match taken {
                        TxnHandle::Write(t) => t.commit()?,
                        TxnHandle::Read(_) | TxnHandle::None => {
                            // Read txns drop on Commit; nothing to flush.
                        }
                    }
                }
                Vop::Drop { table } => {
                    unsupported!("drop table {table:?} (Phase 1: not yet wired through storage)");
                }
                Vop::Insert(table) => {
                    let value = self.pop();
                    self.txn.write_mut()?.put_row(table, value)?;
                }
                Vop::InsertBatch(table, n) => {
                    let values = self.take(*n);
                    let txn = self.txn.write_mut()?;
                    for value in values {
                        txn.put_row(table, value)?;
                    }
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
                    let row = self.cursors[*cursor].curr();
                    let val = row.val.clone();
                    self.push(val);
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
                    let v = self.pop();
                    self.drop(*tofs);
                    return Ok(Some(v));
                }
                Vop::Jump(jmp) => {
                    self.pc = *jmp;
                }
                Vop::Transaction => {
                    // Phase 1: implicit single-statement transactions; no-op.
                }
                Vop::Exit => {
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
                    // SAFETY (lifetimes): Cursor<'a> borrows the txn held inside `self`.
                    // The borrow-checker can't see through the TxnHandle indirection, so
                    // we transmute the shorter lifetime returned by open_cursor up to 'a.
                    // This is sound because (a) the txn lives inside `self.txn` until
                    // Commit consumes it, and (b) we drop all cursors when the txn is
                    // taken (via Vop::Commit), so no cursor outlives its txn.
                    let cursor = self.txn.open_cursor(table)?;
                    let cursor: Cursor<'a> = unsafe { std::mem::transmute(cursor) };
                    self.cursors.push(cursor);
                }
                Vop::Next(cursor, jmp) => {
                    if self.cursors[*cursor].next()? {
                        self.pc = *jmp;
                    }
                }
                Vop::Rewind(cursor, jmp) => {
                    if !self.cursors[*cursor].rewind()? {
                        self.pc = *jmp;
                    }
                }
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

fn table_to_columns(table: &Table) -> Vec<ColumnSchema> {
    table
        .columns
        .iter()
        .map(|c| ColumnSchema {
            name: c.name.clone(),
            typ: match c.typ {
                IrColumnType::Int => StorageColumnType::Int,
                IrColumnType::String => StorageColumnType::String,
            },
        })
        .collect()
}
