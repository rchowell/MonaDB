//! The catalog: table metadata, stored as rows in a reserved system table.
//!
//! Table definitions live as catalog rows (oid 0), like SQLite's
//! `sqlite_master` — each row is an object carrying the original DDL. Catalog
//! rows are written through the ordinary insert path (`Vop::Insert`), so they use
//! the same flat storage codec as data rows (see [`crate::value::Value::encode`]).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use heed::byteorder::{BigEndian, ByteOrder};

use crate::MonaDB;
use crate::error::{Error, Result};
use crate::ir::{Create, Statement, TableDefinition};
use crate::storage::{BTree, Storage};
use crate::transaction::Transaction;
use crate::value::Value;

/// Reserved oid for the catalog table itself.
pub const CATALOG_OID: u32 = 0;

/// Parses a catalog row's stored `sql` back into its table definition.
fn parse_table_definition(sql: &str, name: &str) -> Result<TableDefinition> {
    match MonaDB::parse(sql)? {
        Statement::Create(Create::Table(def) | Create::TableAs { def, .. }) => Ok(def),
        _ => Err(Error::InternalError(format!(
            "catalog row for table '{name}' is not a create table"
        ))),
    }
}

/// Catalog handles all table metadata; cheap to clone.
#[derive(Clone)]
pub struct Catalog {
    /// The 'catalog' table holds all objects (tables) like `sqlite_master`.
    catalog: Arc<BTree>,
    /// In-memory schema cache: table name → parsed definition, populated lazily
    /// on lookup. Shared across clones (an `Rc` bump per bind) and flushed
    /// wholesale when the catalog generation advances. Mirrors SQLite's
    /// connection-resident parsed schema, so a bind that hits the cache avoids
    /// the catalog btree scan and the re-parse of the stored `create table` DDL.
    cache: Rc<RefCell<CatalogCache>>,
}

/// The cached schema and the catalog generation it was captured at. A bump in
/// the generation (any CREATE/DROP) invalidates the whole snapshot.
struct CatalogCache {
    /// The generation this snapshot is valid for.
    generation: u64,
    /// Resolved lookups, keyed by name: `Some(def)` for a present table,
    /// `None` for a known-absent one (a negative tombstone). Both are flushed on
    /// a generation bump, so a table created later still resolves.
    tables: HashMap<String, Option<TableDefinition>>,
}

/// The result of a catalog cache lookup ([`Catalog::cached`]).
pub enum CacheLookup {
    /// A present table, cached.
    Hit(TableDefinition),
    /// A known-absent table, cached as a negative tombstone.
    NegativeHit,
    /// Not in the cache — the caller must scan.
    Miss,
}

impl Catalog {
    /// Loads the catalog from the storage environment.
    pub fn load(storage: &Storage) -> Result<Self> {
        // Bootstrap the catalog table if it doesn't exist
        let mut txn = storage.write_txn()?;
        let key = CATALOG_OID.to_be_bytes();
        let btree: BTree = storage.create_btree(&mut txn, CATALOG_OID)?;
        if btree.get(txn.as_ro(), &key)?.is_none() {
            let row = Value::from(serde_json::json!({
                "name": "catalog",
                "type": "table",
                "sql": "create table catalog;",
            }));
            btree.put(txn.as_rw()?, &key, row.encode()?.as_slice())?;
        }
        txn.commit()?;
        Ok(Self {
            catalog: Arc::new(btree),
            cache: Rc::new(RefCell::new(CatalogCache {
                generation: 0,
                tables: HashMap::new(),
            })),
        })
    }

    /// Returns a cached lookup, flushing the whole cache first if `generation`
    /// has advanced past the captured snapshot.
    ///
    /// Touches no storage and opens no transaction — the binder's fast path.
    pub fn cached(&self, name: &str, generation: u64) -> CacheLookup {
        let mut cache = self.cache.borrow_mut();
        if cache.generation != generation {
            cache.tables.clear();
            cache.generation = generation;
        }
        match cache.tables.get(name) {
            Some(Some(def)) => CacheLookup::Hit(def.clone()),
            Some(None) => CacheLookup::NegativeHit,
            None => CacheLookup::Miss,
        }
    }

    /// Cold path: scans the catalog btree for `name`, parses its stored DDL, and
    /// caches the outcome. Call only after [`cached`] has returned `None` (so the
    /// cache generation is already current). Both a hit and a `UnboundTable` miss
    /// are cached (the latter as a tombstone); a CREATE bumps the generation and
    /// flushes them, so a table created later still resolves. This is the per-row
    /// scan + DDL re-parse the cache elides.
    pub fn scan_and_cache(&self, txn: &Transaction, name: &str) -> Result<TableDefinition> {
        match self.scan_table(txn, name) {
            Ok(def) => {
                self.cache
                    .borrow_mut()
                    .tables
                    .insert(name.to_owned(), Some(def.clone()));
                Ok(def)
            }
            Err(Error::UnboundTable(_)) => {
                self.cache.borrow_mut().tables.insert(name.to_owned(), None);
                Err(Error::UnboundTable(name.to_string()))
            }
            // A transient or parse error is not a stable absence — don't cache it.
            Err(e) => Err(e),
        }
    }

    /// Scans the catalog btree for `name`, parsing its stored DDL (no caching).
    fn scan_table(&self, txn: &Transaction, name: &str) -> Result<TableDefinition> {
        for entry in self.catalog.iter(txn.as_ro())? {
            let (key, val) = entry?;
            let row = Value::from_storage(val)?;
            let is_table = row.jpk("type").as_ref().and_then(Value::as_str) == Some("table");
            let row_name = row.jpk("name");
            if is_table && row_name.as_ref().and_then(Value::as_str) == Some(name) {
                let sql = row.jpk("sql").and_then(|v| v.as_str().map(str::to_owned));
                let sql = sql.ok_or_else(|| {
                    Error::InternalError(format!("catalog row for table '{name}' missing sql"))
                })?;
                let mut def = parse_table_definition(&sql, name)?;
                def.oid = Some(BigEndian::read_u32(key));
                return Ok(def);
            }
        }
        Err(Error::UnboundTable(name.to_string()))
    }
}
