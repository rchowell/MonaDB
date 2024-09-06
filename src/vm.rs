use std::collections::HashMap;
use std::vec;

use serde_json::Value;

use crate::value::JValue;
use crate::Result;
use crate::{table::Table, value::Row, Rho};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

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
    /// Produce a row from the current environment.
    /// 
    /// Description:
    ///   Transforms the current environment into a row (JSON object).
    /// 
    Row,
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
    ///  * var  :   variable name
    ///  * jmp  :   jump location
    ///
    /// Description:
    ///   Advances the cursor, assigning the row to `var`.
    ///   If there is a row, then jump to `jmp`; otherwise goto next.
    ///
    Next { var: String, jmp: usize },
    ///
    /// Spread
    ///
    /// Description:
    ///   Produces a row by spreading all structs in the environment into the result.
    ///   Non-struct values are omitted, and members may be overridden.
    /// 
    Spread,
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
    pub fn row() -> Vop {
        Vop::Row
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
    pub fn next(var: &str, jmp: usize) -> Vop {
        Vop::Next {
            var: var.to_string(),
            jmp,
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

    /// Concatenates the bindings of the two environments.
    pub fn concat(self, _rhs: Self) -> Self {
        todo!()
    }

    /// Sets the current binding to this
    pub fn set(&mut self, key: &str, row: Row) {
        self.bindings.insert(key.to_string(), row);
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
    env: Env,
    // temporary until I have an actual cursor
    cursor: Vcursor,
}

impl<'a> VM<'a> {
    pub fn new(db: &Rho) -> VM {
        VM {
            db,
            env: Env::new(),
            cursor: Vcursor::empty(),
        }
    }

    pub fn execute(&mut self, program: &Program) -> Result<()> {
        let mut pc: usize = 0;
        loop {
            let op = &program[pc];
            pc += 1;
            match op {
                Vop::Init => {
                    // do nothing
                },
                Vop::CreateTable { table } => {
                    self.db.create_table(table)?;
                    break;
                }
                Vop::Insert { table, row } => {
                    self.db.insert(table, row.clone())?;
                    break;
                }
                Vop::DropTable { table } => {
                    self.db.drop_table(table)?;
                    break;
                }
                Vop::Row => {
                    let row = self.env.to_obj();
                    println!("{}", row);
                }
                Vop::Spread => {
                    let row = self.env.spread();
                    println!("{}", row);
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
                Vop::Next { var, jmp } => {
                    if let Some(row) = self.cursor.next() {
                        self.env.set(var, row);
                        pc = *jmp;
                    }
                }
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

    pub fn next(&mut self) -> Option<Row> {
        if self.pos > self.end {
            // exhausted
            return None;
        }
        let row = self.row();
        self.pos += 1;
        Some(row)
    }

    #[inline]
    fn row(&self) -> Row {
        self.rows[self.pos].clone()
    }
}
