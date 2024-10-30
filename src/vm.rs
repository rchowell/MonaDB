use std::vec;

use crate::cursor::Cursor;
use crate::ir::Table;
use crate::value::Value;
use crate::Result;
use crate::MonaDB;

/// Code is a sequence of virtual machine instructions.
pub type Code = Vec<Vop>;

/// Vop is a virtual machine instruction code.
#[derive(Debug, Clone)]
pub enum Vop {
    /// Initialize the VM.
    Init,
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
    /// Copy a value from stack[i] to the top.
    Load(usize),
    /// Next from cursors[p0], else jump to p1.
    Next(usize, usize),
    /// Opens a table for reading with the cursor positioned at the first row.
    /// TODO replace table with cursor.
    Open(String),
    //
    //--- Object --- 
    //
    /// Push an empty object.
    Obj,
    /// Assign a value to a key in an object.
    ObjAssign(String),
    /// Spread a value into an object.
    ObjSpread,
    ///--- Arithmetic ---
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    ///--- Comparison ---
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    ///---- Stack Manipulation ---
    /// Pop a value from the stack.
    Pop,
    /// Push a value onto the stack.
    Push(Value),
    /// Yield top-of-stack from the VM loop; dropping all values after p0.
    Yield(usize),
    /// Begin a transaction.
    Transaction,
    /// Exit the VM.
    Exit,
}

/// VM holds the state of the virtual machine.
/// TODO VM should be using the catalog/connection, not the library.
pub struct VM<'r> {
    db: &'r MonaDB,
    pc: usize,
    program: Code,
    cursors: Vec<Cursor>,
    // moving from registers to stack for simplicity..
    stack: Vec<Value>,
}

impl<'r> VM<'r> {
    pub fn init(db: &MonaDB, program: Code) -> VM {
        VM {
            db,
            pc: 0,
            program,
            cursors: vec![],
            //
            stack: vec![],
        }
    }

    pub fn next(&mut self) -> Result<Option<Value>> {
        loop {
            let op = self.program[self.pc].clone(); // <-- CLONE INSTRUCTION (MAKE THESE u64)
            self.pc += 1;
            match &op {
                Vop::Init => {
                    // do nothing (for now)
                }
                Vop::Clear { table } => {
                    self.db.clear(table)?;
                }
                Vop::CreateTable { table } => {
                    self.db.create_table(table)?;
                }
                Vop::Commit => {
                    self.db.commit()?;
                }
                Vop::Drop { table } => {
                    self.db.drop_table(table)?;
                }
                Vop::Insert(table) => {
                    let value = self.pop();
                    self.db.insert(table, value)?;
                }
                Vop::InsertBatch(table, n) => {
                    let values = self.take(*n);
                    self.db.insert_batch(table, &values)?;
                }
                Vop::Jpe => {
                    let e = self.pop();
                    let v= self.pop();
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
                Vop::Load(idx) => {
                    let v = self.stack[*idx].clone();
                    // println!("VAR: {} ", v);
                    self.push(v);
                }
                Vop::Next(cursor, jmp) => {
                    match self.cursors[*cursor].next()? {
                        Some(next) => self.push(next),
                        None => self.pc = *jmp,
                    };
                }
                Vop::Obj => {
                    self.stack.push(Value::object());
                }
                Vop::ObjAssign(name) => {
                    let val = self.pop();
                    let obj = self.peek();
                    // println!("OBJ_ASSIGN: {}: {} ", name, val);
                    obj.set(name.to_string(), val);
                }
                Vop::ObjSpread => {
                    let val = self.pop();
                    let obj = self.peek();
                    obj.spread(val);
                }
                Vop::Open(table)  => {
                    self.cursors.push(self.db.scan(table)?);
                }
                Vop::Pop => {
                    let _ = self.pop();
                }
                Vop::Push(v) => {
                    self.push(v.clone());
                }
                Vop::Yield(tofs) => {
                    let v = self.pop();
                    self.drop(*tofs);
                    return Ok(Some(v));
                }
                Vop::Jump(jmp) => {
                    self.pc = *jmp;
                }
                Vop::Transaction => {
                    self.db.transaction();
                }
                Vop::Exit => {
                    return Ok(None);
                }
                //
                // Operators
                //
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
            }
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
}
