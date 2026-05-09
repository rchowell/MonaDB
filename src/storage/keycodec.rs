//! Key encoding for the `data` DB.
//!
//! Phase 1: surrogate `u64` row ids only. The data key is
//!
//! ```text
//! [ u32_be(table_id) ]   4 bytes
//! [ u64_be(row_id)   ]   8 bytes  (the single PK component for surrogate-keyed tables)
//! [ reserved suffix  ]   8 bytes  always zero in P1/P2; reserved for future versioning
//! ```
//!
//! `partial_key` returns the first 12 bytes (table_id + pk component). The 8-byte zero
//! suffix is appended at flush time by `WriteTxn::commit`. Splitting it this way leaves
//! room for the future `commit_seq` stamp to land additively without disturbing the
//! staged-write path.

use byteorder::{BigEndian, ByteOrder};

/// Per-row trailing byte count reserved for the future versioning system.
pub const SUFFIX_LEN: usize = 8;

/// Build the table prefix `u32_be(table_id)` — the leading 4 bytes of every key in `table_id`.
#[inline]
pub fn table_prefix(table_id: u32) -> [u8; 4] {
    let mut out = [0u8; 4];
    BigEndian::write_u32(&mut out, table_id);
    out
}

/// The lex-successor of `table_prefix(table_id)`, used as the exclusive upper bound for
/// a table-wide range scan. Returns `[0,0,0,0]` if `table_id == u32::MAX` (the scan would
/// run to the end of the DB anyway).
#[inline]
pub fn table_prefix_upper(table_id: u32) -> [u8; 4] {
    let mut out = [0u8; 4];
    BigEndian::write_u32(&mut out, table_id.wrapping_add(1));
    out
}

/// Surrogate composite-key body: `u32_be(table_id) || u64_be(row_id)`. 12 bytes.
///
/// This is the *partial* key — `commit` appends the 8-byte zero suffix.
pub fn surrogate_partial(table_id: u32, row_id: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(&table_prefix(table_id));
    let mut buf = [0u8; 8];
    BigEndian::write_u64(&mut buf, row_id);
    out.extend_from_slice(&buf);
    out
}

/// Decode the surrogate row id from a full data key.
///
/// Caller is responsible for confirming the key starts with the expected `table_prefix`
/// and is the right length for a surrogate-keyed table. Returns `None` if the key is
/// shorter than `4 + 8 + SUFFIX_LEN`.
pub fn surrogate_row_id(full_key: &[u8]) -> Option<u64> {
    if full_key.len() < 4 + 8 + SUFFIX_LEN {
        return None;
    }
    Some(BigEndian::read_u64(&full_key[4..4 + 8]))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn table_prefix_is_be() {
        assert_eq!(table_prefix(1), [0, 0, 0, 1]);
        assert_eq!(table_prefix(0x0102_0304), [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn surrogate_round_trip() {
        let k = surrogate_partial(7, 42);
        assert_eq!(k.len(), 12);
        assert_eq!(&k[..4], &[0, 0, 0, 7]);
        // Append the zero suffix to make it a "full" key, then decode.
        let mut full = k.clone();
        full.extend_from_slice(&[0u8; SUFFIX_LEN]);
        assert_eq!(surrogate_row_id(&full), Some(42));
    }

    #[test]
    fn surrogate_keys_sort_by_table_then_row() {
        let a = surrogate_partial(1, u64::MAX);
        let b = surrogate_partial(2, 0);
        assert!(a < b, "table_id must dominate row_id in lex order");

        let c = surrogate_partial(7, 1);
        let d = surrogate_partial(7, 2);
        assert!(c < d, "within a table, row_ids must sort big-endian");
    }
}
