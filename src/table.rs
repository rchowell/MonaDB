pub struct Value(serde_json::Value);

impl Value {

    pub fn next(&self) -> Value {
        todo!()
    }
}

// A table (for now) is a vector of rows.
pub struct Table {
    rows: Vec<Row>,
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

impl Iterator for Table {
    type Item = Row;

    fn next(&mut self) -> Option<Self::Item> {
        self.rows.pop()
    }
}