use heed::{RoTxn, RwTxn, WithoutTls};

use crate::{error::{Error, Result}, storage::Storage};

/// Transaction mode used as a flag during compilation.
#[derive(Debug, Clone, Copy)]
pub enum TransactionMode {
    Read,
    Write,
}

impl TransactionMode {
    /// Combines the transaction modes to the common type.
    pub fn coalesce(self, other: Option<TransactionMode>) -> TransactionMode {
        match (self, other) {
            // If either is read-write, return read-write
            (_, Some(TransactionMode::Write)) | (TransactionMode::Write, _) => TransactionMode::Write,
            // Fallback to read-only
            _ => TransactionMode::Read,
        }
    }
}

/// Transaction handle wrapping LMDB transactions.
pub struct Transaction {
    /// Dropped first.
    inner: TransactionInner,
    /// Dropped last; keeps Arc<Env> alive for inner.
    _storage: Storage,
}

/// Inner transaction state actually holding the LMDB handles.
enum TransactionInner {
    /// Read-only LMDB transaction; lifetime-erased, backed by storge.
    Read(RoTxn<'static, WithoutTls>),
    /// Read-write LMDB transaction; lifetime-erased, backed by storage.
    Write(RwTxn<'static>),
}

impl Transaction {
    pub fn new(storage: &Storage, mode: TransactionMode) -> Result<Self> {
        match mode {
            TransactionMode::Read => Self::read(storage),
            TransactionMode::Write => Self::write(storage),
        }
    }

    /// Creates a new read transaction.
    pub fn read(storage: &Storage) -> Result<Self> {
        // SAFETY: The transaction's lifetime is bound by the storage
        // which is dropped AFTER the transaction. This is required
        // because the VM requires self-references where transactions
        // point to storage.
        let txn = storage.env.read_txn()?;
        let txn: RoTxn<'static, WithoutTls> = unsafe { std::mem::transmute(txn) };
        let txn = TransactionInner::Read(txn);
        Ok(Self { inner: txn, _storage: storage.clone() })
    }

    /// Creates a new write transaction.
    pub fn write(storage: &Storage) -> Result<Self> {
        // SAFETY: The transaction's lifetime is bound by the storage
        // which is dropped AFTER the transaction. This is required
        // because the VM requires self-references where transactions
        // point to storage.
        let txn = storage.env.write_txn()?;
        let txn: RwTxn<'static> = unsafe { std::mem::transmute(txn) };
        let txn = TransactionInner::Write(txn);
        Ok(Self { inner: txn, _storage: storage.clone() })
    }

    /// Borrow as a read txn; write transactions can deref as read-only.
    pub fn as_ro(&self) -> &RoTxn<'_, WithoutTls> {
        match &self.inner {
            TransactionInner::Read(t) => t,
            TransactionInner::Write(t) => t,
        }
    }

    /// Borrow mutably as a write txn; fails if read-only.
    #[allow(clippy::unnecessary_wraps)]
    pub fn as_rw(&mut self) -> Result<&mut RwTxn<'static>> {
        match &mut self.inner {
            TransactionInner::Read(_) => Err(Error::InternalError("transaction is read-only".into())),
            TransactionInner::Write(t) => Ok(t),
        }
    }

    /// Commit the transaction, releasing any underlying resources.
    pub fn commit(self) -> Result<()> {
        match self.inner {
            TransactionInner::Read(t) => Ok(t.commit()?),
            TransactionInner::Write(t) => Ok(t.commit()?),
        }
    }

    /// Abort the transaction; drop is sufficient too.
    pub fn abort(self) {
        drop(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BTree;
    use tempfile::TempDir;

    fn open() -> (TempDir, Storage) {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(dir.path().join("test.db")).unwrap();
        (dir, storage)
    }

    fn make_btree(storage: &Storage, name: &str) -> BTree {
        let mut txn = Transaction::write(storage).unwrap();
        let btree = storage.create_btree(&mut txn, name).unwrap();
        txn.commit().unwrap();
        btree
    }

    #[test]
    fn read_then_commit_is_noop() {
        let (_dir, storage) = open();
        Transaction::read(&storage).unwrap().commit().unwrap();
    }

    #[test]
    fn write_then_commit_persists() {
        let (_dir, storage) = open();
        let btree = make_btree(&storage, "t");
        let mut txn = Transaction::write(&storage).unwrap();
        btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        txn.commit().unwrap();

        let ro = Transaction::read(&storage).unwrap();
        assert_eq!(btree.get(ro.as_ro(), b"k").unwrap(), Some(b"v".as_slice()));
    }

    #[test]
    fn write_then_drop_aborts() {
        let (_dir, storage) = open();
        let btree = make_btree(&storage, "t");
        {
            let mut txn = Transaction::write(&storage).unwrap();
            btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        }
        let ro = Transaction::read(&storage).unwrap();
        assert_eq!(btree.get(ro.as_ro(), b"k").unwrap(), None);
    }

    #[test]
    fn abort_discards_writes() {
        let (_dir, storage) = open();
        let btree = make_btree(&storage, "t");
        let mut txn = Transaction::write(&storage).unwrap();
        btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        txn.abort();

        let ro = Transaction::read(&storage).unwrap();
        assert_eq!(btree.get(ro.as_ro(), b"k").unwrap(), None);
    }

    #[test]
    fn as_rw_on_read_returns_internal_error() {
        let (_dir, storage) = open();
        let mut txn = Transaction::read(&storage).unwrap();
        assert!(matches!(txn.as_rw(), Err(Error::InternalError(_))));
    }

    #[test]
    fn as_ro_on_write_succeeds() {
        let (_dir, storage) = open();
        let _btree = make_btree(&storage, "t");
        let txn = Transaction::write(&storage).unwrap();
        let _ = txn.as_ro();
    }

    #[test]
    fn new_dispatches_by_mode() {
        let (_dir, storage) = open();
        let mut rt = Transaction::new(&storage, TransactionMode::Read).unwrap();
        assert!(rt.as_rw().is_err());
        let mut wt = Transaction::new(&storage, TransactionMode::Write).unwrap();
        assert!(wt.as_rw().is_ok());
    }

    #[test]
    fn coalesce_table() {
        use TransactionMode::{Read, Write};
        assert!(matches!(Read.coalesce(None), Read));
        assert!(matches!(Read.coalesce(Some(Read)), Read));
        assert!(matches!(Read.coalesce(Some(Write)), Write));
        assert!(matches!(Write.coalesce(None), Write));
        assert!(matches!(Write.coalesce(Some(Read)), Write));
        assert!(matches!(Write.coalesce(Some(Write)), Write));
    }

    #[test]
    fn transaction_outlives_original_storage() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let storage = Storage::open(&path).unwrap();
        let btree = make_btree(&storage, "t");
        let mut txn = Transaction::write(&storage).unwrap();
        btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        drop(storage);
        txn.commit().unwrap();

        let storage = Storage::open(&path).unwrap();
        let ro = Transaction::read(&storage).unwrap();
        let btree = storage.open_btree(&ro, "t").unwrap();
        assert_eq!(btree.get(ro.as_ro(), b"k").unwrap(), Some(b"v".as_slice()));
    }
}
