use rusqlite::Statement;

use crate::value::Value;

#[derive(Debug, Clone)]
pub struct Row {
    pub oid: u64,
    pub val: Value,
}

/// Cursor holds a prepared SQLite statement that can be stepped.
/// 
/// Usage:
///   1. always rewind before iterating.
///   2. always check next before curr.
/// 
pub struct Cursor {
    /// for now, hold a vector.
    rows: Vec<Row>,
    /// pos holds the cursor's current index.
    pos: usize,
    /// end holds the cursor's last index.
    end: usize,
}

impl Cursor {
    /// Create a new cursor from an SQLite statement.
    pub fn new(statement: Statement<'_>) -> Self {
        // TODO use rows and make it lazy.
        let mut statement = Box::new(statement);
        let mut query = statement.query([]).unwrap();
        let mut rows: Vec<Row> = vec![];
        while let Some(row) = query.next().unwrap() {
            let oid: u64 = row.get(0).unwrap();
            let s: String = row.get(1).unwrap();
            let val = Value::from(s);
            rows.push(Row { oid, val });
        }
        let pos = 0;
        let end = rows.len();
        Self { rows, pos, end }
    }

    /// Create a new Cursor from a vector of Rows.
    pub fn vec(rows: Vec<Row>) -> Self {
        let pos = 0;
        let end = rows.len();
        Self { rows, pos, end }
    }

    /// Advance the cursor to the next row; returns true if there is a next row.
    pub fn next(&mut self) -> bool {
        self.pos += 1;
        self.pos < self.end
    }

    /// Reset the cursor to the start; returns true if there is a next row.
    pub fn rewind(&mut self) -> bool {
        self.pos = 0;
        self.end > 0
    }

    /// Return a reference to the current row.
    pub fn curr(&self) -> &Row {
        self.rows.get(self.pos).expect("illegal cursor position")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cursor() {
        test_vector(0);
        test_vector(1);
        test_vector(10);
        test_vector(100);
    }

    fn test_vector(n: usize) {
        let rows: Vec<Row> = (0..n)
            .map(|i| Row {
                oid: i as u64,
                val: Value::null(),
            })
            .collect();
        let mut cursor = Cursor::vec(rows);
        test_count(&mut cursor, n);
        let _ = cursor.rewind();
        test_count(&mut cursor, n);
    }

    fn test_count(cursor: &mut Cursor, n: usize) {
        // rewind
        let has_some = cursor.rewind();
        if !has_some {
            assert_eq!(n, 0);
            return;
        }
        // iterate with do-while
        let mut count = 0;
        while {
            let _ = cursor.curr();
            count += 1;
            cursor.next()
        } {}
        assert_eq!(count, n);
    }
}
