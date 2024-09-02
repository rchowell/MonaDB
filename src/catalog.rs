use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter}, path::Path,
};

use crate::{
    error::Error, table::{Row, Table}, Result
};
use rusqlite::{named_params, Connection};

/// Catalog controls all access to the underlying storage (sqlite).
/// This is like the "InnerConnection" to the actual storage, where Rho is the interface.
pub struct Catalog {
    conn: Connection,
    tables: HashMap<String, Table>,
}

const CREATE_CATALOG: &str = "
    CREATE TABLE IF NOT EXISTS catalog (
        name TEXT PRIMARY KEY,
        rql TEXT
    );";


impl Catalog {
    /// Load the catalog from a file.
    pub fn open<P>(path: P) -> Result<Catalog>
    where P: AsRef<Path> {
        let conn = Connection::open(path)?;
        let mut catalog = Catalog::init(conn)?;
        catalog.sync()?;
        Ok(catalog)
    }

    /// Load the catalog from an in-memory database.
    pub fn memory() -> Result<Catalog> {
        let conn = Connection::open_in_memory()?;
        let catalog = Catalog::init(conn)?;
        Ok(catalog)
    }

    /// Initialize the catalog.
    fn init(conn: Connection) -> Result<Catalog> {
        conn.execute(CREATE_CATALOG, [])?;
        Ok(Catalog {
            conn,
            tables: HashMap::new(),
        })
    }

    /// Load a table by name.
    ///
    /// # Errors
    /// - TableNotFound](error::Error::TableNotFound).
    /// 
    pub fn load_table(&self, name: &str) -> Result<&Table> {
        self.tables.get(name).ok_or(Error::TableNotFound(name.to_string()))
    }

    /// Create a table in the catalog.
    pub fn create_table(&mut self, table: Table) -> Result<()> {
        let create = format!("CREATE TABLE IF NOT EXISTS {} (row TEXT);", table.name);
        let insert = "INSERT INTO catalog VALUES (:name, :rql);";

        //
        let tx = self.conn.transaction()?;
        tx.execute(&create, [])?;
        tx.execute(insert, named_params! { ":name": table.name, ":rql": table.rql })?;
        tx.commit()?;

        self.sync()
    }

    /// Insert a row into the given table.
    pub fn insert(&mut self, table: &Table, row: Row) -> Result<()> {
        let insert = sql::insert(table);
        let mut stmt = self.conn.prepare(&insert)?;
        stmt.execute(named_params! { ":row": row.to_string() })?;
        Ok(())
    }

    /// Sync the catalog with the sqlite3 `catalog` table.
    fn sync(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare(sql::SYNC)?;
        let mut rows = stmt.query([])?;
        let mut tables = HashMap::new();
        while let Some(row) = rows.next()? {
            let table = Table {
                name: row.get(0)?,
                rql: row.get(1)?,
            };
            tables.insert(table.name.clone(), table);
        }
        // Replace the catalog
        self.tables = tables;
        Ok(())
    }
}

impl Debug for Catalog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Catalog")?;
        for table in &self.tables {
            write!(f, "\n{:?}", table)?;
        }
        Ok(())
    }
}

mod sql {
    use crate::table::Table;

    pub const SYNC: &str = "SELECT name, rql FROM catalog;";

    pub fn insert(table: &Table) -> String {
        format!("INSERT INTO {} VALUES (:row);", table.name)
    }

}

mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_catalog() {
        let mut catalog = Catalog::memory().unwrap();
        let table = Table {
            name: "foo".to_string(),
            rql: "select * from foo".to_string(),
        };
        catalog.create_table(table).unwrap();
        let table = catalog.load_table("foo").unwrap();
        assert_eq!(table.name, "foo");
        assert_eq!(table.rql, "select * from foo");
    }
}
