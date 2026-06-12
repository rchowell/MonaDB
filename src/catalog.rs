//! The catalog: table metadata, stored as rows in a reserved system table.
//!
//! Table definitions live as catalog rows (oid 0), like SQLite's
//! `sqlite_master` — each row is an [`Object`] carrying the original DDL.

use std::sync::Arc;

use heed::byteorder::{BigEndian, ByteOrder};
use serde::{Deserialize, Serialize};

use crate::MonaDB;
use crate::error::{Error, Result};
use crate::ir::{Create, Statement, TableDefinition};
use crate::storage::{BTree, Storage};
use crate::transaction::Transaction;

/// Reserved oid for the catalog table itself.
pub const CATALOG_OID: u32 = 0;

/// The single catalog object value, for now only tables.
#[derive(Serialize, Deserialize)]
pub struct Object {
    /// The object name, e.g. "catalog" or "users".
    pub name: String,
    /// The object kind, e.g. "table" or "view".
    #[serde(rename = "type")]
    pub kind: String,
    /// The original SQL statement that created the object.
    pub sql: String,
}

impl Object {
    /// Parses the stored `sql` back into its table definition.
    fn table_definition(&self) -> Result<TableDefinition> {
        if let Statement::Create(Create::Table(def)) = MonaDB::parse(&self.sql)? {
            Ok(def)
        } else {
            Err(Error::InternalError(format!(
                "catalog row for table '{}' is not a create table",
                self.name
            )))
        }
    }
}

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
            let obj = Object {
                name: "catalog".to_string(),
                kind: "table".to_string(),
                sql: "create table catalog;".to_string(),
            };
            let bytes = serde_json::to_vec(&obj)?;
            btree.put(txn.as_rw()?, &key, bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(Self {
            catalog: Arc::new(btree),
        })
    }

    /// Looks up a table by name, returning its full definition: oid, name, keys.
    pub fn get_table(&self, txn: &Transaction, name: &str) -> Result<TableDefinition> {
        for entry in self.catalog.iter(txn.as_ro())? {
            let (key, val) = entry?;
            let obj: Object = serde_json::from_slice(val)?;
            if obj.kind == "table" && obj.name == name {
                let mut def = obj.table_definition()?;
                def.oid = Some(BigEndian::read_u32(key));
                return Ok(def);
            }
        }
        Err(Error::UnboundTable(name.to_string()))
    }
}
