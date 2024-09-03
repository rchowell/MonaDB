use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter}, path::Path,
};

use crate::{
    error::Error, parser, table::{self, Table}, value::Row, Result
};
use rusqlite::{named_params, Connection};

/// Catalog manages the database tables.
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
    fn load_table(&self, name: &str) -> Result<&Table> {
        self.tables.get(name).ok_or(Error::TableNotFound(name.to_string()))
    }

    /// Create a table in the catalog.
    pub fn create_table(&mut self, table: &Table) -> Result<()> {
        let create = format!("CREATE TABLE IF NOT EXISTS {} (row TEXT);", table.name);
        let insert = "INSERT INTO catalog VALUES (:name, :rql);";
        //
        let tx = self.conn.transaction()?;
        tx.execute(&create, [])?;
        tx.execute(insert, named_params! { ":name": table.name, ":rql": table.to_string() })?;
        tx.commit()?;
        // 
        self.sync()
    }

    /// Drop a table from the catalog.
    pub fn drop_table(&mut self, name: &str) -> Result<()> {
        let drop = format!("DROP TABLE IF EXISTS {};", name);
        let delete = "DELETE FROM catalog WHERE name = :name;";
        //
        let tx = self.conn.transaction()?;
        tx.execute(&drop, [])?;
        tx.execute(delete, named_params! { ":name": name })?;
        tx.commit()?;
        // 
        self.sync()
    }

    /// Insert a row into the given table.
    pub fn insert(&mut self, table: &str, row: Row) -> Result<()> {
        let table = self.load_table(table)?;
        let insert = sql::insert(table);
        let mut stmt = self.conn.prepare(&insert)?;
        stmt.execute(named_params! { ":row": row.to_string() })?;
        Ok(())
    }

    /// Scan all rows from the given table.
    /// TODO use some kind of iterator instead of returning a Vec. 
    pub fn scan(&mut self, table: &str) -> Result<Vec<Row>> {
        let table = self.load_table(table)?;
        let scan = sql::scan(&table);
        let mut stmt = self.conn.prepare(&scan)?;
        let mut rows = stmt.query([])?;
        let mut values: Vec<Row> = vec![];
        while let Some(row) = rows.next()? {
            let value: String = row.get(0)?;
            let value = Row::from_str(&value);
            values.push(value);
        }
        Ok(values)
    }

    /// Sync the catalog with the sqlite3 `catalog` table.
    fn sync(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare(sql::SYNC)?;
        let mut rows = stmt.query([])?;
        let mut tables = HashMap::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let rql: String = row.get(1)?;
            let table = parser::parse_table(&rql)?;
            if table.name != name {
                return Err(Error::Unknown("Table name mismatch".to_string()));
            }
            tables.insert(name, table);
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

    pub fn scan(table: &Table) -> String {
        format!("SELECT row FROM {};", table.name)
    }
}

mod tests {
    use super::*;
    use rusqlite::Connection;
    use table::Schema;

    #[test]
    fn test_catalog() {
        let mut catalog = Catalog::memory().unwrap();
        let schema = Schema::empty();
        let table = Table::new("foo".to_string(), schema);
        catalog.create_table(&table).unwrap();
        let table = catalog.load_table("foo").unwrap();
        assert_eq!(table.name, "foo");
    }
}
