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
pub struct Table {
    pub name: String,
    pub rql: String,
    pub cols: Vec<String>,
}