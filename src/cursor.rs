use crate::value::Value;

#[derive(Debug, Clone)]
pub struct Row {
    pub oid: u64,
    pub val: Value,
}

pub struct Cursor {
    /// for now, hold a vector.
    rows: Vec<Row>,
    /// pos holds the cursor's current index.
    pos: usize,
    /// end holds the cursor's last index.
    end: usize,
}

impl Cursor {

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
