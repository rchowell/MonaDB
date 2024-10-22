use std::vec;

use crate::cursor::Cursor;
use crate::ir::Table;
use crate::value::Value;
use crate::Result;
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
    Jpi { inp: usize, idx: usize, dst: usize },
    /// JSON Path Key
    Jpk { inp: usize, key: String, dst: usize },
    /// JSON Path Expression
    Jpe { inp: usize, exp: usize, dst: usize },
    /// Load a value into a register.
    Load { val: Value, dst: usize },
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
    /// Push a value from the cursor to the stack.
    Push { cursor: usize },
    /// Returns the value at the top of the stack.
    Return,
    /// Set cursor to the start; jump to `jmp` if the table is empty.
    Rewind { jmp: usize },
    /// Begin a transaction.
    Transaction,
    /// `stack.push(stack[idx])`
    Var {
        idx: usize,
    },
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
    pub fn jpk(inp: usize, key: String, dst: usize) -> Vop {
        Vop::Jpk { inp, key, dst }
    }

    #[inline]
    pub fn jpi(inp: usize, idx: usize, dst: usize) -> Vop {
        Vop::Jpi { inp, idx, dst }
    }

    #[inline]
    pub fn jpe(inp: usize, exp: usize, dst: usize) -> Vop {
        Vop::Jpe { inp, exp, dst }
    }

    #[inline]
    pub fn load(val: Value, dst: usize) -> Vop {
        Vop::Load { val, dst }
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
    pub fn push(cursor: usize) -> Vop {
        Vop::Push { cursor }
    }

    #[inline]
    pub fn rewind(jmp: usize) -> Vop {
        Vop::Rewind { jmp }
    }

    #[inline]
    pub fn var(idx: usize) -> Vop {
        Vop::Var { idx }
    }
}

/// VM holds the state of the virtual machine.
pub struct VM<'r> {
    db: &'r Rho,
    pc: usize,
    program: Code,
    mem: Vec<Value>,
    cursors: Vec<Cursor>,
    // moving from registers to stack for simplicity..
    stack: Vec<Value>,
}

impl<'r> VM<'r> {
    pub fn init(db: &Rho, program: Code) -> VM {
        VM {
            db,
            mem: vec![Value::null(); 100],
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
                    let v = self.load(*row).clone(); // TODO NO CLONE
                    self.db.insert(tbl, v)?;
                }
                Vop::Jpi { inp, idx, dst } => {
                    self.mem[*dst] = match self.mem[*inp].jpi(*idx) {
                        Some(v) => v,
                        None => Value::null(),
                    };
                }
                Vop::Jpk { inp, key, dst } => {
                    self.mem[*dst] = match self.mem[*inp].jpk(key) {
                        Some(v) => v,
                        None => Value::null(),
                    };
                }
                Vop::Jpe { inp, exp, dst } => {
                    let e = &self.mem[*exp];
                    // json path index
                    if let Some(idx) = e.as_u64() {
                        self.mem[*dst] = match self.mem[*inp].jpi(idx as usize) {
                            Some(v) => v,
                            None => Value::null(),
                        };
                        continue;
                    }
                    // json path key
                    if let Some(key) = e.as_str() {
                        self.mem[*dst] = match self.mem[*inp].jpk(key) {
                            Some(v) => v,
                            None => Value::null(),
                        };
                        continue;
                    }
                    self.mem[*dst] = Value::null();
                }
                Vop::Obj => {
                    self.stack.push(Value::object());
                }
                Vop::ObjAssign { name } => {
                    let val = self.pop();
                    let obj = self.peek_mut();
                    // println!("OBJ_ASSIGN: {}: {} ", name, val);
                    obj.set(name.to_string(), val);
                }
                Vop::ObjSpread => {
                    let val = self.pop();
                    let obj = self.peek_mut();
                    // println!("OBJ_SPREAD: {} ", val);
                    obj.spread(val);
                }
                Vop::Open { table } => {
                    self.cursors.push(self.db.scan(table)?);
                }
                Vop::Pop => {
                    let _ = self.pop();
                }
                Vop::Push { cursor } => {
                    let row = self.cursors[*cursor].row();
                    // println!("PUSH {:?}", row);
                    self.push(row);
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
                Vop::Var { idx } => {
                    let v = self.stack[*idx].clone();
                    // println!("VAR: {} ", v);
                    self.push(v);
                }
                Vop::Load { val, dst } => {
                    self.mem[*dst] = val.clone();
                }
                Vop::Exit => {
                    return Ok(None);
                }
            }
        }
    }

    fn load(&self, idx: usize) -> &Value {
        self.mem.get(idx).unwrap()
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
    }

    fn peek(&mut self) -> &Value {
        self.stack.last().unwrap()
    }

    fn peek_mut(&mut self) -> &mut Value {
        self.stack.last_mut().unwrap()
    }
}
