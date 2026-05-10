use heed::{RoTxn, RwTxn, WithTls};

use crate::Result;

use crate::cursor::Cursor;
use crate::storage::Env;

/// A read-only transaction handle,
pub struct ReadTxn<'env> {
    /// The heed environment handle.
    env: &'env Env,
    /// The heed read transaction handle.
    txn: RoTxn<'env, WithTls>,
}

impl<'env> ReadTxn<'env> {
    /// Opens a new read transaction handle, released on drop.
    pub fn open(env: &'env Env) -> Result<Self> {
        let txn = env.heed.read_txn()?;
        Ok(Self { env, txn })
    }

    /// Opens a cursor over the given table.
    pub fn cursor(&self, table: u32) -> Result<Cursor<'_>> {
        Cursor::open(self.env, &self.txn, table)
    }
}

/// A read-write transaction handle.
pub struct WriteTxn<'env> {
    /// The heed environment handle.
    env: &'env Env,
    /// The heed write transaction handle.
    txn: RwTxn<'env>,
}

impl<'env> WriteTxn<'env> {
    /// Opens a new write transaction handle, released on drop.
    pub fn open(env: &'env Env) -> Result<Self> {
        let txn = env.heed.write_txn()?;
        Ok(Self { env, txn })
    }

    /// Insert a row. Phase 1: surrogate-keyed only — the caller passes the row's
    /// JSON value and the storage layer assigns a fresh `u64` row id.
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.env.data.put(&mut self.txn, &key, &value)?;
        Ok(())
    }

    /// Commits the transaction.
    pub fn commit(self) -> Result<()> {
        self.txn.commit()?;
        Ok(())
    }

    /// Discard staged writes and abandon the heed transaction.
    pub fn abort(self) {
        drop(self);
    }

    /// Open a cursor that reads through the *committed* state (staged writes are invisible).
    pub fn cursor(&self, table: u32) -> Result<Cursor<'_>> {
        Cursor::open(self.env, &self.txn, table)
    }
}
