//! Shared report labels and environment banner for the benchmark harnesses.

use std::env;

/// Formats cardinality for reports (`empty`, `10k`, `100k`).
pub fn format_cardinality(n: usize) -> String {
    if n == 0 {
        "empty".into()
    } else if n % 1000 == 0 {
        format!("{}k", n / 1000)
    } else {
        n.to_string()
    }
}

/// Prints runtime environment metadata at benchmark start.
pub fn print_environment(sqlite_version: &str) {
    let host = env::var("HOSTNAME")
        .or_else(|_| env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".into());
    println!("MonaDB doc_workloads benchmark");
    println!("  monadb: {}", env!("CARGO_PKG_VERSION"));
    println!("  sqlite: {sqlite_version}");
    println!("  host:   {host}");
}
