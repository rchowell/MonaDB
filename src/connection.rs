use core::str;
use std::{
    cell::RefCell, fmt::{self, Debug, Formatter}, path::Path
};

use bytes::Bytes;

use crate::{
    cask::Cask, cursor::Cursor, error::Error, ir::Table, lexer::RqlLexer, parser::RqlParser, value::Value, Result
};

/// Database Connection
pub struct Connection {
    cask: RefCell<Cask>
}

impl Connection {
    /// Create a cask connection
    pub fn new<P>(path: P) -> Result<Connection>
    where
        P: AsRef<Path>,
    {
        let cask = Cask::new(path)?;
        let conn = Connection { cask: cask.into() };
        Ok(conn)
    }

    /// Load a cask connection.
    pub fn open<P>(path: P) -> Result<Connection>
    where
        P: AsRef<Path>,
    {
        let cask = Cask::open(path)?;
        let conn = Connection { cask: cask.into() };
        Ok(conn)
    }

    /// Load the connection from an in-memory database.
    pub fn memory() -> Result<Connection> {
        todo!("memory")
    }

    /// Lookup a table by name.
    pub fn get_table(&self, name: &str) -> Result<Table> {
        let key = Bytes::copy_from_slice(name.as_bytes());
        let val = self.cask.borrow_mut().get(0, key)?.unwrap();
        let ddl = std::str::from_utf8(&val).unwrap();
        parse_table(ddl)
    }

    /// Create a table in the connection.
    pub fn create_table(&mut self, table: &Table) -> Result<()> {
        let bin = 0;
        let key: Bytes = Bytes::copy_from_slice(table.name.as_bytes());
        let val: Bytes = Bytes::copy_from_slice(table.to_string().as_bytes());
        self.cask.borrow_mut().put(bin, key, val)
    }

    /// Delete all rows from the table.
    pub fn clear(&mut self, _table: &str) -> Result<()> {
        todo!("clear")
    }

    /// Drop a table from the connection.
    pub fn drop_table(&mut self, _table: &str) -> Result<()> {
        todo!("drop_table")
    }

    /// Insert a value into the table.
    pub fn insert(&mut self, _table: &str, _value: Value) -> Result<()> {
        todo!("insert")
    }

    /// TODO `insert_batch` which prepares then calls execute many times in a transaction.
    pub fn insert_batch(&mut self, _table: &str, _values: &[Value]) -> Result<()> {
        todo!("insert_batch")
    }

    /// Open a cursor to the given table.
    pub fn open_cursor(&mut self, _table: &str) -> Result<Cursor> {
        todo!("cursor")
    }

    /// Begin a (goofy) transaction (really just a batch of SQL statements).
    pub fn transaction(&mut self) {
        todo!("transaction")
    }

    /// Commit the current transaction.
    pub fn commit(&mut self) -> Result<()> {
        todo!("commit")
    }
}

impl Debug for Connection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "\nCatalog")?;
        writeln!(f, "-------\n")?;
        // for table in self.tables.values() {
        //     writeln!(f, "{}", table)?;
        // }
        Ok(())
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
        Err(Error::InternalError(
            "Expected `create table` statement".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{TMember, TObject, Type};

    const DDL: &str = "create table example ({ x: number, y: number|null });";

    #[test]
    fn test_create_table() {
        let mut connection = Connection::memory().unwrap();
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
        connection.create_table(&actual).unwrap();
        let expected = connection.get_table("foo").unwrap();

        // assert round-trip equality
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_ddl_roundtrip() {
        let tbl1 = parse_table(DDL).unwrap();
        let tbl2 = parse_table(&tbl1.to_string()).unwrap();
        assert_eq!(tbl1, tbl2)
    }
}
