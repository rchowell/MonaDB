//! CSV, TSV, and JSONL file readers and writers for the SQL file-I/O surface.
//!
//! All readers produce `Vec<Value>` of row objects; writers consume the same
//! shape. Options are parsed from runtime `Value::Object` literals.

mod options;

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::rc::Rc;

use csv::{ReaderBuilder, WriterBuilder};

pub use options::{ReadOptions, WriteOptions};
use options::cell_value;

use crate::error::{Error, Result};
use crate::value::{Object, Value};

/// Supported on-disk tabular formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Comma-separated values.
    Csv,
    /// Tab-separated values.
    Tsv,
    /// Newline-delimited JSON objects.
    Jsonl,
}

/// Infers the file format from a path's extension.
pub fn infer_format(path: &str) -> Option<FileFormat> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)?;
    match ext.as_str() {
        "csv" => Some(FileFormat::Csv),
        "tsv" => Some(FileFormat::Tsv),
        "jsonl" | "ndjson" => Some(FileFormat::Jsonl),
        _ => None,
    }
}

/// Returns true when `path` has a recognized tabular file extension.
pub fn looks_like_file(path: &str) -> bool {
    infer_format(path).is_some()
}

/// Reads all rows from `path` using `format` and `opts`.
pub fn read_rows(path: &str, format: FileFormat, opts: ReadOptions) -> Result<Vec<Value>> {
    match format {
        FileFormat::Csv => read_csv(path, opts),
        FileFormat::Tsv => read_csv(path, opts.for_tsv()),
        FileFormat::Jsonl => read_jsonl(path, opts),
    }
}

/// Writes `rows` to `path` using `format` and `opts`.
pub fn write_rows(path: &str, format: FileFormat, opts: WriteOptions, rows: &[Value]) -> Result<()> {
    match format {
        FileFormat::Csv => write_csv(path, opts, rows),
        FileFormat::Tsv => write_csv(path, opts.for_tsv(), rows),
        FileFormat::Jsonl => write_jsonl(path, rows),
    }
}

/// Default alias for a file path: the filename stem (`test.csv` → `test`).
pub fn default_alias(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string()
}

/// Builtin name for reading `format`.
pub fn read_builtin(format: FileFormat) -> &'static str {
    match format {
        FileFormat::Csv | FileFormat::Tsv => "read_csv",
        FileFormat::Jsonl => "read_jsonl",
    }
}

/// Builtin name for writing `format`.
pub fn write_builtin(format: FileFormat) -> &'static str {
    match format {
        FileFormat::Csv | FileFormat::Tsv => "write_csv",
        FileFormat::Jsonl => "write_jsonl",
    }
}

fn read_csv(path: &str, opts: ReadOptions) -> Result<Vec<Value>> {
    let file = File::open(path).map_err(|e| read_err(path, e))?;
    let mut reader = ReaderBuilder::new()
        .has_headers(opts.header)
        .delimiter(opts.delimiter as u8)
        .quote(opts.quote as u8)
        .flexible(true)
        .from_reader(file);

    let headers: Vec<String> = if opts.header {
        reader
            .headers()
            .map_err(|e| read_err(path, e))?
            .iter()
            .map(str::to_string)
            .collect()
    } else if !opts.columns.is_empty() {
        opts.columns.clone()
    } else {
        Vec::new()
    };

    let mut rows = Vec::new();
    for (line_no, record) in reader.records().enumerate() {
        if line_no < opts.skip {
            continue;
        }
        let record = record.map_err(|e| read_err(path, e))?;
        let mut obj = Object::new();
        let names: Vec<String> = if headers.is_empty() {
            (0..record.len())
                .map(|i| format!("column{i}"))
                .collect()
        } else {
            headers.clone()
        };
        for (name, field) in names.iter().zip(record.iter()) {
            obj.insert(Rc::from(name.as_str()), cell_value(field));
        }
        rows.push(Value::Object(Rc::new(obj)));
    }
    Ok(rows)
}

fn read_jsonl(path: &str, opts: ReadOptions) -> Result<Vec<Value>> {
    let file = File::open(path).map_err(|e| read_err(path, e))?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        if line_no < opts.skip {
            continue;
        }
        let line = line.map_err(|e| read_err(path, e))?;
        if line.trim().is_empty() {
            continue;
        }
        let value = Value::decode(line.as_bytes()).map_err(|e| read_err(path, e))?;
        if !value.is_object() {
            return Err(Error::InternalError(format!(
                "jsonl row at {path}:{} must be a JSON object",
                line_no + 1
            )));
        }
        rows.push(value);
    }
    Ok(rows)
}

fn write_csv(path: &str, opts: WriteOptions, rows: &[Value]) -> Result<()> {
    let file = File::create(path).map_err(|e| read_err(path, e))?;
    let mut writer = WriterBuilder::new()
        .has_headers(opts.header)
        .delimiter(opts.delimiter as u8)
        .quote(opts.quote as u8)
        .from_writer(file);

    let columns: Vec<String> = rows
        .iter()
        .find_map(|row| row.members())
        .map(|members| members.into_iter().map(|(k, _)| k).collect())
        .unwrap_or_default();

    if opts.header && !columns.is_empty() {
        writer
            .write_record(columns.iter().map(String::as_str))
            .map_err(|e| read_err(path, e))?;
    }

    for row in rows {
        let members = row.members().ok_or_else(|| {
            Error::InternalError("write_csv() requires object rows".to_string())
        })?;
        let map: std::collections::HashMap<_, _> = members.into_iter().collect();
        let record: Vec<String> = columns
            .iter()
            .map(|col| map.get(col).map(cell_to_string).unwrap_or_default())
            .collect();
        writer
            .write_record(record.iter().map(String::as_str))
            .map_err(|e| read_err(path, e))?;
    }
    writer.flush().map_err(|e| read_err(path, e))?;
    Ok(())
}

fn write_jsonl(path: &str, rows: &[Value]) -> Result<()> {
    let mut file = File::create(path).map_err(|e| read_err(path, e))?;
    for row in rows {
        if !row.is_object() {
            return Err(Error::InternalError(
                "write_jsonl() requires object rows".to_string(),
            ));
        }
        let json = row.clone().into_json();
        let line = serde_json::to_string(&json).map_err(|e| read_err(path, e))?;
        writeln!(file, "{line}").map_err(|e| read_err(path, e))?;
    }
    Ok(())
}

fn cell_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.to_string(),
        other => other.to_string(),
    }
}

fn read_err(path: &str, e: impl std::fmt::Display) -> Error {
    Error::IoError(format!("{path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use tempfile::NamedTempFile;

    #[test]
    fn infer_format_extensions() {
        assert_eq!(infer_format("a.csv"), Some(FileFormat::Csv));
        assert_eq!(infer_format("a.tsv"), Some(FileFormat::Tsv));
        assert_eq!(infer_format("a.jsonl"), Some(FileFormat::Jsonl));
        assert_eq!(infer_format("a.ndjson"), Some(FileFormat::Jsonl));
        assert_eq!(infer_format("hello"), None);
    }

    #[test]
    fn read_csv_with_header() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "a,b\n1,2\n3,4").unwrap();
        let path = f.path().to_str().unwrap();
        let rows = read_rows(path, FileFormat::Csv, ReadOptions::default()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].jpk("a"), Some(Value::Int(1)));
        assert_eq!(rows[0].jpk("b"), Some(Value::Int(2)));
    }

    #[test]
    fn read_csv_no_header() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "1,2\n3,4").unwrap();
        let path = f.path().to_str().unwrap();
        let mut opts = ReadOptions::default();
        opts.header = false;
        let rows = read_rows(path, FileFormat::Csv, opts).unwrap();
        assert_eq!(rows[0].jpk("column0"), Some(Value::Int(1)));
    }

    #[test]
    fn round_trip_jsonl() {
        let mut f = NamedTempFile::new().unwrap();
        let src = f.path().to_str().unwrap();
        write!(f, "{{\"x\":1}}\n{{\"x\":2}}").unwrap();
        let rows = read_rows(src, FileFormat::Jsonl, ReadOptions::default()).unwrap();
        let mut out = NamedTempFile::new().unwrap();
        let dst = out.path().to_str().unwrap();
        write_rows(dst, FileFormat::Jsonl, WriteOptions::default(), &rows).unwrap();
        let again = read_rows(dst, FileFormat::Jsonl, ReadOptions::default()).unwrap();
        assert_eq!(rows.len(), again.len());
        assert_eq!(rows[0].jpk("x"), again[0].jpk("x"));
    }
}
