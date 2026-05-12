use crate::transaction::{Transaction, TransactionMode};

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
    env: Arc<Env<WithoutTls>>,
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

    /// Returns a read transaction.
    pub fn read(&self) -> Result<Transaction<'_>> {
        let txn = self.env.read_txn()?;
        let txn = txn.into();
        Ok(txn)
    }

    /// Returns a write transaction handle.
    pub fn write(&self) -> Result<Transaction<'_>> {
        let txn = self.env.write_txn()?;
        let txn = txn.into();
        Ok(txn)
    }

    /// Creates a new b-tree and returns a handle.
    pub fn create_btree(&self, txn: &mut Transaction<'_>, name: &str) -> Result<BTree> {
        let mut wtxn = txn.as_rw()?;
        let btree = self.env.create_database(&mut wtxn, Some(name))?;
        Ok(btree)
    }
}
