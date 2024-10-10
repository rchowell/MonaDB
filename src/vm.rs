use std::collections::HashMap;
use std::vec;

use crate::cursor::Cursor;
use crate::ir::Table;
use crate::value::Value;
use crate::Result;
use crate::{value::Record, Rho};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Vop is a virtual machine instruction code.
#[derive(Debug, Clone)]
pub enum Vop {
    /// Initialize the VM.
    Init,
    /// Bind the row from the cursor to the binder like `Column` from SQLite.
    Bind { 
        cursor: usize,
        binder: String,
    },
    /// Delete all rows of the table.
    Clear { 
        table: String,
    },
    ///
    Commit,
    /// Consider replacing with an `Insert` instruction on the catalog table.
    CreateTable { 
        table: Table,
    },
    /// Consider replacing with a `Delete` instruction on the catalog table.
    Drop { 
        table: String,
    },
    /// Insert row into a table.
    Insert { 
        tbl: String,
        row: usize,
    },
    /// JSON Path Index
    Jpi { 
        inp: usize,
        idx: usize,
        dst: usize,
    },
    /// JSON Path Key
    Jpk {
        inp: usize,
        key: String,
        dst: usize,
    },
    /// JSON Path Expression
    Jpe {
        inp: usize,
        exp: usize,
        dst: usize,
    },
    /// Load a value into a register.
    Load {
        val: Value,
        dst: usize,
    },
    /// Advances the cursor, assigning the row to `var`.
    /// If there is a row, then jump to `jmp`; otherwise goto next.
    Next {
        jmp: usize,
    },
    /// Initialize an empty object.
    Obj {
        dst: usize,
    },
    /// Opens a table for reading with the cursor positioned at the first row.
    Open {
        /// TODO replace table with cursor.
        table: String,
    },
    /// Returns the value in register `ptr`.
    Return { 
        ptr: usize,
    },
    /// Set cursor to the start; jump to `jmp` if the table is empty.
    Rewind {
        jmp: usize,
    },
    /// obj[name] = expr // if name is None, then spread.
    Set {
        obj: usize,
        name: Option<String>,
        expr: usize,
    },
    /// Begin a transaction.
    Transaction,
    /// Load the variable `name` from the environment into the destination register.
    Var { 
        var: String,
        dst: usize,
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
    pub fn bind(binder: String) -> Vop {
        Vop::Bind { cursor: 0, binder }
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
    pub fn obj(dst: usize) -> Vop {
        Vop::Obj { dst }
    }

    #[inline]
    pub fn open(table: String) -> Vop {
        Vop::Open { table }
    }

    #[inline]
    pub fn rewind(jmp: usize) -> Vop {
        Vop::Rewind { jmp }
    }

    #[inline]
    pub fn set(obj: usize, name: Option<String>, expr: usize) -> Vop {
        Vop::Set { 
            obj,
            name,
            expr,
         }
    }


    #[inline]
    pub fn var(var: String, dst: usize) -> Vop {
        Vop::Var { var, dst }
    }
}

/// The bindings environment.
pub struct Env {
    bindings: HashMap<String, Value>,
}

impl Env {
    /// Create an empty [Env].
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Sets the current binding to this
    pub fn set(&mut self, key: &str, row: Record) {
        self.bindings.insert(key.to_string(), row);
    }

    /// Gets the current binding for this key (or null).
    pub fn get(&mut self, key: &str) -> Value {
        if let Some(v) = self.bindings.get(key) {
            v.clone()
        } else {
            Value::null()
        }
    }
}

/// VM holds the state of the virtual machine.
pub struct VM<'r> {
    db: &'r Rho,
    pc: usize,
    program: Program,
    mem: Vec<Value>,
    cursors: Vec<Cursor>,
    // temporary until the registers are implemented
    env: Env,
}

impl<'r> VM<'r> {
    pub fn init(db: &Rho, program: Program) -> VM {
        VM {
            db,
            mem: vec![Value::null(); 100],
            pc: 0,
            program,
            env: Env::new(),
            cursors: vec![],
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
                // TEMP – bind the row to the environment
                Vop::Bind { binder, .. } => {
                    let row = self.cursors[0].row();
                    self.env.set(&binder, row);
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
                Vop::Jpe { inp, exp, dst  } => {
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
                Vop::Open { table } => {
                    self.cursors.push(self.db.scan(table)?);
                }
                Vop::Return { ptr } => {
                    let v = self.mem[*ptr].clone(); // TODO NO CLONE
                    return Ok(Some(v));
                }
                Vop::Rewind { jmp } => {
                    if self.cursors[0].is_empty() {
                        self.pc = *jmp;
                    }
                }
                Vop::Obj { dst } => {
                    self.mem[*dst] = Value::object();
                },
                Vop::Set { obj, name, expr } => {
                    let val = self.load(*expr).clone(); // TODO NO CLONE; BORROW+DROP
                    let obj = self.loadm(*obj); // <-- MUTABLE BORROW OCCURS HERE
                    match name {
                        Some(name) => obj.set(name.to_string(), val),
                        None => obj.spread(val),
                    }
                    // MUTABLE BORROW IS DROPPED
                },
                Vop::Next { jmp } => {
                    if self.cursors[0].next() {
                        self.pc = *jmp;
                    }
                }
                Vop::Transaction => {
                    self.db.transaction();
                }
                Vop::Var { var,  dst } => {
                    self.mem[*dst] = self.env.get(var);
                }
                Vop::Load { val,  dst } => {
                    self.mem[*dst] = val.clone();
                }
                Vop::Exit => {
                    return Ok(None);
                }
            }
        }
    }

    fn loadm(&mut self, idx: usize) -> &mut Value {
        self.mem.get_mut(idx).unwrap()
    }

    fn load(&self, idx: usize) -> &Value {
        self.mem.get(idx).unwrap()
    }
}
