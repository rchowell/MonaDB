//! Read and write transactions on the storage engine.
//!
//! Both wrap a heed transaction. `WriteTxn` additionally maintains a staged
//! `BTreeMap<Vec<u8>, Vec<u8>>` of pending writes, flushed to `data` on `commit`.
//! Read-your-own-writes inside a single `WriteTxn` is **not** supported in v1 —
//! cursors opened on a `WriteTxn` see only the committed state. Add a merge cursor
//! when a real workload demands it (storage-reference §9).

use std::collections::BTreeMap;

use heed::{RoTxn, RwTxn, WithTls};

use crate::error::Error;
use crate::value::Value;
use crate::Result;

use crate::catalog::{self, ColumnSchema, TableSchema};
use crate::cursor::Cursor;
use crate::storage::StorageInner;
use crate::storage::keycodec;
use crate::storage::value_codec;

/// Length of the trailing zero suffix appended to every data key. Reserved for
/// the future `commit_seq` stamp (Appendix A).
const ZERO_SUFFIX: [u8; keycodec::SUFFIX_LEN] = [0u8; keycodec::SUFFIX_LEN];

/// Maximum data-key length LMDB will accept by default. Surfaced as
/// `Error::Storage(...)` if exceeded.
const MAX_KEY_LEN: usize = 511;

pub struct ReadTxn<'env> {
    /// The storage engine inner.
    env: &'env StorageInner,
    /// The heed read transaction handle.
    txn: RoTxn<'env, WithTls>,
}

impl<'env> ReadTxn<'env> {
    pub(super) fn begin(env: &'env StorageInner) -> Result<Self> {
        let rotxn = env.heed.read_txn()?;
        Ok(Self { env, txn: rotxn })
    }

    pub fn get_table(&self, name: &str) -> Result<TableSchema> {
        catalog::get_table(self.env, &self.txn, name)
    }

    pub fn open_cursor(&self, table: &str) -> Result<Cursor<'_>> {
        let schema = catalog::get_table(self.env, &self.txn, table)?;
        Cursor::open(self.env, &self.txn, schema.table_id)
    }
}

pub struct WriteTxn<'env> {
    env: &'env StorageInner,
    rwtxn: RwTxn<'env>,
    /// Staged writes, keyed by the *partial* data key (no trailing 8-byte suffix).
    /// On `commit`, each entry is flushed by appending `ZERO_SUFFIX` to the key.
    staged: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl<'env> WriteTxn<'env> {
    pub(super) fn begin(env: &'env StorageInner) -> Result<Self> {
        let rwtxn = env.heed.write_txn()?;
        Ok(Self {
            env,
            rwtxn,
            staged: BTreeMap::new(),
        })
    }

    pub fn get_table(&self, name: &str) -> Result<TableSchema> {
        catalog::get_table(self.env, &self.rwtxn, name)
    }

    pub fn create_table(
        &mut self,
        name: &str,
        columns: Vec<ColumnSchema>,
    ) -> Result<TableSchema> {
        catalog::create_table(self.env, &mut self.rwtxn, name, columns)
    }

    /// Insert a row. Phase 1: surrogate-keyed only — the caller passes the row's
    /// JSON value and the storage layer assigns a fresh `u64` row id.
    pub fn put_row(&mut self, table: &str, value: Value) -> Result<()> {
        let schema = catalog::get_table(self.env, &self.rwtxn, table)?;
        let row_id = catalog::next_row_id(self.env, &mut self.rwtxn, schema.table_id)?;
        let partial_key = keycodec::surrogate_partial(schema.table_id, row_id);
        let body = value_codec::encode(&value);
        let total = partial_key.len() + keycodec::SUFFIX_LEN;
        if total > MAX_KEY_LEN {
            return Err(Error::Storage(format!(
                "key too long: {total} > {MAX_KEY_LEN}"
            )));
        }
        self.staged.insert(partial_key, body);
        Ok(())
    }

    /// Flush staged writes and commit the underlying heed transaction. Atomic.
    pub fn commit(mut self) -> Result<()> {
        let staged: Vec<(Vec<u8>, Vec<u8>)> =
            std::mem::take(&mut self.staged).into_iter().collect();
        for (partial, body) in staged {
            let mut full_key = Vec::with_capacity(partial.len() + ZERO_SUFFIX.len());
            full_key.extend_from_slice(&partial);
            full_key.extend_from_slice(&ZERO_SUFFIX);
            self.env.data.put(&mut self.rwtxn, &full_key, &body)?;
        }
        self.rwtxn.commit()?;
        Ok(())
    }

    /// Discard staged writes and abandon the heed transaction.
    pub fn abort(self) {
        drop(self);
    }

    /// Open a cursor that reads through the *committed* state (staged writes are invisible).
    pub fn open_cursor(&self, table: &str) -> Result<Cursor<'_>> {
        let schema = catalog::get_table(self.env, &self.rwtxn, table)?;
        Cursor::open(self.env, &self.rwtxn, schema.table_id)
    }
}
