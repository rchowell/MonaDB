use crate::{table::{Row, Table}, value::Value};

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
    /// `INSERT P1 * * P4`
    /// 
    /// P1: Table cursor to insert.\
    /// P4: Row to insert.\
    /// 
    /// Insert a row at the cursor.
    Insert,
    ///
    /// `ROW P1 P2 * *`
    /// 
    /// P1: Register for the row's start.\
    /// P2: Register for the row's end.\
    /// 
    /// Return a row whose values are registers[P1@P2]
    Row,
    ///
    /// `RETURN * * * *`
    /// 
    /// Return from the program.
    Return,
}

/// Vop is a virtual machine instruction.
pub struct Vop {
    code: Vcode,
    p1: usize,
    p2: usize,
    p3: usize,
    p4: Option<Value>,
}

impl Vop {

    pub fn next(p1: usize, p2: usize) -> Vop {
        Vop { code: Vcode::Next, p1, p2, p3: 0, p4: None }
    }

    pub fn row(p1: usize, p2: usize) -> Vop {
        Vop { code: Vcode::Row, p1, p2, p3: 0, p4: None }
    }

    pub fn return_() -> Vop {
        Vop { code: Vcode::Return, p1: 0, p2: 0, p3: 0, p4: None }
    }
    
}

/// Vcursor holds a position and table.
pub struct Vcursor<'a> {
    pos: usize,
    end: usize,
    table: &'a Table,
}

impl <'a> Vcursor<'a> {
    pub fn new(table: &'a Table) -> Vcursor<'a> {
        Vcursor {
            pos: 0,
            end: 0,
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
    pub fn row(&self) -> &Row {
        // self.table.row(self.pos).expect("Illegal cursor position")
        todo!()
    }
}

/// Vm holds the state of the virtual machine.
pub struct Vm<'a> {
    pub cursor: Vcursor<'a>,
    pub sink: Box<dyn Vsink>,
}

impl <'a> Vm<'a> {

    pub fn execute(&mut self, program: &Program) {
        use Vcode::*;
        let mut pc: usize = 0;
        loop {
            let op = &program[pc];
            pc += 1;
            match op.code {
                Next => {
                    if self.cursor.next() {
                        pc = op.p2;
                    }
                }
                Insert => {

                }
                Row => {
                    let row = self.cursor.row();
                    self.sink.write(row);
                },
                Return => {
                    // consider some kind of return code
                    return;
                }
            }
        }
    }
}

// TEMPORARY – How to handle output??
pub trait Vsink {
    fn write(&self, row: &Row);
}
