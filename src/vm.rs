use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::vec;

use crate::ir::Table;
use crate::value::{ObjInitializer, Value};
use crate::Result;
use crate::{value::Row, Rho};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Registers are the VM's working memory.
pub type Vmem = Vec<Value>;

/// Vop is a virtual machine instruction code.
#[derive(Debug)]
pub enum Vop {
    ///
    /// Init is always the first instruction.
    ///
    Init,
    /// Bind the row from the cursor to the binder like `Column` from SQLite.
    Bind { cursor: usize, binder: String },
    /// Delete all rows of the table.
    Clear { table: String },
    ///
    Commit,
    ///
    /// Insert the table into the catalog table.
    ///   * table   : table to create
    ///
    /// Description:
    ///  Invokes the catalog to create the given table.
    ///
    /// Consider replacing with an `Insert` instruction on the catalog table.
    ///
    CreateTable { table: Table },
    ///
    /// Drop
    ///   * table   : table to drop
    ///
    /// Description:
    ///   Invokes the catalog to drop the given table.
    ///
    /// Consider replacing with a `Delete` instruction on the catalog table.
    ///
    Drop { table: String },
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
    ///
    /// Next
    ///  * jmp  :   jump location
    ///
    /// Description:
    ///   Advances the cursor, assigning the row to `var`.
    ///   If there is a row, then jump to `jmp`; otherwise goto next.
    ///
    Next { jmp: usize },
    /// Begin an object initializer.
    ObjInit,
    /// Assgin the value in `expr` to `name` the current object initializer.
    ObjAssign {
        name: String,
        expr: usize,
    },
    /// Spread the value in `expr` into the current object initializer.
    ObjSpread {
        expr: usize,
    },
    /// Complete the object initializer.
    ObjDone {
        dst: usize,
    },
    /// Opens a table for reading with the cursor positioned at the first row.
    Open {
        /// TODO replace table with cursor.
        table: String,
    },
    /// Returns the value in register `ptr`.
    Return { ptr: usize },
    ///
    /// Rewind
    ///  * jmp  :   jump location
    ///
    /// Description:
    ///   Set cursor to the start; jump to `jmp` if the table is empty.
    ///
    Rewind { jmp: usize },
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
    pub fn obj_init() -> Vop {
        Vop::ObjInit
    }

    #[inline]
    pub fn obj_assign(name: String, expr: usize) -> Vop {
        Vop::ObjAssign { name, expr }
    }

    #[inline]
    pub fn obj_spread(expr: usize) -> Vop {
        Vop::ObjSpread { expr }
    }

    #[inline]
    pub fn obj_done(dst: usize) -> Vop {
        Vop::ObjDone { dst }
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
    pub fn set(&mut self, key: &str, row: Row) {
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
pub struct VM<'a> {
    db: &'a Rho,
    mem: Vmem,
    pc: usize,
    program: Program,
    // temporary until the registers are implemented
    env: Env,
    // temporary until I have an actual cursor
    cursor: Vcursor,
    // Current Object Initliazer .. temporary until I have an environment stack
    coi: ObjInitializer,
}

impl<'a> VM<'a> {
    pub fn init(db: &Rho, program: Program) -> VM {
        // temporary (??)
        let mut mem: Vmem = vec![];
        mem.resize(100, Value::null());

        VM {
            db,
            mem,
            pc: 0,
            program,
            env: Env::new(),
            cursor: Vcursor::empty(),
            coi: ObjInitializer::init(),
        }
    }

    pub fn next(&mut self) -> Result<Option<Row>> {
        loop {
            let op = &self.program[self.pc];
            self.pc += 1;
            match op {
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
                    let row = self.cursor.row();
                    self.env.set(&binder, row);
                }
                Vop::Drop { table } => {
                    self.db.drop_table(table)?;
                }
                Vop::Insert { tbl, row } => {
                    let v = self.mem[*row].clone();
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
                    // TEMP – load all into the fake "cursor"
                    let rows = self.db.select(table)?;
                    self.cursor = Vcursor::new(rows)
                }
                Vop::Return { ptr } => {
                    let v = self.mem[*ptr].clone();
                    return Ok(Some(v));
                }
                Vop::Rewind { jmp } => {
                    if self.cursor.is_empty() {
                        self.pc = *jmp;
                    }
                }
                Vop::ObjInit => {
                    self.coi.clear();
                },
                Vop::ObjAssign { name, expr } => {
                    let v = self.mem[*expr].clone();
                    self.coi.assign(name, v);
                },
                Vop::ObjSpread { expr } => {
                    let v = self.mem[*expr].clone();
                    self.coi.spread(v);
                },
                Vop::ObjDone { dst } => {
                    self.mem[*dst] = self.coi.done();
                },
                Vop::Next { jmp } => {
                    if self.cursor.next() {
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
}

/// Vcursor is an iterator-like interface backed by a table.
///
/// TODO this is preliminary.
pub struct Vcursor {
    /// for now, hold onto a vector.
    rows: Vec<Row>,
    /// pos holds the cursor's current index.
    pos: usize,
    /// end holds the cursor's last index.
    end: usize,
}

impl Vcursor {
    /// Hack to have an empty cursor for VM state.
    pub fn empty() -> Self {
        Self {
            rows: vec![],
            pos: 0,
            end: 0,
        }
    }

    /// Create a cursor over the vector of rows.
    pub fn new(rows: Vec<Row>) -> Self {
        let pos = 0;
        let end = rows.len();
        Self { rows, pos, end }
    }

    pub fn is_empty(&self) -> bool {
        self.end == 0
    }

    pub fn next(&mut self) -> bool {
        self.pos += 1;
        self.pos < self.end
    }

    #[inline]
    pub fn row(&self) -> Row {
        self.rows[self.pos].clone()
    }
}
