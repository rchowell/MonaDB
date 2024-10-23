use std::vec;

use crate::cursor::Cursor;
use crate::ir::Table;
use crate::value::Value;
use crate::{unsupported, Result};
use crate::{value::Record, Rho};

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
    /// Insert row into a table.
    Insert { tbl: String, row: usize },
    /// JSON Path Index
    Jpi(usize),
    /// JSON Path Key
    Jpk(String),
    /// JSON Path Expression
    Jpe,
    /// Load a value (push) from cursors[i].
    LoadC(usize),
    /// Load a value (push) from stack[i].
    LoadV(usize),
    /// Advances the cursor, assigning the row to `var`.
    /// If there is a row, then jump to `jmp`; otherwise goto next.
    Next { jmp: usize },
    /// Push an empty object.
    Obj,
    /// Assign a value to a key in an object.
    ObjAssign { name: String },
    /// Spread a value into an object.
    ObjSpread,
    /// Opens a table for reading with the cursor positioned at the first row.
    Open {
        /// TODO replace table with cursor.
        table: String,
    },
    /// Pop a value from the stack.
    Pop,
    /// Push a value into a register.
    Push(Value),
    /// Returns the value at the top of the stack.
    Return,
    /// Set cursor to the start; jump to `jmp` if the table is empty.
    Rewind { jmp: usize },
    /// Begin a transaction.
    Transaction,
    /// Exit the VM.
    Exit,
}

/// These are methods so that I can later optimize the Vop representation.
/// ... without having to gut compiler.rs
impl Vop {
    #[inline]
    pub fn exit() -> Vop {
        Vop::Exit
    }

    #[inline]
    pub fn clear(table: String) -> Vop {
        Vop::Clear { table }
    }

    #[inline]
    pub fn create_table(table: Table) -> Vop {
        Vop::CreateTable { table }
    }

    #[inline]
    pub fn drop(table: String) -> Vop {
        Vop::Drop { table }
    }

    #[inline]
    pub fn insert(tbl: String, row: usize) -> Vop {
        Vop::Insert { tbl, row }
    }

    #[inline]
    pub fn init() -> Vop {
        Vop::Init
    }

    #[inline]
    pub fn next(jmp: usize) -> Vop {
        Vop::Next { jmp }
    }

    #[inline]
    pub fn open(table: String) -> Vop {
        Vop::Open { table }
    }

    #[inline]
    pub fn rewind(jmp: usize) -> Vop {
        Vop::Rewind { jmp }
    }
}

/// VM holds the state of the virtual machine.
pub struct VM<'r> {
    db: &'r Rho,
    pc: usize,
    program: Code,
    cursors: Vec<Cursor>,
    // moving from registers to stack for simplicity..
    stack: Vec<Value>,
}

impl<'r> VM<'r> {
    pub fn init(db: &Rho, program: Code) -> VM {
        VM {
            db,
            pc: 0,
            program,
            cursors: vec![],
            //
            stack: vec![],
        }
    }

    pub fn next(&mut self) -> Result<Option<Record>> {
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
                Vop::Insert { tbl, row } => {
                    unsupported!("insert not supported")
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
                Vop::LoadC(idx) => {
                    let row = self.cursors[*idx].row();
                    // println!("PUSH {:?}", row);
                    self.push(row);
                }
                Vop::LoadV(idx) => {
                    let v = self.stack[*idx].clone();
                    // println!("VAR: {} ", v);
                    self.push(v);
                }
                Vop::Obj => {
                    self.stack.push(Value::object());
                }
                Vop::ObjAssign { name } => {
                    let val = self.pop();
                    let obj = self.peek();
                    // println!("OBJ_ASSIGN: {}: {} ", name, val);
                    obj.set(name.to_string(), val);
                }
                Vop::ObjSpread => {
                    let val = self.pop();
                    let obj = self.peek();
                    // println!("OBJ_SPREAD: {} ", val);
                    obj.spread(val);
                }
                Vop::Open { table } => {
                    self.cursors.push(self.db.scan(table)?);
                }
                Vop::Pop => {
                    let _ = self.pop();
                }
                Vop::Push(v) => {
                    self.push(v.clone());
                }
                Vop::Return => {
                    let v = self.pop();
                    return Ok(Some(v));
                }
                Vop::Rewind { jmp } => {
                    if self.cursors[0].is_empty() {
                        self.pc = *jmp;
                    }
                }
                Vop::Next { jmp } => {
                    if self.cursors[0].next() {
                        self.pc = *jmp;
                    }
                }
                Vop::Transaction => {
                    self.db.transaction();
                }
                Vop::Exit => {
                    return Ok(None);
                }
            }
        }
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
    }

    fn peek(&mut self) -> &mut Value {
        self.stack.last_mut().unwrap()
    }
}
