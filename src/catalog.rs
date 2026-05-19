use heed::byteorder::{BigEndian, ByteOrder};
use serde_json::json;

use crate::error::{Error, Result};
use crate::storage::{BTree, Storage};
use crate::transaction::Transaction;

/// Reserved oid for the catalog table itself.
pub const CATALOG_OID: u32 = 0;

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
        // Bootstrap the catalog table if it doesn't exist
        let mut txn = Transaction::write(&storage)?;
        let btree: BTree = storage.create_btree(&mut txn, CATALOG_OID)?;
        let key = CATALOG_OID.to_be_bytes();
        if btree.get(txn.as_ro(), &key)?.is_none() {
            let val = json!({
                "name": "catalog",
                "type": "table",
                "sql": "create table catalog;",
            });
            let bytes = serde_json::to_vec(&val)?;
            btree.put(txn.as_rw()?, &key, bytes.as_slice())?;
        }
        txn.commit()?;
        // I don't think we actually need this, but out of scope for now
        let objects = Table { name: "catalog".to_string(), btree };
        Ok(Self { storage, objects })
    }

    /// Look up a table by name and return its stable oid.
    pub fn get_table(&self, name: &str) -> Result<u32> {
        let txn = Transaction::read(&self.storage)?;
        let iter = self.objects.btree.iter(txn.as_ro())?;
        for entry in iter {
            let (key, val) = entry?;
            let val: serde_json::Value = serde_json::from_slice(val)?;
            if val.get("type").and_then(|v| v.as_str()) == Some("table")
                && val.get("name").and_then(|v| v.as_str()) == Some(name)
            {
                return Ok(BigEndian::read_u32(key));
            }
        }
        Err(Error::UnboundTable(name.to_string()))
    }
}

pub struct Table {
    /// The name of the table.
    pub name: String,
    /// The inner LMDB database handle; useable across transactions.
    pub btree: BTree,
}
