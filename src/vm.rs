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
///
/// TODOs
/// - Lookup VM design patterns for Rust
/// - Consider codes from Lua and SQLite, but those are C
#[derive(Debug)]
pub enum Vop {
    ///
    /// Init is always the first instruction.
    /// 
    Init,
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
    /// Bind the row from the cursor to the binder like `Column` from SQLite.
    Bind {
        cursor: usize,
        binder: String,
    },
    /// Load the variable `name` from the environment into the destination register.
    Variable {
        name: String,
        dest: usize,
    },
    /// Insert a row into a table.
    Insert { table: String, row: Row },
    ///
    /// DropTable
    ///   * table   : table to drop
    ///
    /// Description:
    ///   Invokes the catalog to drop the given table.
    /// 
    /// Consider replacing with a `Delete` instruction on the catalog table.
    ///
    DropTable { table: String },
    ///
    /// Obj
    ///   * ptr     : register count for the row start.
    ///   * members : member names.
    /// 
    /// Description:
    ///   Construct an object with members { members[i]: mem[ptr + i] } for i in members.
    /// 
    Obj {
        ptr: usize,
        keys: Vec<String>,
    },
    ///
    /// Rewind
    ///  * jmp  :   jump location
    /// 
    /// Description:
    ///   Set cursor to the start; jump to `jmp` if the table is empty.
    /// 
    Rewind {
        jmp: usize,
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
    ///
    /// Spread
    ///
    /// Description:
    ///   Produces a row by spreading all structs in the environment into the result.
    ///   Non-struct values are omitted, and members may be overridden.
    /// 
    Spread {
        dest: usize,
    },
    ///
    /// Open
    /// 
    /// Description:
    ///   Opens a table for reading with the cursor positioned at the first row.
    /// 
    /// TODO replace table with cursor.
    /// 
    Open { table: String },
    /// Return from the VM – TODO merge with Vop::Row (??)
    Exit,
}

impl Vop {

    #[inline]
    pub fn exit() -> Vop {
        Vop::Exit
    }

    #[inline]
    pub fn bind(binder: &str) -> Vop {
        Vop::Bind { cursor: 0, binder: binder.to_string() }
    }

    #[inline]
    pub fn create_table(table: Table) -> Vop {
        Vop::CreateTable { table }
    }

    #[inline]
    pub fn drop_table(table: String) -> Vop {
        Vop::DropTable { table }
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
    pub fn obj(ptr: usize, members: Vec<String>) -> Vop {
        Vop::Obj {
            ptr,
            keys: members,
        }
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
        Vop::Variable {
            name: name.to_string(),
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

    /// Returns this [Env] as a [JValue] object.
    pub fn to_obj(&self) -> JValue {
        let mut members = serde_json::Map::new();
        for (k, v) in &self.bindings {
            members.insert(k.to_string(), v.clone().into());
        }
        Value::Object(members).into()
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
    // temporary until the registers are implemented
    env: Env,
    // temporary until I have an actual cursor
    cursor: Vcursor,
    mem: Vmem,
}

impl<'a> VM<'a> {
    pub fn new(db: &Rho) -> VM {
        VM {
            db,
            env: Env::new(),
            cursor: Vcursor::empty(),
            mem: vec![],
        }
    }

    // TEMP!!
    fn alloc(&mut self, n: usize) -> usize {
        let ptr = self.mem.len();
        self.mem.resize(ptr + n, JValue::null());
        ptr
    }

    pub fn execute(&mut self, program: &Program) -> Result<()> {
        let mut pc: usize = 0;
        loop {
            let op = &program[pc];
            pc += 1;
            match op {
                Vop::Init => {
                    // do nothing (for now)
                    self.alloc(100);
                },
                Vop::CreateTable { table } => {
                    self.db.create_table(table)?;
                }
                // TEMP – bind the row to the environment
                Vop::Bind { binder, .. } => {
                    let row = self.cursor.row();
                    self.env.set(&binder, row);
                }
                Vop::Insert { table, row } => {
                    self.db.insert(table, row.clone())?;
                }
                Vop::DropTable { table } => {
                    self.db.drop_table(table)?;
                }
                Vop::Open { table } => {
                    // TEMP – load all into the fake "cursor"
                    let rows = self.db.select(table)?;
                    self.cursor = Vcursor::new(rows)
                }
                Vop::Rewind { jmp } => {
                    if self.cursor.is_empty() {
                        pc = *jmp;
                    }
                }
                Vop::Obj { ptr, keys } => {

                    let n = keys.len();
                    let mut members = serde_json::Map::new();

                    for (i, k) in keys.iter().enumerate() {
                        let o = *ptr + i;
                        let v = self.mem[o].clone();
                        let k = k.clone();
                        members.insert(k, v.into());
                    }

                    let obj: JValue = Value::Object(members).into();
                    println!("{}", obj); // TODO assign the obj
                }
                Vop::Spread { dest } => {
                    // self.mem[*dest] = self.env.spread();
                    let obj = self.env.spread();
                    println!("{}", obj); // TODO assign the obj
                }
                Vop::Next { jmp } => {
                    if self.cursor.next() {
                        pc = *jmp;
                    }
                }
                Vop::Variable { name, dest } => {
                    self.mem[*dest] = self.env.get(name);
                },
                Vop::Exit => {
                    // TODO return codes.
                    break;
                }
            }
        }
        Ok(())
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
        let end = rows.len() - 1;
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

