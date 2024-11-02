use core::str;
use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    path::Path,
};

use crate::{
    cursor::Cursor,
    error::Error,
    ir::{self, Table},
    lexer::RqlLexer,
    parser::RqlParser,
    value::Value,
    Result,
};
use rusqlite::{named_params, types::FromSql, Connection, ToSql};

const SQL_INIT: &str = "CREATE TABLE IF NOT EXISTS catalog ( name TEXT PRIMARY KEY, ddl TEXT );";
const SQL_SYNC: &str = "SELECT name, ddl FROM catalog;";

/// Catalog manages the database tables, routines, and eventually types.
pub struct Catalog {
    conn: Connection,
    tables: HashMap<String, Table>,
    routines: HashMap<String, i32>,
}

impl Catalog {
    /// Load the catalog from a file.
    pub fn open<P>(path: P) -> Result<Catalog>
    where
        P: AsRef<Path>,
    {
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
        conn.execute(SQL_INIT, [])?;
        Ok(Catalog {
            conn,
            tables: HashMap::new(),
            routines: Self::routines(),
        })
    }

    /// Lookup a table by name.
    pub fn get_table(&self, name: &str) -> Result<&Table> {
        self.tables
            .get(name)
            .ok_or(Error::UnknownTable(name.to_string()))
    }

    /// Lookup a routine by name and arity.
    pub fn get_routine(&self, _name: &str, _arity: usize) -> Result<()> {
        // self.routines
        // .get(name)
        // .map(|&v| v)
        // .ok_or(Error::UnknownRoutine(name.to_string()))
        Ok(())
    }

    /// Create a table in the catalog.
    pub fn create_table(&mut self, table: &Table) -> Result<()> {
        let create = table.to_sqlite_ddl();
        let insert = "INSERT INTO catalog VALUES (:name, :ddl);";
        //
        let tx = self.conn.transaction()?;
        tx.execute(&create, [])?;
        tx.execute(
            insert,
            named_params! { ":name": table.name, ":ddl": table.to_string() },
        )?;
        tx.commit()?;
        //
        self.sync()
    }

    /// Delete all rows from the table.
    pub fn clear(&mut self, table: &str) -> Result<()> {
        let delete = format!("DELETE FROM {} WHERE true", table);

        let tx = self.conn.transaction()?;
        tx.execute(&delete, [])?;
        tx.commit()?;

        Ok(())
    }

    /// Drop a table from the catalog.
    pub fn drop(&mut self, table: &str) -> Result<()> {
        let drop = format!("DROP TABLE IF EXISTS {};", table);
        let delete = "DELETE FROM catalog WHERE name = :name;";

        let tx = self.conn.transaction()?;
        tx.execute(&drop, [])?;
        tx.execute(delete, named_params! { ":name": table })?;
        tx.commit()?;

        self.sync()
    }

    /// Insert a value into the table.
    pub fn insert(&mut self, table: &str, value: Value) -> Result<()> {
        let table = self.get_table(table)?;
        let insert = table.to_sqlite_insert();
        let v = value.to_string();
        let _ = self.conn.execute(&insert, [v])?;
        Ok(())
    }

    /// TODO `insert_batch` which prepares then calls execute many times in a transaction.
    pub fn insert_batch(&mut self, _table: &str, _values: &[Value]) -> Result<()> {
        todo!("insert_batch")
    }

    /// Open a cursor to the given table.
    pub fn scan(&mut self, table: &str) -> Result<Cursor> {
        let table = self.get_table(table)?;
        let select = table.to_sqlite_select();
        let statement = self.conn.prepare(&select)?;
        Ok(Cursor::new(statement))
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
        let mut stmt = self.conn.prepare(SQL_SYNC)?;
        let mut rows = stmt.query([])?;
        let mut tables = HashMap::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let ddl: String = row.get(1)?;
            let table = parse_table(&ddl)?;
            if table.name != name {
                return Err(Error::UnknownTable("Table name mismatch".to_string()));
            }
            tables.insert(name, table);
        }
        // Replace the catalog
        self.tables = tables;
        Ok(())
    }

    // TODO fn pointers
    fn routines() -> HashMap<String, i32> {
        [("upper", 1), ("lower", 1)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }
}

/// Convert a `rusqlite::types::ValueRef` to a `Value`.
impl FromSql for Value {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        use rusqlite::types::ValueRef;
        let v = match value {
            ValueRef::Null => Value::null(),
            ValueRef::Integer(i) => Value::number(i as f64),
            ValueRef::Real(r) => Value::number(r),
            ValueRef::Text(b) => Value::string(str::from_utf8(b).unwrap().to_string()),
            ValueRef::Blob(_) => unimplemented!("invalid data type 'blob'"),
        };
        Ok(v)
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

// Convert a `Value` to a `rusqlite::types::Value`.
impl ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        use rusqlite::types::ToSqlOutput;
        use rusqlite::types::Value;
        Ok(ToSqlOutput::Owned(Value::Text(self.to_string())))
    }
}

// TODO add to `extern { .. }` of lalrpop.
fn parse_table(ddl: &str) -> Result<Table> {
    use crate::ir::*;
    let rl = RqlLexer::new(ddl);
    let rp = RqlParser::new();
    let ddl = rp.parse(rl)?;
    if let Statement::Create(Create::Table(table)) = ddl {
        Ok(table)
    } else {
        Err(Error::SyntaxError(
            "Expected `create table` statement".to_string(),
        ))
    }
}

impl ir::Table {
    /// The sqlite `CREATE TABLE` statement.
    fn to_sqlite_ddl(&self) -> String {
        format!("CREATE TABLE {} (_ BLOB)", self.name)
    }

    /// Convert the table to a scan query (with primary key rowid->oid).
    pub fn to_sqlite_select(&self) -> String {
        format!("SELECT oid as oid, _ FROM {}", self.name)
    }

    /// Convert the table to an insert query with the appropriate parameters (i.e. ?).
    pub fn to_sqlite_insert(&self) -> String {
        format!("INSERT INTO {} (_) VALUES (?)", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::{TMember, TObject, Type};

    const DDL: &str = "create table example ({ x: number, y: number|null });";

    #[test]
    fn test_create_table() {
        let mut catalog = Catalog::memory().unwrap();
        let actual = Table {
            name: "foo".to_string(),
            schema: Type::Object(TObject {
                members: vec![
                    TMember {
                        name: "x".to_owned(),
                        typ_: Box::new(Type::Number),
                    },
                    TMember {
                        name: "y".to_owned(),
                        typ_: Box::new(Type::Number),
                    },
                ],
                open: false,
            }),
        };

        // load the table
        catalog.create_table(&actual).unwrap();
        let expected = catalog.get_table("foo").unwrap();

        // assert round-trip equality
        assert_eq!(actual, *expected);
    }

    #[test]
    fn test_ddl_roundtrip() {
        let tbl1 = parse_table(DDL).unwrap();
        let tbl2 = parse_table(&tbl1.to_string()).unwrap();
        assert_eq!(tbl1, tbl2)
    }

    #[test]
    fn test_to_sqlite_ddl() {
        let tbl = parse_table(DDL).unwrap();
        let sql = tbl.to_sqlite_ddl();
        println!("{}", sql);
    }

    #[test]
    fn test_to_sqlite_select() {
        let tbl = parse_table(DDL).unwrap();
        let sql = tbl.to_sqlite_select();
        println!("{}", sql);
    }

    #[test]
    fn test_to_sqlite_insert() {
        let tbl = parse_table(DDL).unwrap();
        let sql = tbl.to_sqlite_insert();
        println!("{}", sql);
    }
}
