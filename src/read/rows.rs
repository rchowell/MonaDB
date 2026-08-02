//! Pull-based row readers — one row at a time, no whole-file buffer.
//!
//! [`open_rows`] binds a file and [`RowReader::next_row`] yields its rows one at
//! a time, which is what lets a file back a VM cursor:
//!
//!   open_rows ─▶ next_row ─▶ next_row ─▶ … ─▶ None
//!
//! This is the crate's single parsing implementation: the eager
//! [`read_rows`](super::read_rows) simply drains a `RowReader`, so the streaming
//! cursor and the `read_csv`/`read_jsonl` builtins cannot drift apart.
//!
//! CSV and JSONL stream for real. JSON is the exception — a `[…]` document
//! cannot be pulled element-wise without an incremental parser, so
//! [`JsonRows`] parses the whole document up front and iterates it in memory.

use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::rc::Rc;

use csv::{Reader, ReaderBuilder, StringRecord};

use super::options::cell_value;
use super::{FileFormat, ReadOptions, read_err};
use crate::error::{Error, Result};
use crate::value::{Object, Value};

/// A pull-based reader over one tabular file.
pub enum RowReader {
    /// Streaming CSV/TSV records.
    Csv(CsvRows),
    /// Streaming newline-delimited JSON.
    Jsonl(JsonlRows),
    /// A whole JSON document, iterated in memory.
    Json(JsonRows),
}

impl RowReader {
    /// Returns the next row, or `None` at end of file.
    pub fn next_row(&mut self) -> Result<Option<Value>> {
        match self {
            RowReader::Csv(r) => r.next_row(),
            RowReader::Jsonl(r) => r.next_row(),
            RowReader::Json(r) => Ok(r.next_row()),
        }
    }
}

/// Opens `path` as a reader for `format`, applying `opts`.
///
/// The reader owns its `File`, so — unlike the btree scan in
/// [`Cursor::scan`](crate::cursor::Cursor::scan), whose heed iterator borrows
/// the storage env — no lifetime erasure and no `unsafe` are involved.
pub fn open_rows(path: &str, format: FileFormat, opts: &ReadOptions) -> Result<RowReader> {
    match format {
        FileFormat::Csv => Ok(RowReader::Csv(CsvRows::open(path, opts)?)),
        FileFormat::Tsv => Ok(RowReader::Csv(CsvRows::open(path, &opts.clone().for_tsv())?)),
        FileFormat::Jsonl => Ok(RowReader::Jsonl(JsonlRows::open(path, opts)?)),
        FileFormat::Json => Ok(RowReader::Json(JsonRows::open(path, opts)?)),
    }
}

/// Streaming CSV/TSV reader.
pub struct CsvRows {
    reader: Reader<File>,
    /// Refilled in place by `read_record` — one record buffer for the whole file.
    record: StringRecord,
    /// Column names, interned once at open; a row clones the `Rc` (a refcount
    /// bump), never the string.
    names: Vec<Rc<str>>,
    /// True when `names` were synthesized (`column0`, …), so they may still grow
    /// to match a wider record — the reader is `flexible`.
    synthesized: bool,
    /// True when `names` are pairwise distinct, which is what allows the
    /// [`Object::from_members`] fast path. Duplicate CSV headers (`a,b,a`) are
    /// legal input and must collapse last-wins through [`Object::insert`].
    unique: bool,
    path: Rc<str>,
    /// Data records seen so far; `skip` counts these, *after* the header.
    seen: usize,
    skip: usize,
}

impl CsvRows {
    fn open(path: &str, opts: &ReadOptions) -> Result<Self> {
        let file = File::open(path).map_err(|e| read_err(path, e))?;
        let mut reader = ReaderBuilder::new()
            .has_headers(opts.header)
            .delimiter(opts.delimiter as u8)
            .quote(opts.quote as u8)
            .flexible(true)
            .from_reader(file);

        let names: Vec<Rc<str>> = if opts.header {
            reader
                .headers()
                .map_err(|e| read_err(path, e))?
                .iter()
                .map(Rc::from)
                .collect()
        } else if opts.columns.is_empty() {
            Vec::new()
        } else {
            opts.columns.iter().map(|c| Rc::from(c.as_str())).collect()
        };

        // No names at open — synthesize `column{i}` on demand, as the eager
        // reader did per record.
        let synthesized = names.is_empty();
        let unique = names
            .iter()
            .enumerate()
            .all(|(i, n)| !names[..i].contains(n));

        Ok(Self {
            reader,
            record: StringRecord::new(),
            names,
            synthesized,
            unique,
            path: Rc::from(path),
            seen: 0,
            skip: opts.skip,
        })
    }

    fn next_row(&mut self) -> Result<Option<Value>> {
        loop {
            if !self
                .reader
                .read_record(&mut self.record)
                .map_err(|e| read_err(&self.path, e))?
            {
                return Ok(None);
            }
            let n = self.seen;
            self.seen += 1;
            if n >= self.skip {
                break;
            }
        }

        if self.synthesized && self.names.len() < self.record.len() {
            for i in self.names.len()..self.record.len() {
                self.names.push(Rc::from(format!("column{i}").as_str()));
            }
        }

        let fields = self.names.iter().zip(self.record.iter());
        let obj = if self.unique {
            Object::from_members(
                fields
                    .map(|(name, field)| (Rc::clone(name), cell_value(field)))
                    .collect(),
            )
        } else {
            let mut obj = Object::new();
            for (name, field) in fields {
                obj.insert(Rc::clone(name), cell_value(field));
            }
            obj
        };
        Ok(Some(Value::Object(Rc::new(obj))))
    }
}

/// Streaming newline-delimited JSON reader.
pub struct JsonlRows {
    lines: Lines<BufReader<File>>,
    path: Rc<str>,
    /// Raw lines seen so far; `skip` counts these, blanks included.
    line_no: usize,
    skip: usize,
}

impl JsonlRows {
    fn open(path: &str, opts: &ReadOptions) -> Result<Self> {
        let file = File::open(path).map_err(|e| read_err(path, e))?;
        Ok(Self {
            lines: BufReader::new(file).lines(),
            path: Rc::from(path),
            line_no: 0,
            skip: opts.skip,
        })
    }

    fn next_row(&mut self) -> Result<Option<Value>> {
        loop {
            let Some(line) = self.lines.next() else {
                return Ok(None);
            };
            let n = self.line_no;
            self.line_no += 1;
            // Checked before the line is unwrapped, so a skipped line's read
            // error stays suppressed — the eager reader's behavior.
            if n < self.skip {
                continue;
            }
            let line = line.map_err(|e| read_err(&self.path, e))?;
            if line.trim().is_empty() {
                continue;
            }
            let value = Value::decode(line.as_bytes())
                .map_err(|e| Error::IoError(format!("{}:{e:?}", self.path)))?;
            if !value.is_object() {
                return Err(Error::InternalError(format!(
                    "jsonl row at {}:{} must be a JSON object",
                    self.path,
                    n + 1
                )));
            }
            return Ok(Some(value));
        }
    }
}

/// A whole JSON document, iterated in memory.
///
/// A top-level array is the row bag; a top-level object is a single row. Rows
/// are not required to be objects — `[1, 2, 3]` iterates like the value source
/// `from [1, 2, 3] as x` does.
pub struct JsonRows {
    rows: Vec<Value>,
    pos: usize,
}

impl JsonRows {
    fn open(path: &str, opts: &ReadOptions) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|e| read_err(path, e))?;
        let value = Value::decode(&bytes).map_err(|e| Error::IoError(format!("{path}:{e:?}")))?;
        let rows = match value {
            Value::Array(items) => Rc::try_unwrap(items).unwrap_or_else(|rc| (*rc).clone()),
            value @ Value::Object(_) => vec![value],
            _ => {
                return Err(Error::IoError(format!(
                    "{path}: json source must be an array or an object"
                )));
            }
        };
        // `pos` is the cursor, so `skip` is just its start — no second vector.
        Ok(Self {
            rows,
            pos: opts.skip,
        })
    }

    fn next_row(&mut self) -> Option<Value> {
        let row = self.rows.get(self.pos)?.clone();
        self.pos += 1;
        Some(row)
    }
}
