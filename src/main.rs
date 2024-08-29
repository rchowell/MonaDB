use std::io;
use std::str::FromStr;

use table::Table;
use table::Value;
use vm::{Vcursor, Vm, Vop, Vsink};

mod table;
mod vm;

macro_rules! row {
    ($($json:tt)+) => {
        Value::new(json!($($json)+))
    };
}

fn main() {

    // CREATE TABLE ... stdin
    let mut table = Table::new();
    let lines = io::stdin().lines();
    for line in lines {
        let l = line.unwrap();
        let str = l.as_str();
        let row = serde_json::Value::from_str(str).unwrap();
        table.insert(Value::new(row));
    }

    // Initialize the virtual machine.
    let mut vm = Vm {
        cursor: Vcursor::new(table),
        sink: Box::new(Printer {}),
    };

    // Hardcoded scan.
    let program = vec![
        Vop::row(0, 0),  // 0
        Vop::next(0, 0), // 1
        Vop::return_(),  // 2
    ];
    vm.execute(&program);

    println!();
    println!("Done")
}

struct Printer {}

impl Vsink for Printer {
    fn write(&self, row: &table::Row) {
        println!("{}", row);
    }
}
