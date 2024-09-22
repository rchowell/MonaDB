use std::{
    collections::HashMap, fmt::{self, Debug, Formatter}, path::Path
};

use crate::{
    error::Error, ir::Table, lexer::Lexer, parser::RqlParser, value::Row, Result
};
use rusqlite::{named_params, Connection, ToSql};

/// Catalog manages the database tables.
pub struct Catalog {
    conn: Connection,
    tables: HashMap<String, Table>,
    //
    transaction: Vec<String>,
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
            transaction: vec![],
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
        let create = format!("CREATE TABLE IF NOT EXISTS {} (row JSON);", table.name);
        let insert = "INSERT INTO catalog VALUES (:name, :rql);";
        //
        let tx = self.conn.transaction()?;
        tx.execute(&create, [])?;
        tx.execute(insert, named_params! { ":name": table.name, ":rql": table.to_string() })?;
        tx.commit()?;
        // 
        self.sync()
    }

    /// Delete all rows from the table.
    pub fn clear(&mut self, table: &str) -> Result<()> {
        let delete = format!("DELETE FROM {} WHERE true", table);
        //
        let tx = self.conn.transaction()?;
        tx.execute(&delete, [])?;
        tx.commit()?;
        //
        Ok(())
    }

    /// Drop a table from the catalog.
    pub fn drop(&mut self, table: &str) -> Result<()> {
        let drop = format!("DROP TABLE IF EXISTS {};", table);
        let delete = "DELETE FROM catalog WHERE name = :name;";
        //
        let tx = self.conn.transaction()?;
        tx.execute(&drop, [])?;
        tx.execute(delete, named_params! { ":name": table })?;
        tx.commit()?;
        // 
        self.sync()
    }

    /// Insert a row into the given table.
    pub fn insert(&mut self, table: &str, row: Row) -> Result<()> {
        let table = self.load_table(table)?;
        let insert = format!("INSERT INTO {} VALUES (?);", table.name);
        let mut insert = self.conn.prepare(&insert)?;
        insert.raw_bind_parameter(1, row)?;
        insert.expanded_sql();
        let insert = insert.expanded_sql().unwrap();
        if self.transaction.is_empty() {
            self.conn.execute(&insert, [])?;
        } else {
            self.transaction.push(insert);
        }
        Ok(())
    }

    /// Scan all rows from the given table.
    /// TODO use some kind of iterator instead of returning a Vec. 
    pub fn scan(&mut self, table: &str) -> Result<Vec<Row>> {
        let table = self.load_table(table)?;
        let scan = format!("SELECT row FROM {};", table.name);
        let mut stmt = self.conn.prepare(&scan)?;
        let mut rows = stmt.query([])?;
        let mut values: Vec<Row> = vec![];
        while let Some(row) = rows.next()? {
            let value: String = row.get(0)?;
            let value = Row::from_str(&value)?;
            values.push(value);
        }
        Ok(values)
    }

    /// Begin a (goofy) transaction (really just a batch of SQL statements).
    pub fn transaction(&mut self) {
        self.transaction.clear();
    }

    /// Commit the current transaction.
    pub fn commit(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        for sql in &self.transaction {
            tx.execute(sql, [])?;
        }
        tx.commit()?;
        self.transaction.clear();
        Ok(())
    }

    /// Sync the catalog with the sqlite3 `catalog` table.
    fn sync(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare(sql::SYNC)?;
        let mut rows = stmt.query([])?;
        let mut tables = HashMap::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let ddl: String = row.get(1)?;
            let table = parse_table(&ddl)?;
            if table.name != name {
                return Err(Error::TableNotFound("Table name mismatch".to_string()));
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
        writeln!(f, "\nCatalog")?;
        writeln!(f, "-------\n")?;
        for (_, table) in &self.tables {
            writeln!(f, "{}", table)?;
        }
        Ok(())
    }
}

impl ToSql for Row {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        use rusqlite::types::ToSqlOutput;
        use rusqlite::types::Value;
        Ok(ToSqlOutput::Owned(Value::Text(self.to_string())))
    }
}

mod sql {
    pub const SYNC: &str = "SELECT name, rql FROM catalog;";
}

fn parse_table(ddl: &str) -> Result<Table> {
    use crate::ir::*;
    let rl = Lexer::new(ddl);
    let rp = RqlParser::new();
    let ddl = rp.parse(rl)?;
    if let Statement::Create(Create::Table(table)) = ddl {
        Ok(table)
    } else {
        Err(Error::SyntaxError("Expected CREATE TABLE statement".to_string()))
    }
}

// Helper function to return a comma-separated sequence of `?`.
// - `repeat_vars(0) => panic!(...)`
// - `repeat_vars(1) => "?"`
// - `repeat_vars(2) => "?,?"`
// - `repeat_vars(3) => "?,?,?"`
// - ...
fn repeat_vars(count: usize) -> String {
    assert_ne!(count, 0);
    let mut s = "?,".repeat(count);
    // Remove trailing comma
    s.pop();
    s
}

#[cfg(test)]
mod tests {
    use crate::ir::{self, Type};
    use super::*;

    #[test]
    fn test_catalog() {
        let mut catalog = Catalog::memory().unwrap();
        let table = Table {
            name: "foo".to_string(),
            members: vec![
                ir::table_member("id".into(), Type::Number),
                ir::table_member("name".into(), Type::String),
            ],
        };
        println!("{}", table);
        catalog.create_table(&table).unwrap();
        let table = catalog.load_table("foo").unwrap();
        assert_eq!(table.name, "foo");
    }
}
