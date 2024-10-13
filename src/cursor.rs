use rusqlite::Statement;

use crate::value::Record;

/// Cursor holds a prepared SQLite statement that can be stepped.
pub struct Cursor {
    // cols: Vec<TableMember>,
    /// for now, hold onto a vector.
    rows: Vec<Record>,
    /// pos holds the cursor's current index.
    pos: usize,
    /// end holds the cursor's last index.
    end: usize,
}

impl Cursor {

    pub fn new(statement: Statement<'_>) -> Self {

        // TODO use rows and make it lazy.
        let mut statement = Box::new(statement);
        let mut query = statement.query([]).unwrap();
        let mut rows: Vec<Record> = vec![];
        while let Some(row) = query.next().unwrap() {
            let s: String = row.get(0).unwrap();
            let v = Record::from_str(&s).unwrap();
            rows.push(v);
        }

        let pos = 0;
        let end = rows.len();
        Self { rows, pos, end }
    }

    pub fn is_empty(&self) -> bool {
        self.end == 0
    }

    pub fn next(&mut self) -> bool {
        self.pos += 1;
        self.pos < self.end
    }

    pub fn row(&self) -> Record {
        self.rows[self.pos].clone()
    }
}
