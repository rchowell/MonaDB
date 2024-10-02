use std::{
    collections::HashMap, fmt::{self, Debug, Formatter}, path::Path
};

use crate::{
    error::Error, ir::{self, Table}, lexer::RqlLexer, parser::RqlParser, value::{Record, Value}, Result
};
use rusqlite::{named_params, params_from_iter, Connection, ParamsFromIter, ToSql};
use sqlite::ToSqlite;

/// Catalog manages the database tables.
pub struct Catalog {
    conn: Connection,
    tables: HashMap<String, Table>,
}

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
        conn.execute(sqlite::INIT, [])?;
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
        let create = table.to_sqlite_ddl();
        let insert = "INSERT INTO catalog VALUES (:name, :ddl);";
        //
        let tx = self.conn.transaction()?;
        tx.execute(&create, [])?;
        tx.execute(insert, named_params! { ":name": table.name, ":ddl": table.to_string() })?;
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

    /// Insert a record into the given table.
    /// 
    /// TODO `insert_batch` which prepares then calls execute many times in a transaction.
    /// 
    pub fn insert(&mut self, table: &str, record: Record) -> Result<usize> {
        let table = self.load_table(table)?;
        let insert = table.to_sqlite_insert();
        let params = to_params(record, table);
        let n = self.conn.execute(&insert, params)?;
        Ok(n)
    }

    /// Scan all rows from the given table.
    pub fn scan(&mut self, table: &str) -> Result<Vec<Record>> {

        let table = self.load_table(table)?;
        let scan = table.to_scan();
        let mut stmt = self.conn.prepare(&scan)?;
        let mut rows = stmt.query([])?;

        // TODO use some kind of iterator instead of returning a Vec. 
        let mut values: Vec<Record> = vec![];
        while let Some(row) = rows.next()? {
            let value: String = row.get(0)?;
            let value = Record::from_str(&value)?;
            values.push(value);
        }
        Ok(values)
    }

    /// Begin a (goofy) transaction (really just a batch of SQL statements).
    pub fn transaction(&mut self) {
        // self.transaction.clear();
    }

    /// Commit the current transaction.
    pub fn commit(&mut self) -> Result<()> {
        // let tx = self.conn.transaction()?;
        // for sql in &self.transaction {
        //     tx.execute(sql, [])?;
        // }
        // tx.commit()?;
        // self.transaction.clear();
        Ok(())
    }

    /// Sync the catalog with the sqlite3 `catalog` table.
    fn sync(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare(sqlite::SYNC)?;
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

fn to_params(record: Record, table: &Table) -> ParamsFromIter<Vec<Value>> {
    let row = record.shred(&table.members);
    let iter = row.values();
    params_from_iter(iter)
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

impl ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        use rusqlite::types::ToSqlOutput;
        use rusqlite::types::Value;
        Ok(ToSqlOutput::Owned(Value::Text(self.to_string())))
    }
}

fn parse_table(ddl: &str) -> Result<Table> {
    use crate::ir::*;
    let rl = RqlLexer::new(ddl);
    let rp = RqlParser::new();
    let ddl = rp.parse(rl)?;
    if let Statement::Create(Create::Table(table)) = ddl {
        Ok(table)
    } else {
        Err(Error::SyntaxError("Expected `create table` statement".to_string()))
    }
}

impl ir::Table {
    /// Convert the table to a scan query.
    pub fn to_scan(&self) -> String {
        format!("SELECT * FROM {}", self.name)
    }

    pub fn to_sqlite_insert(&self) -> String {
        let members: String = self.members
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<&str>>()
            .join(", ");
        let params = repeat_vars(self.members.len());
        format!("INSERT INTO {} ({}) VALUES ({})", self.name, members, params)
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

/// sqlite3 tranlsations
mod sqlite {
    use crate::ir;

    pub const INIT: &str = "CREATE TABLE IF NOT EXISTS catalog ( name TEXT PRIMARY KEY, ddl TEXT);";
    pub const SYNC: &str = "SELECT name, ddl FROM catalog;";

    /// The `ToSqlite` trait is used to convert a rho object to a SQLite object.
    pub trait ToSqlite {
        /// TODO implementations consider https://github.com/hoodie/concatenation_benchmarks-rs
        fn to_sqlite_ddl(&self) -> String;
    }

    /// Write the table definition as an sqlite3 string.
    impl ToSqlite for ir::Table {

        fn to_sqlite_ddl(&self) -> String {
            let mut sql = String::new();
            sql.push_str(format!("CREATE TABLE {} (\n", self.name).as_str());
            for (i, m) in self.members.iter().enumerate() {
                let col = m.to_sqlite_ddl();
                if i > 0 {
                    sql.push_str(",");
                    sql.push_str("\n");
                }
                sql.push_str("  ");
                sql.push_str(&col);
            }
            sql.push_str("\n)");
            sql
        }
    }

    impl ToSqlite for ir::TableMember {
        fn to_sqlite_ddl(&self) -> String {
            let typ_ = self.typ_.to_sqlite_ddl();
            if self.nullable {
                format!("{} {} NULL", self.name, typ_)
            } else {
                format!("{} {} NOT NULL", self.name, typ_)
            }
        }
    }

    impl ToSqlite for ir::Type {
        fn to_sqlite_ddl(&self) -> String {
            match self {
                ir::Type::Bool => "INT",
                ir::Type::Number => "REAL",
                ir::Type::String => "TEXT",
                ir::Type::Object => "ANY",
                ir::Type::Array => "ANY",
                ir::Type::Any => "ANY",
            }.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ir::{self, Type};
    use super::*;

    #[test]
    fn test_create_table() {
        let mut catalog = Catalog::memory().unwrap();
        let actual = Table {
            name: "foo".to_string(),
            members: vec![
                ir::table_member("id".into(), Type::Number, false),
                ir::table_member("name".into(), Type::String, true),
            ],
            constraints: vec![],
        };

        // load the table
        catalog.create_table(&actual).unwrap();
        let expected = catalog.load_table("foo").unwrap();

        // assert round-trip equality
        assert_eq!(actual, *expected);
    }

    #[test]
    fn test_to_sqlite_ddl() {
        let rql = "create table example ( x number, y number|null, );";
        let tbl = parse_table(rql).unwrap();
        let sql = tbl.to_sqlite_ddl();
        println!("{}", sql);
    }

    #[test]
    fn test_to_sqlite_insert() {
        // let mut catalog = Catalog::memory().unwrap();
        // catalog.create_table(&tbl).unwrap();
        // catalog.insert("example", row)
        let rql = "create table example ( x number, y number|null, );";
        let tbl = parse_table(rql).unwrap();
        let sql = tbl.to_sqlite_insert();
        println!("{}", sql);
    }
}
