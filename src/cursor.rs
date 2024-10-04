use crate::value::Record;

/// Cursor is an iterator-like interface backed by a table.
///
/// TODO this is preliminary.
pub struct Cursor {
    /// for now, hold onto a vector.
    rows: Vec<Record>,
    /// pos holds the cursor's current index.
    pos: usize,
    /// end holds the cursor's last index.
    end: usize,
}

impl Cursor {
    /// Hack to have an empty cursor for VM state.
    pub fn empty() -> Self {
        Self {
            rows: vec![],
            pos: 0,
            end: 0,
        }
    }

    /// Create a cursor over the vector of rows.
    pub fn new(rows: Vec<Record>) -> Self {
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
