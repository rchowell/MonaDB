//! File read/write option bags parsed from `Value::Object`.

use std::rc::Rc;

use crate::error::{Error, Result};
use crate::value::Value;

/// Options controlling CSV/TSV/JSONL reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOptions {
    /// Whether the first CSV/TSV row names columns. Default: `true`.
    pub header: bool,
    /// Field delimiter for CSV/TSV. Default: `,` (TSV sets `'\t'`).
    pub delimiter: char,
    /// Quote character for CSV/TSV. Default: `"`.
    pub quote: char,
    /// Column names when `header` is false. Empty → `column0`, `column1`, …
    pub columns: Vec<String>,
    /// Leading lines to skip before data (blank JSONL lines count).
    pub skip: usize,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            header: true,
            delimiter: ',',
            quote: '"',
            columns: Vec::new(),
            skip: 0,
        }
    }
}

/// Options controlling CSV/TSV/JSONL writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOptions {
    /// Whether to emit a header row for CSV/TSV. Default: `true`.
    pub header: bool,
    /// Field delimiter for CSV/TSV. Default: `,`.
    pub delimiter: char,
    /// Quote character for CSV/TSV. Default: `"`.
    pub quote: char,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            header: true,
            delimiter: ',',
            quote: '"',
        }
    }
}

impl ReadOptions {
    /// Parses read options from a `Value::Object`, or returns defaults for
    /// `null` / an empty object.
    pub fn from_value(v: &Value) -> Result<Self> {
        let mut opts = Self::default();
        let Value::Object(obj) = v else {
            if v.is_null() {
                return Ok(opts);
            }
            return Err(Error::InternalError(
                "read options must be an object".to_string(),
            ));
        };
        for (key, val) in obj.iter() {
            match key {
                "header" => opts.header = want_bool("header", val)?,
                "delimiter" => opts.delimiter = want_char("delimiter", val)?,
                "quote" => opts.quote = want_char("quote", val)?,
                "columns" => opts.columns = want_string_array("columns", val)?,
                "skip" => opts.skip = want_usize("skip", val)?,
                other => {
                    return Err(Error::Unsupported(format!(
                        "unknown read option '{other}'"
                    )));
                }
            }
        }
        Ok(opts)
    }

    /// Applies TSV defaults when the format is tab-separated.
    pub fn for_tsv(mut self) -> Self {
        self.delimiter = '\t';
        self
    }
}

impl WriteOptions {
    /// Parses write options from a `Value::Object`, or returns defaults for
    /// `null` / an empty object.
    pub fn from_value(v: &Value) -> Result<Self> {
        let mut opts = Self::default();
        let Value::Object(obj) = v else {
            if v.is_null() {
                return Ok(opts);
            }
            return Err(Error::InternalError(
                "write options must be an object".to_string(),
            ));
        };
        for (key, val) in obj.iter() {
            match key {
                "header" => opts.header = want_bool("header", val)?,
                "delimiter" => opts.delimiter = want_char("delimiter", val)?,
                "quote" => opts.quote = want_char("quote", val)?,
                other => {
                    return Err(Error::Unsupported(format!(
                        "unknown write option '{other}'"
                    )));
                }
            }
        }
        Ok(opts)
    }

    /// Applies TSV defaults when the format is tab-separated.
    pub fn for_tsv(mut self) -> Self {
        self.delimiter = '\t';
        self
    }
}

fn want_bool(name: &str, v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(Error::InternalError(format!(
            "option '{name}' requires a boolean"
        ))),
    }
}

fn want_char(name: &str, v: &Value) -> Result<char> {
    let s = v.as_str().ok_or_else(|| {
        Error::InternalError(format!("option '{name}' requires a string"))
    })?;
    let mut chars = s.chars();
    let c = chars.next().ok_or_else(|| {
        Error::InternalError(format!("option '{name}' requires a non-empty string"))
    })?;
    if chars.next().is_some() {
        return Err(Error::InternalError(format!(
            "option '{name}' requires a single character"
        )));
    }
    Ok(c)
}

fn want_usize(name: &str, v: &Value) -> Result<usize> {
    match v {
        Value::Int(i) if *i >= 0 => Ok(*i as usize),
        Value::Float(f) if *f >= 0.0 && f.fract() == 0.0 => Ok(*f as usize),
        _ => Err(Error::InternalError(format!(
            "option '{name}' requires a non-negative integer"
        ))),
    }
}

fn want_string_array(name: &str, v: &Value) -> Result<Vec<String>> {
    let Value::Array(items) = v else {
        return Err(Error::InternalError(format!(
            "option '{name}' requires an array of strings"
        )));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| {
                    Error::InternalError(format!(
                        "option '{name}' requires an array of strings"
                    ))
                })
        })
        .collect()
}

/// Coerces a cell string to a `Value`, preferring int then float when parseable.
pub(crate) fn cell_value(raw: &str) -> Value {
    if let Ok(i) = raw.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = raw.parse::<f64>() {
        if f.is_finite() {
            return Value::Float(f);
        }
    }
    Value::String(Rc::from(raw))
}
