use rusqlite::Statement;

use crate::value::Value;
use crate::Result;

/// Cursor holds a prepared SQLite statement that can be stepped.
pub struct Cursor {
    /// for now, hold onto a vector.
    rows: Vec<Value>,
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
        let mut rows: Vec<Value> = vec![];
        while let Some(row) = query.next().unwrap() {
            let s: String = row.get(0).unwrap();
            let v = Value::from_str(&s).unwrap();
            rows.push(v);
        }

        let pos = 0;
        let end = rows.len();
        Self { rows, pos, end }
    }

    pub fn next(&mut self) -> Result<Option<Value>> {
        if self.pos >= self.end {
            Ok(None)
        } else {
            let value = self.rows[self.pos].clone();
            self.pos += 1;
            Ok(Some(value))
        }
    }
}
