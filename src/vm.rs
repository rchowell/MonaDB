use std::collections::HashMap;
use std::vec;

use serde_json::Value;

use crate::value::JValue;
use crate::Result;
use crate::{table::Table, value::Row, Rho};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Registers are the VM's working memory.
pub type Vmem = Vec<JValue>;

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
    /// Insert a row into a table.
    Insert { table: String, row: Row },
    /// JSON Path Index
    ///   * idx     : index to lookup
    ///   * inp     : operand register
    ///   * dest    : result register
    Jpi {
        idx: usize,
        inp: usize,
        dest: usize,
    },
    /// JSON Path Key
    ///   * key     : key to lookup
    ///   * inp     : input register
    ///   * dest    : result register
    Jpk {
        key: String,
        inp: usize,
        dest: usize,
    },
    ///
    /// Obj
    ///   * members : key:register pairs.
    ///   * dest    : result register.
    ///
    /// TODO use contiguous memory // two-pass for objects!!
    ///
    Obj { 
        members: Vec<(String, usize)>,
        dest: usize,
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
    ///
    /// Next
    ///  * jmp  :   jump location
    ///
    /// Description:
    ///   Advances the cursor, assigning the row to `var`.
    ///   If there is a row, then jump to `jmp`; otherwise goto next.
    ///
    Next { jmp: usize },
    ///
    /// Spread
    ///
    /// Description:
    ///   Produces a row by spreading all structs in the environment into the result.
    ///   Non-struct values are omitted, and members may be overridden.
    ///
    Spread { dest: usize },
    /// Opens a table for reading with the cursor positioned at the first row.
    Open {
        /// TODO replace table with cursor.
        table: String,
    },
    /// Load the variable `name` from the environment into the destination register.
    Var { name: String, dest: usize },
    /// Exit the VM.
    Exit,
}

impl Vop {
    #[inline]
    pub fn exit() -> Vop {
        Vop::Exit
    }

    #[inline]
    pub fn bind(binder: &str) -> Vop {
        Vop::Bind {
            cursor: 0,
            binder: binder.to_string(),
        }
    }

    #[inline]
    pub fn clear(table: &str) -> Vop {
        Vop::Clear {
            table: table.to_string(),
        }
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
    pub fn insert(table: String, row: Row) -> Vop {
        Vop::Insert { table, row }
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
    pub fn obj(members: Vec<(String, usize)>, dest: usize) -> Vop {
        Vop::Obj { members, dest }
    }

    #[inline]
    pub fn open(table: &str) -> Vop {
        Vop::Open {
            table: table.to_string(),
        }
    }

    #[inline]
    pub fn rewind(jmp: usize) -> Vop {
        Vop::Rewind { jmp }
    }

    #[inline]
    pub fn spread(dest: usize) -> Vop {
        Vop::Spread { dest }
    }

    #[inline]
    pub fn var(name: &str, dest: usize) -> Vop {
        Vop::Var {
            name: name.to_string(),
            dest,
        }
    }

    #[inline]
    pub fn jpk(key: &str, inp: usize, dest: usize) -> Vop {
        Vop::Jpk {
            key: key.to_string(),
            inp,
            dest,
        }
    }

    #[inline]
    pub fn jpi(inp: usize, idx: usize, dest: usize) -> Vop {
        Vop::Jpi {
            idx,
            inp,
            dest,
        }
    }
}

/// The bindings environment.
pub struct Env {
    bindings: HashMap<String, JValue>,
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
    pub fn get(&mut self, key: &str) -> JValue {
        if let Some(v) = self.bindings.get(key) {
            v.clone()
        } else {
            JValue::null()
        }
    }

    /// Produces a row by spreading all structs in the environment into the result.
    pub fn spread(&self) -> JValue {
        let mut members = serde_json::Map::new();
        for (_, v) in &self.bindings {
            if let Some(member) = v.members() {
                for (k, v) in member {
                    members.insert(k, v.into());
                }
            }
        }
        Value::Object(members).into()
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
}

impl<'a> VM<'a> {
    pub fn init(db: &Rho, program: Program) -> VM {
        // temporary (??)
        let mut mem: Vmem = vec![];
        mem.resize(100, JValue::null());

        VM {
            db,
            mem,
            pc: 0,
            program,
            env: Env::new(),
            cursor: Vcursor::empty(),
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
                // TEMP – bind the row to the environment
                Vop::Bind { binder, .. } => {
                    let row = self.cursor.row();
                    self.env.set(&binder, row);
                }
                Vop::Drop { table } => {
                    self.db.drop_table(table)?;
                }
                Vop::Insert { table, row } => {
                    self.db.insert(table, row.clone())?;
                }
                Vop::Jpi { inp, idx, dest } => {
                    self.mem[*dest] = match self.mem[*inp].jpi(*idx) {
                        Some(v) => v,
                        None => JValue::null(),
                    };
                }
                Vop::Jpk { key, inp, dest } => {
                    self.mem[*dest] = match self.mem[*inp].jpk(key) {
                        Some(v) => v,
                        None => JValue::null(),
                    };
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
                Vop::Obj { members, dest } => {
                    let mut map = serde_json::Map::new();
                    for (k, v) in members {
                        let v = self.mem[*v].clone();
                        let k = k.clone();
                        map.insert(k, v.into());
                    }
                    self.mem[*dest] = Value::Object(map).into();
                }
                Vop::Spread { dest } => {
                    self.mem[*dest] = self.env.spread();
                }
                Vop::Next { jmp } => {
                    if self.cursor.next() {
                        self.pc = *jmp;
                    }
                }
                Vop::Var { name, dest } => {
                    self.mem[*dest] = self.env.get(name);
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
        let len = rows.len();
        let end = match len {
            0 => 0,
            _ => len - 1,
        };
        Self { rows, pos, end }
    }

    pub fn is_empty(&self) -> bool {
        self.end == 0
    }

    pub fn next(&mut self) -> bool {
        if self.pos < self.end {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn row(&self) -> Row {
        self.rows[self.pos].clone()
    }
}
