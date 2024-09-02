use crate::value::Value;

#[macro_export]
macro_rules! row {
    ($($json:tt)+) => {
        rho::value::Value::new(serde_json::json!($($json)+))
    };
}

/// A row (for now) is just a JSON value
pub type Row = Value;

/// A table (for now) is just a handle.
#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub rql: String,
}

impl Table {
    pub fn new(name: String) -> Table {
        Table { name, rql: "TODO".to_string() }
    }
}