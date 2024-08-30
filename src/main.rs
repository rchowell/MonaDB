use std::env;
use std::path::PathBuf;

use rho::table::Table;
use rho::{row, Rho};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <table>", args[0]);
        std::process::exit(1);
    }
    let path: PathBuf = PathBuf::from(&args[1]);

    // let rho = Rho::open(&path).expect("Could not open rho");
    // rho.exec("rql..".to_string());

    println!();
    println!("Done");

    println!("opening table..");
    let mut table = Table::open(&path).expect("Could not open table");
    table.insert(row!({"name": "Alice", "age": 25}));
    table.insert(row!({"name": "Bob", "age": 25}));
    println!("closing table..");
    table.close().expect("Could not close table");
    println!();
    println!("Done");
}
