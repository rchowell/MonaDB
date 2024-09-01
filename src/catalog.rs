use std::collections::HashMap;

use rusqlite::Connection;
use serde_json::map::Iter;
use crate::{table::Table, Result};

pub struct Catalog {
    tables: HashMap<String, Table>,
}

const CREATE_CATALOG: &str = "
    CREATE TABLE IF NOT EXISTS catalog (
        name TEXT PRIMARY KEY,
        rql TEXT
    );";

const SELECT_CATALOG: &str = "
    SELECT name, rql
    FROM catalog;";

impl Catalog {

    pub fn load(conn: &Connection) -> Result<Catalog> {
        // create catalog if not exists
        conn.execute(CREATE_CATALOG, [])?;
        // load tables into memory
        let mut stmt = conn.prepare(SELECT_CATALOG)?;
        let mut rows = stmt.query([])?;
        let mut tables = HashMap::new();
        while let Some(row) = rows.next()? {
            let table = Table {
                name: row.get(0)?,
                rql: row.get(1)?,
                cols: vec![],
            };
            tables.insert(table.name.clone(), table);
        }
        Ok(Catalog { tables })
    }

    pub fn describe(&self) {
        for (name, table) in &self.tables {
            println!("Table: {}", name);
            println!("RQL: {}", table.rql);
            println!();
        }
    }
}
