use crate::table::{Row, Table, Value};

/// Program is a sequence of virtual machine instructions.
pub type Program = Vec<Vop>;

/// Vcode is a virtual machine instruction code.
pub enum Vcode {
    ///
    /// `NEXT P1 P2 * *`
    /// 
    /// P1: Cursor to advance.\
    /// P2: Next instruction.\
    /// 
    /// Advance the cursor by P1 and jump to the instruction at P2. 
    /// If there are no more rows, fall through to the next instruction.
    /// If the advance was successful, jump to program[P2].
    Next,
    ///
    /// `ROW P1 P2 * *`
    /// 
    /// P1: Register for the row's start.\
    /// P2: Register for the row's end.\
    /// 
    /// Return a row whose values are registers[P1@P2]
    Row,
}

/// Vop is a virtual machine instruction.
pub struct Vop {
    code: Vcode,
    p1: u8,
    p2: u8,
    p3: u8,
    p4: Value,
}

/// Vcursor holds a position and table.
pub struct Vcursor {
    pos: usize,
    end: usize,
    table: Box<Table>,
}

impl Vcursor {
    pub fn new(table: Box<Table>) -> Vcursor {
        Vcursor {
            pos: 0,
            end: table.len() - 1,
            table,
        }
    }

    /// Advance the cursor, returning true iff the cursor was advanced.
    pub fn next(&mut self) -> bool {
        if self.pos < self.end {
            self.pos += 1;
            return true;
        }
        false
    }

    /// Return the current row.
    pub fn row(&self) -> Option<&Row> {
        self.table.row(self.pos)
    }
}

/// Vm holds the state of the virtual machine.
pub struct Vm {}

impl Vm {

    pub fn execute(table: &Table, program: &Program) {
        let mut pc: usize = 0;
        loop {
            let op = &program[pc];
            pc += 1;
            match op.code {
                Vcode::Next => {
                }
                Vcode::Row => {
                    return;
                },
            }
        }
    }
}
