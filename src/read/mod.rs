//! CSV, TSV, JSON, and JSONL file sources for the SQL file-I/O surface.
//!
//! Two ways in: [`open_rows`] hands back a pull-based [`RowReader`] that
//! `Vop::ScanFile` drives one row at a time, and [`read_rows`] materializes the
//! whole file for the `read_*` builtins. Both yield row objects as [`Value`];
//! [`write_rows`] consumes the same shape. Files are orthogonal to storage —
//! nothing here touches a transaction.

mod options;
mod rows;

use std::fs::File;
use std::io::Write;
use std::path::Path;

use csv::WriterBuilder;

pub use options::{ReadOptions, WriteOptions};
pub use rows::{RowReader, open_rows};

use crate::error::{Error, Result};
use crate::value::Value;

/// Supported on-disk tabular formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Comma-separated values.
    Csv,
    /// Tab-separated values.
    Tsv,
    /// Newline-delimited JSON objects.
    Jsonl,
    /// A single JSON document: an array of rows, or one object.
    Json,
}

/// Everything needed to open a file reader, resolved at compile time.
///
/// The binder lowers a file-path FROM source to one of these; the compiler
/// interns it in [`Program::files`](crate::vm::Program) and `Vop::ScanFile`
/// addresses it by index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSource {
    /// The path, as written in the query (resolved against the process cwd).
    pub path: String,
    /// The format, inferred from the path's extension.
    pub format: FileFormat,
    /// Reader options, const-folded from the query.
    pub options: ReadOptions,
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
        "json" => Some(FileFormat::Json),
        _ => None,
    }
}

/// Returns true when `path` has a recognized tabular file extension.
pub fn looks_like_file(path: &str) -> bool {
    infer_format(path).is_some()
}

/// Reads all rows from `path` using `format` and `opts`.
///
/// Drains [`open_rows`], so the eager builtins and the streaming cursor share
/// one parser.
pub fn read_rows(path: &str, format: FileFormat, opts: ReadOptions) -> Result<Vec<Value>> {
    let mut reader = open_rows(path, format, &opts)?;
    let mut rows = Vec::new();
    while let Some(row) = reader.next_row()? {
        rows.push(row);
    }
    Ok(rows)
}

/// Writes `rows` to `path` using `format` and `opts`.
pub fn write_rows(path: &str, format: FileFormat, opts: WriteOptions, rows: &[Value]) -> Result<()> {
    match format {
        FileFormat::Csv => write_csv(path, &opts, rows),
        FileFormat::Tsv => write_csv(path, &opts.for_tsv(), rows),
        FileFormat::Jsonl => write_jsonl(path, rows),
        FileFormat::Json => write_json(path, rows),
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
        FileFormat::Json => "read_json",
    }
}

/// Builtin name for writing `format`.
pub fn write_builtin(format: FileFormat) -> &'static str {
    match format {
        FileFormat::Csv | FileFormat::Tsv => "write_csv",
        FileFormat::Jsonl => "write_jsonl",
        FileFormat::Json => "write_json",
    }
}

fn write_csv(path: &str, opts: &WriteOptions, rows: &[Value]) -> Result<()> {
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

/// Writes `rows` as a single top-level JSON array — the inverse of [`rows::JsonRows`].
fn write_json(path: &str, rows: &[Value]) -> Result<()> {
    let items: Vec<serde_json::Value> = rows.iter().map(|r| r.clone().into_json()).collect();
    let text = serde_json::to_string(&items).map_err(|e| read_err(path, e))?;
    let mut file = File::create(path).map_err(|e| read_err(path, e))?;
    writeln!(file, "{text}").map_err(|e| read_err(path, e))?;
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

    /// `skip` counts data records *after* the header for CSV — the enumerate in
    /// `read_csv` runs over `records()`, which has already consumed the header.
    #[test]
    fn csv_skip_counts_data_records_after_header() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "a,b\n1,2\n3,4\n").unwrap();
        let path = f.path().to_str().unwrap();
        let mut opts = ReadOptions::default();
        opts.skip = 1;
        let rows = read_rows(path, FileFormat::Csv, opts).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].jpk("a"), Some(Value::Int(3)));
    }

    /// `skip` counts *raw lines* for JSONL, blanks included — the skip check
    /// precedes the blank-line check. Pinned because the two formats differ.
    #[test]
    fn jsonl_skip_counts_blank_lines() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "\n{{\"x\":1}}\n{{\"x\":2}}\n").unwrap();
        let path = f.path().to_str().unwrap();
        let mut opts = ReadOptions::default();
        opts.skip = 1;
        let rows = read_rows(path, FileFormat::Jsonl, opts).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].jpk("x"), Some(Value::Int(1)));
    }

    /// Duplicate CSV headers are legal input and collapse last-wins, because
    /// rows are built through `Object::insert`. This is why the streaming
    /// reader may only take the `Object::from_members` fast path when the
    /// header names are pairwise distinct.
    #[test]
    fn csv_duplicate_headers_last_wins() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "a,b,a\n1,2,3\n").unwrap();
        let path = f.path().to_str().unwrap();
        let rows = read_rows(path, FileFormat::Csv, ReadOptions::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].members().unwrap().len(), 2);
        assert_eq!(rows[0].jpk("a"), Some(Value::Int(3)));
        assert_eq!(rows[0].jpk("b"), Some(Value::Int(2)));
    }

    /// The reader is `flexible`, so a row wider than the header is truncated by
    /// the header/field zip rather than erroring.
    #[test]
    fn csv_flexible_wider_row_truncates_to_header() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "a,b\n1,2,3\n").unwrap();
        let path = f.path().to_str().unwrap();
        let rows = read_rows(path, FileFormat::Csv, ReadOptions::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].members().unwrap().len(), 2);
        assert_eq!(rows[0].jpk("b"), Some(Value::Int(2)));
    }

    /// A row narrower than the header contributes only the fields present.
    #[test]
    fn csv_flexible_narrow_row_drops_missing_fields() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "a,b\n1\n").unwrap();
        let path = f.path().to_str().unwrap();
        let rows = read_rows(path, FileFormat::Csv, ReadOptions::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].members().unwrap().len(), 1);
        assert_eq!(rows[0].jpk("a"), Some(Value::Int(1)));
    }

    /// The eager `read_rows` and a manual `open_rows` drain must agree — this is
    /// what keeps the streaming cursor and the `read_*` builtins from drifting.
    #[test]
    fn read_rows_matches_open_rows_drain() {
        let cases: [(FileFormat, &str); 4] = [
            (FileFormat::Csv, "a,b\n1,2\n3,4\n"),
            (FileFormat::Tsv, "a\tb\n1\t2\n3\t4\n"),
            (FileFormat::Jsonl, "{\"x\":1}\n{\"x\":2}\n"),
            (FileFormat::Json, "[{\"x\":1},{\"x\":2}]"),
        ];
        for (format, text) in cases {
            let mut f = NamedTempFile::new().unwrap();
            write!(f, "{text}").unwrap();
            let path = f.path().to_str().unwrap();

            let eager = read_rows(path, format, ReadOptions::default()).unwrap();
            let mut reader = open_rows(path, format, &ReadOptions::default()).unwrap();
            let mut drained = Vec::new();
            while let Some(row) = reader.next_row().unwrap() {
                drained.push(row);
            }

            assert_eq!(eager.len(), drained.len(), "{format:?}");
            for (a, b) in eager.iter().zip(drained.iter()) {
                assert_eq!(a.clone().into_json(), b.clone().into_json(), "{format:?}");
            }
        }
    }

    /// A reader yields rows lazily: the malformed row must not be reached when
    /// the consumer stops early. This is the unit-level streaming proof.
    #[test]
    fn reader_stops_before_malformed_row() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{{\"x\":1}}\n{{\"x\":2}}\n{{\"x\":3\n").unwrap();
        let path = f.path().to_str().unwrap();

        let mut reader = open_rows(path, FileFormat::Jsonl, &ReadOptions::default()).unwrap();
        assert_eq!(reader.next_row().unwrap().unwrap().jpk("x"), Some(Value::Int(1)));
        assert_eq!(reader.next_row().unwrap().unwrap().jpk("x"), Some(Value::Int(2)));
        // Only now is the malformed line touched.
        assert!(reader.next_row().is_err());

        // The eager read of the same file fails outright.
        assert!(read_rows(path, FileFormat::Jsonl, ReadOptions::default()).is_err());
    }

    #[test]
    fn json_top_level_array_object_and_error() {
        let cases: [(&str, Option<usize>); 5] = [
            ("[{\"x\":1},{\"x\":2}]", Some(2)),
            ("{\"x\":1}", Some(1)),
            ("[]", Some(0)),
            // Rows need not be objects — this matches `from [1, 2] as x`.
            ("[1,2,3]", Some(3)),
            ("42", None),
        ];
        for (text, expected) in cases {
            let mut f = NamedTempFile::new().unwrap();
            write!(f, "{text}").unwrap();
            let path = f.path().to_str().unwrap();
            let rows = read_rows(path, FileFormat::Json, ReadOptions::default());
            match expected {
                Some(n) => assert_eq!(rows.unwrap().len(), n, "{text}"),
                None => assert!(rows.is_err(), "{text}"),
            }
        }
    }

    #[test]
    fn round_trip_json() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "[{{\"x\":1}},{{\"x\":2}}]").unwrap();
        let src = f.path().to_str().unwrap().to_string();
        let rows = read_rows(&src, FileFormat::Json, ReadOptions::default()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let dst = out.path().to_str().unwrap().to_string();
        write_rows(&dst, FileFormat::Json, WriteOptions::default(), &rows).unwrap();
        let again = read_rows(&dst, FileFormat::Json, ReadOptions::default()).unwrap();
        assert_eq!(rows.len(), again.len());
        assert_eq!(rows[1].jpk("x"), again[1].jpk("x"));
    }

    #[test]
    fn round_trip_jsonl() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{{\"x\":1}}\n{{\"x\":2}}").unwrap();
        let src = f.path().to_str().unwrap().to_string();
        let rows = read_rows(&src, FileFormat::Jsonl, ReadOptions::default()).unwrap();
        let out = NamedTempFile::new().unwrap();
        let dst = out.path().to_str().unwrap().to_string();
        write_rows(&dst, FileFormat::Jsonl, WriteOptions::default(), &rows).unwrap();
        let again = read_rows(&dst, FileFormat::Jsonl, ReadOptions::default()).unwrap();
        assert_eq!(rows.len(), again.len());
        assert_eq!(rows[0].jpk("x"), again[0].jpk("x"));
    }
}
