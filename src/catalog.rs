use crate::storage::{BTree, Storage};

use crate::error::Result;

/// Catalog handles all table metadata.
pub struct Catalog {
    /// Owned reference to the storage environment.
    storage: Storage,
    /// Table of objects for plan binding.
    objects: Table,
}

impl Catalog {
    /// Loads the catalog from the storage environment.
    pub fn load(storage: Storage) -> Result<Self> {
        // Create system table(s); this is idempotent.
        let mut txn = storage.write()?;
        let objects: BTree = storage.create_btree(&mut txn, "catalog")?;
        let objects = Table { name: "catalog".to_string(), btree: objects };
        txn.commit()?;
        Ok(Self { storage, objects })
    }
}

pub struct Table {
    /// The name of the table.
    pub name: String,
    /// The inner LMDB database handle; useable across transactions.
    pub btree: BTree,
}
