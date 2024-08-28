use serde_json::json;
use table::Table;
use table::Value;
use vm::{Vcursor, Vm, Vop, Vsink};

mod table;
mod vm;

macro_rules! row {
    // Hide distracting implementation details from the generated rustdoc.
    ($($json:tt)+) => {
        Value::new(json!($($json)+))
    };
}

fn main() {

    // CREATE TABLE
    let mut table = Table::new();
    table.insert(row!("a"));
    table.insert(row!("123"));
    table.insert(row!("false"));
    table.insert(row!({ "a": 1, "b": 2 }));
    table.insert(row!([1, 2, 3]));


    // testing...
    // let sink = Box::new(Printer {});
    // for row in table.rows {
    //     sink.write(&row);
    // }

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
}

struct Printer {}

impl Vsink for Printer {
    fn write(&self, row: &table::Row) {
        println!("{}", row);
    }
}
