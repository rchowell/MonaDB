use std::sync::Arc;

use heed::byteorder::{BigEndian, ByteOrder};
use serde_json::json;

use crate::error::{Error, Result};
use crate::storage::{BTree, Storage};
use crate::transaction::Transaction;

/// Reserved oid for the catalog table itself.
pub const CATALOG_OID: u32 = 0;

/// Catalog handles all table metadata; cheap to clone.
#[derive(Clone)]
pub struct Catalog {
    /// The 'catalog' table holds all objects (tables) like `sqlite_master`.
    catalog: Arc<BTree>,
}

impl Catalog {
    /// Loads the catalog from the storage environment.
    pub fn load(storage: &Storage) -> Result<Self> {
        // Bootstrap the catalog table if it doesn't exist
        let mut txn = storage.write_txn()?;
        let key = CATALOG_OID.to_be_bytes();
        let btree: BTree = storage.create_btree(&mut txn, CATALOG_OID)?;
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
        Ok(Self {
            catalog: Arc::new(btree),
        })
    }

    /// Look up a table by name and return its stable oid using a provided transaction.
    pub fn get_table(&self, txn: &Transaction, name: &str) -> Result<u32> {
        let iter = self.catalog.iter(txn.as_ro())?;
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
