use crate::cursor::Cursor;
use crate::transaction::Transaction;

use std::path::Path;
use std::sync::Arc;

use heed::types::Bytes;
use heed::{Database, Env, EnvFlags, EnvOpenOptions, WithoutTls};

use crate::Result;

/// Reserved capacity for future named DBs (branches, commits, etc.).
const LMDB_MAX_DBS: u32 = 8;

/// Default virtual address-space reservation for the file (1 GiB).
const LMDB_MMAP_SIZE: usize = 1024 * 1024 * 1024;

/// LMDB b-tree handle; reusable across transactions.
pub type BTree = Database<Bytes, Bytes>;

/// The storage environment, shared across transactions.
#[derive(Clone)]
pub struct Storage {
    /// The inner LMDB environment.
    pub(crate) env: Arc<Env<WithoutTls>>,
}

impl Storage {
    /// Open or create a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let env = unsafe {
            EnvOpenOptions::new()
                .read_txn_without_tls()
                .map_size(LMDB_MMAP_SIZE)
                .max_dbs(LMDB_MAX_DBS)
                .flags(EnvFlags::NO_SUB_DIR)
                .open(path.as_ref())?
        };
        let env = Arc::new(env);
        Ok(Self { env })
    }

    /// Creates a new b-tree and returns a handle.
    pub fn create_btree(&self, txn: &mut Transaction, name: &str) -> Result<BTree> {
        let wtxn = txn.as_rw()?;
        let btree = self.env.create_database(wtxn, Some(name))?;
        Ok(btree)
    }

    pub fn open_btree(&self, txn: &Transaction, name: &str) -> Result<BTree> {
        let rtxn = txn.as_ro();
        let cursor = self.env.open_database(rtxn, Some(name))?.unwrap();
        Ok(cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::Transaction;
    use tempfile::TempDir;

    #[test]
    fn reopen_sees_committed_data() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        {
            let storage = Storage::open(&path).unwrap();
            let mut txn = Transaction::write(&storage).unwrap();
            let btree = storage.create_btree(&mut txn, "t").unwrap();
            btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
            txn.commit().unwrap();
        }
        let storage = Storage::open(&path).unwrap();
        let txn = Transaction::read(&storage).unwrap();
        let btree = storage.open_btree(&txn, "t").unwrap();
        assert_eq!(btree.get(txn.as_ro(), b"k").unwrap(), Some(b"v".as_slice()));
    }

    #[test]
    fn create_btree_returns_existing_handle() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(dir.path().join("t.db")).unwrap();
        let mut txn = Transaction::write(&storage).unwrap();
        let a = storage.create_btree(&mut txn, "x").unwrap();
        let b = storage.create_btree(&mut txn, "x").unwrap();
        a.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        assert_eq!(b.get(txn.as_ro(), b"k").unwrap(), Some(b"v".as_slice()));
    }

    #[test]
    fn multiple_read_txns_coexist() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(dir.path().join("t.db")).unwrap();
        let mut txn = Transaction::write(&storage).unwrap();
        let btree = storage.create_btree(&mut txn, "t").unwrap();
        btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        txn.commit().unwrap();

        let r1 = Transaction::read(&storage).unwrap();
        let r2 = Transaction::read(&storage).unwrap();
        assert_eq!(btree.get(r1.as_ro(), b"k").unwrap(), Some(b"v".as_slice()));
        assert_eq!(btree.get(r2.as_ro(), b"k").unwrap(), Some(b"v".as_slice()));
    }

    #[test]
    fn read_txn_holds_snapshot_through_concurrent_write() {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(dir.path().join("t.db")).unwrap();
        let mut txn = Transaction::write(&storage).unwrap();
        let btree = storage.create_btree(&mut txn, "t").unwrap();
        btree.put(txn.as_rw().unwrap(), b"a", b"1").unwrap();
        txn.commit().unwrap();

        let snap = Transaction::read(&storage).unwrap();
        assert_eq!(btree.get(snap.as_ro(), b"a").unwrap(), Some(b"1".as_slice()));
        assert_eq!(btree.get(snap.as_ro(), b"b").unwrap(), None);

        let mut w = Transaction::write(&storage).unwrap();
        btree.put(w.as_rw().unwrap(), b"b", b"2").unwrap();
        w.commit().unwrap();

        assert_eq!(btree.get(snap.as_ro(), b"b").unwrap(), None);
    }
}
