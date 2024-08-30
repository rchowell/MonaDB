use std::{fs::File, io::{BufRead, BufReader, Read, Write}, path::Path, str::FromStr, vec};

use crate::{errors::RhoResult, value::Value};

#[macro_export]
macro_rules! row {
    ($($json:tt)+) => {
        rho::value::Value::new(serde_json::json!($($json)+))
    };
}

// A row (for now) is just a JSON value
pub type Row = Value;

// A table (for now) is a vector of rows.
pub struct Table {
    file: File,
    rows: Vec<Row>,
}

impl Table {

    pub fn open<P>(path: P) -> RhoResult<Table>
    where P: AsRef<Path> {

        // !!TEMPORARY !!
        // Load the rows from the file – not how it will be actually done.
        let mut rows: Vec<Row> = vec![];
        let buffer = std::fs::read(&path)?;
        let lines = buffer.lines();
        for line in lines {
            let l = line.unwrap();
            let str = l.as_str();
            let row = serde_json::Value::from_str(str).unwrap();
            rows.push(Value::new(row));
        }

        // Open the file for appending.
        let file = File::options().append(true).open(path)?;

        Ok(Table { file, rows })
    }

    // Write the table to the file.
    pub fn close(&mut self) -> RhoResult<()> {
        for row in &self.rows {
            let buf = row.to_vec();
            self.file.write_all(buf.as_slice())?;
            self.file.write_all(b"\n")?;
        }
        Ok(())
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