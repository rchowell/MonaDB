//! Storage cursor — a forward iterator over the rows of one table in `data`.
//!
//! Holds no per-scan upper-bound state; range-bound termination lives in the future
//! `Idx*` opcode family (Phase 2). The cursor's only job here is:
//!   1. Position at `[u32_be(table_id) ..]`.
//!   2. Walk forward, stopping when the table prefix changes.
//!   3. Strip the leading tag byte from each value; decode the body via `value_codec`.

use std::ops::Bound;

use heed::types::Bytes;
use heed::{Database, RoRange, RoTxn};

use crate::Result;

use crate::storage::StorageInner;
use crate::storage::keycodec;
use crate::storage::value_codec;
use crate::storage::Row;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorState {
    BeforeStart,
    Positioned,
    Exhausted,
}

/// Forward cursor over one table's rows. Created via `ReadTxn::open_cursor`.
pub struct Cursor<'a> {
    db: &'a Database<Bytes, Bytes>,
    txn: &'a RoTxn<'a>,
    table_id: u32,
    range_lo: [u8; 4],
    range_hi: [u8; 4],
    iter: Option<RoRange<'a, Bytes, Bytes>>,
    curr: Option<Row>,
    state: CursorState,
}

impl<'a> Cursor<'a> {
    pub(super) fn open(env: &'a StorageInner, txn: &'a RoTxn<'a>, table_id: u32) -> Result<Self> {
        Ok(Self {
            db: &env.data,
            txn,
            table_id,
            range_lo: keycodec::table_prefix(table_id),
            range_hi: keycodec::table_prefix_upper(table_id),
            iter: None,
            curr: None,
            state: CursorState::BeforeStart,
        })
    }

    /// `table_id` this cursor was opened against — exposed for diagnostics & tests.
    pub fn table_id(&self) -> u32 {
        self.table_id
    }

    /// Reset to the start of the table. Returns `true` if the table has at least one row.
    pub fn rewind(&mut self) -> Result<bool> {
        // Drop any outstanding iter borrow before re-borrowing the txn.
        self.iter = None;
        self.curr = None;
        self.state = CursorState::BeforeStart;
        let lo: &[u8] = &self.range_lo;
        let hi: &[u8] = &self.range_hi;
        let range = (Bound::Included(lo), Bound::Excluded(hi));
        let iter = self.db.range(self.txn, &range)?;
        self.iter = Some(iter);
        self.advance()
    }

    /// Step to the next row. Returns `true` if newly positioned, `false` at end.
    pub fn next(&mut self) -> Result<bool> {
        if matches!(self.state, CursorState::BeforeStart) {
            // VM contract: callers should rewind before next; treat as "not positioned".
            self.curr = None;
            return Ok(false);
        }
        self.advance()
    }

    /// The row the cursor is currently positioned on. Panics if not positioned —
    /// callers must check `rewind()` / `next()` returned `true` first.
    pub fn curr(&self) -> &Row {
        self.curr
            .as_ref()
            .expect("StorageCursor::curr called when not positioned")
    }

    fn advance(&mut self) -> Result<bool> {
        let Some(iter) = self.iter.as_mut() else {
            self.state = CursorState::Exhausted;
            self.curr = None;
            return Ok(false);
        };
        match iter.next() {
            Some(Ok((_key, value))) => {
                let decoded = value_codec::decode(value)?;
                // Surrogate row id sits in bytes 4..12 of the full key.
                let oid = keycodec::surrogate_row_id(_key).unwrap_or(0);
                self.curr = Some(Row { oid, val: decoded });
                self.state = CursorState::Positioned;
                Ok(true)
            }
            Some(Err(e)) => Err(e.into()),
            None => {
                self.state = CursorState::Exhausted;
                self.curr = None;
                Ok(false)
            }
        }
    }
}
