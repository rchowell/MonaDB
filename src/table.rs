use std::fmt::Display;


// #[macro_export]
// macro_rules! row {
//     ($($json:tt)+) => {
//         Value::new(json!($($json)+))
//     };
// }

pub struct Value(serde_json::Value);

impl Value {

    pub fn new(value: serde_json::Value) -> Value {
        Value(value)
    }

    pub fn next(&self) -> Value {
        todo!()
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

// A table (for now) is a vector of rows.
pub struct Table {
    pub rows: Vec<Row>,
}

// A row (for now) is just a JSON value
pub type Row = Value;

impl Table {
    pub fn new() -> Table {
        Table { rows: Vec::new() }
    }

    pub fn insert(&mut self, row: Row) {
        self.rows.push(row);
    }

    pub fn row(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
}

/// TODO THIS GOES IN REVERSE ORDER
impl Iterator for Table {
    type Item = Row;

    fn next(&mut self) -> Option<Self::Item> {
        self.rows.pop()
    }
}