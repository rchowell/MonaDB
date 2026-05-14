use heed::{RoTxn, RwTxn, WithoutTls};

use crate::error::{Result, Error};

/// Transaction mode used as a flag during compilation. 
#[derive(Debug, Clone, Copy)]
pub enum TransactionMode {
    Ro,
    Rw,
}

/// Transaction handle wrapping LMDB transactions, tied to storage lifetime.
pub enum Transaction<'s> {
    Ro(RoTxn<'s, WithoutTls>),
    Rw(RwTxn<'s>),
}

impl<'s> Transaction<'s> {
    /// Borrow as a read txn; write transactions can deref as read-only.
    pub fn as_ro(&self) -> &RoTxn<'s> {
        match self {
            Transaction::Ro(t) => t,
            Transaction::Rw(t) => t,
        }
    }

    /// Borrow mutably as a write txn; fails if read-only.
    #[allow(clippy::unnecessary_wraps)]
    pub fn as_rw(&mut self) -> Result<&mut RwTxn<'s>, Error> {
        match self {
            Transaction::Ro(_) => panic!("read-only"),
            Transaction::Rw(t) => Ok(t),
        }
    }

    /// Commit the transaction, releasing any underlying resources.
    pub fn commit(self) -> Result<(), heed::Error> {
        match self {
            Transaction::Ro(t) => t.commit(),
            Transaction::Rw(t) => t.commit(),
        }
    }

    /// Abort the transaction; drop is sufficient too.
    pub fn abort(self) {
        drop(self);
    }
} 

/// Wraps a read-only LMDB transaction.
impl<'s> From<RoTxn<'s, WithoutTls>> for Transaction<'s> {
    fn from(txn: RoTxn<'s, WithoutTls>) -> Self {
        Self::Ro(txn)
    }
}

/// Wraps a read-write LMDB transaction
impl<'s> From<RwTxn<'s>> for Transaction<'s> {
    fn from(txn: RwTxn<'s>) -> Self {
        Self::Rw(txn)
    }
}
