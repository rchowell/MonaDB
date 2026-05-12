//! Catalog: table-id allocation, schema records, and per-table counters.
//!
//! Lives entirely in the `meta` named DB. Layout:
//!
//! ```text
//!   b"schema/<table_name>"      → encoded TableSchema (see encode/decode below)
//!   b"table_id/<u32_be>"        → table_name (reverse map; useful for diagnostics)
//!   b"next_table_id"            → u32 LE
//!   b"row_seq/<u32_be table>"   → u64 LE  (surrogate row id counter, per-table)
//! ```

use byteorder::{BigEndian, ByteOrder, LittleEndian};
use heed::{RoTxn, RwTxn};

use crate::error::Error;
use crate::Result;

use crate::storage::Env;

/// Storage-layer column type. Parallel to `ir::ColumnType`; the connection layer
/// translates between them. Keeping the storage module ir-free lets it be reused
/// or refactored without dragging the parser along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Int,
    String,
}

impl ColumnType {
    fn tag(self) -> u8 {
        match self {
            ColumnType::Int => 0,
            ColumnType::String => 1,
        }
    }

    fn from_tag(tag: u8) -> Result<Self> {
        match tag {
            0 => Ok(ColumnType::Int),
            1 => Ok(ColumnType::String),
            other => Err(Error::InternalError(format!(
                "storage: unknown column type tag 0x{other:02x}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSchema {
    pub name: String,
    pub typ: ColumnType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSchema {
    pub name: String,
    pub table_id: u32,
    pub columns: Vec<ColumnSchema>,
}

// ---------- meta-DB key construction ----------

fn key_schema(name: &str) -> Vec<u8> {
    let mut k = b"schema/".to_vec();
    k.extend_from_slice(name.as_bytes());
    k
}

fn key_row_seq(table_id: u32) -> [u8; 8 + 4] {
    let mut k = [0u8; 12];
    k[..8].copy_from_slice(b"row_seq/");
    BigEndian::write_u32(&mut k[8..], table_id);
    k
}

const KEY_NEXT_TABLE_ID: &[u8] = b"next_table_id";

// ---------- schema serialization ----------
//
// Hand-rolled binary layout — `serde` is dev-only in this crate. Schemas are tiny and
// stable; this avoids pulling derives across `ir.rs`.
//
// ```text
//   [ u32_le(table_id) ]
//   [ u16_be(name.len()) ][ name bytes ]
//   [ u16_be(columns.len()) ]
//   for each column:
//     [ u8 typ tag ]
//     [ u16_be(name.len()) ][ name bytes ]
// ```

fn encode_schema(s: &TableSchema) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    let mut buf4 = [0u8; 4];
    LittleEndian::write_u32(&mut buf4, s.table_id);
    out.extend_from_slice(&buf4);
    write_str(&mut out, &s.name);
    let mut buf2 = [0u8; 2];
    BigEndian::write_u16(&mut buf2, s.columns.len() as u16);
    out.extend_from_slice(&buf2);
    for c in &s.columns {
        out.push(c.typ.tag());
        write_str(&mut out, &c.name);
    }
    out
}

fn decode_schema(bytes: &[u8]) -> Result<TableSchema> {
    let mut p = 0;
    let table_id = read_u32_le(bytes, &mut p)?;
    let name = read_str(bytes, &mut p)?;
    let n = read_u16_be(bytes, &mut p)? as usize;
    let mut columns = Vec::with_capacity(n);
    for _ in 0..n {
        let tag = read_u8(bytes, &mut p)?;
        let typ = ColumnType::from_tag(tag)?;
        let name = read_str(bytes, &mut p)?;
        columns.push(ColumnSchema { name, typ });
    }
    if p != bytes.len() {
        return Err(Error::InternalError(format!(
            "storage: catalog record has {} trailing bytes",
            bytes.len() - p
        )));
    }
    Ok(TableSchema {
        name,
        table_id,
        columns,
    })
}

fn write_str(out: &mut Vec<u8>, s: &str) {
    let mut buf = [0u8; 2];
    BigEndian::write_u16(&mut buf, s.len() as u16);
    out.extend_from_slice(&buf);
    out.extend_from_slice(s.as_bytes());
}

fn read_u8(bytes: &[u8], p: &mut usize) -> Result<u8> {
    if *p >= bytes.len() {
        return Err(catalog_short());
    }
    let v = bytes[*p];
    *p += 1;
    Ok(v)
}

fn read_u16_be(bytes: &[u8], p: &mut usize) -> Result<u16> {
    if *p + 2 > bytes.len() {
        return Err(catalog_short());
    }
    let v = BigEndian::read_u16(&bytes[*p..*p + 2]);
    *p += 2;
    Ok(v)
}

fn read_u32_le(bytes: &[u8], p: &mut usize) -> Result<u32> {
    if *p + 4 > bytes.len() {
        return Err(catalog_short());
    }
    let v = LittleEndian::read_u32(&bytes[*p..*p + 4]);
    *p += 4;
    Ok(v)
}

fn read_str(bytes: &[u8], p: &mut usize) -> Result<String> {
    let n = read_u16_be(bytes, p)? as usize;
    if *p + n > bytes.len() {
        return Err(catalog_short());
    }
    let s = std::str::from_utf8(&bytes[*p..*p + n])
        .map_err(|e| Error::InternalError(format!("storage: catalog utf-8: {e}")))?
        .to_owned();
    *p += n;
    Ok(s)
}

fn catalog_short() -> Error {
    Error::InternalError("storage: catalog record truncated".to_string())
}

// ---------- public CRUD ----------

/// Look up a table by name. Errors with `UnknownTable` if absent.
pub(crate) fn get_table(env: &Env, txn: &RoTxn, name: &str) -> Result<TableSchema> {
    let key = key_schema(name);
    match env.meta.get(txn, &key)? {
        Some(bytes) => decode_schema(bytes),
        None => Err(Error::UnknownTable(name.to_string())),
    }
}

/// Whether a table exists. Useful for "create-if-not-exists" semantics.
pub(crate) fn table_exists(env: &Env, txn: &RoTxn, name: &str) -> Result<bool> {
    Ok(env.meta.get(txn, &key_schema(name))?.is_some())
}

/// Create a new table. Allocates a fresh `table_id`. Errors if the table already
/// exists; the caller decides whether to swallow that.
pub(crate) fn create_table(
    env: &Env,
    txn: &mut RwTxn,
    name: &str,
    columns: Vec<ColumnSchema>,
) -> Result<TableSchema> {
    if env.meta.get(txn, &key_schema(name))?.is_some() {
        return Err(Error::InternalError(format!(
            "storage: table {name:?} already exists"
        )));
    }
    let table_id = next_table_id(env, txn)?;
    let schema = TableSchema {
        name: name.to_string(),
        table_id,
        columns,
    };
    let body = encode_schema(&schema);
    env.meta.put(txn, &key_schema(name), &body)?;
    // reverse map for diagnostics
    let mut tid_key = [0u8; 9];
    tid_key[..9].copy_from_slice(b"table_id/");
    let mut tid_be = [0u8; 4];
    BigEndian::write_u32(&mut tid_be, table_id);
    let mut full = Vec::with_capacity(9 + 4);
    full.extend_from_slice(&tid_key);
    full.extend_from_slice(&tid_be);
    env.meta.put(txn, &full, name.as_bytes())?;
    Ok(schema)
}

/// Allocate the next `u64` surrogate row id for `table_id`. Mutates the meta DB.
pub(crate) fn next_row_id(env: &Env, txn: &mut RwTxn, table_id: u32) -> Result<u64> {
    let key = key_row_seq(table_id);
    let curr = match env.meta.get(txn, &key)? {
        Some(bytes) => {
            let bytes: &[u8] = bytes;
            if bytes.len() == 8 {
                LittleEndian::read_u64(bytes)
            } else {
                return Err(Error::InternalError("storage: row_seq corrupt".to_string()));
            }
        }
        None => 0,
    };
    let next = curr + 1;
    let mut buf = [0u8; 8];
    LittleEndian::write_u64(&mut buf, next);
    env.meta.put(txn, &key, &buf)?;
    Ok(next)
}

fn next_table_id(env: &Env, txn: &mut RwTxn) -> Result<u32> {
    let curr = match env.meta.get(txn, KEY_NEXT_TABLE_ID)? {
        Some(bytes) => {
            let bytes: &[u8] = bytes;
            if bytes.len() == 4 {
                LittleEndian::read_u32(bytes)
            } else {
                return Err(Error::InternalError(
                    "storage: next_table_id corrupt".to_string(),
                ));
            }
        }
        None => 0,
    };
    let allocated = curr + 1;
    let mut buf = [0u8; 4];
    LittleEndian::write_u32(&mut buf, allocated);
    env.meta.put(txn, KEY_NEXT_TABLE_ID, &buf)?;
    Ok(allocated)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn schema_round_trip() {
        let s = TableSchema {
            name: "events".to_string(),
            table_id: 7,
            columns: vec![
                ColumnSchema {
                    name: "tenant".to_string(),
                    typ: ColumnType::String,
                },
                ColumnSchema {
                    name: "user".to_string(),
                    typ: ColumnType::String,
                },
                ColumnSchema {
                    name: "ts".to_string(),
                    typ: ColumnType::Int,
                },
            ],
        };
        let bytes = encode_schema(&s);
        let back = decode_schema(&bytes).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn schema_no_columns() {
        let s = TableSchema {
            name: "x".to_string(),
            table_id: 1,
            columns: vec![],
        };
        let bytes = encode_schema(&s);
        assert_eq!(decode_schema(&bytes).unwrap(), s);
    }
}
