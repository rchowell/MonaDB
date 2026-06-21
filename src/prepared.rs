//! Prepared statements: parse/bind/compile caching for repeated execution.
//!
//! Every statement — parameter-free or not — is bound and compiled once at
//! prepare time. Parameter placeholders compile to runtime slots (`Vop::LoadParam`),
//! so the same program serves any bound values: `execute_prepared` just hands the
//! `Params` to the VM.

use std::rc::Rc;

use crate::MonaDB;
use crate::error::{Error, Result};
use crate::value::Params;
use crate::vm::{Program, Rows, VM};

/// A prepared statement: a compiled program plus the catalog generation it was
/// bound against (to detect a CREATE/DROP that would invalidate it). The program
/// is `Rc`-shared so re-executing a cached plan is a refcount bump, not a copy.
#[derive(Debug)]
pub struct PreparedStatement {
    sql: String,
    program: Rc<Program>,
    catalog_generation: u64,
}

impl PreparedStatement {
    /// Returns the original SQL text (for error messages).
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

impl MonaDB {
    /// Parses, binds, and compiles `sql` into a reusable program. Parameter
    /// placeholders become runtime slots, so the result is reusable across any
    /// bound values.
    pub fn prepare(&self, sql: &str) -> Result<PreparedStatement> {
        let mut bound = Self::parse(sql)?;
        self.bind(&mut bound)?;
        let program = self.compile(bound)?;
        Ok(PreparedStatement {
            sql: sql.to_string(),
            program: Rc::new(program),
            catalog_generation: self.catalog_generation(),
        })
    }

    /// Runs a prepared statement with bound `params`, returning a lazy row iterator.
    pub fn execute_prepared(
        &mut self,
        stmt: &PreparedStatement,
        params: &Params,
        debug: bool,
    ) -> Result<Rows> {
        if self.catalog_generation() != stmt.catalog_generation {
            return Err(Error::StalePreparedStatement);
        }
        if debug {
            Self::debug(&stmt.program);
        }
        Ok(Rows::new(VM::init(
            self.storage.clone(),
            self.catalog_generation.clone(),
            stmt.program.clone(),
            params.clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn compiled_prepare_reuses_program() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t;").unwrap();
        db.execute(r#"insert into t ({"x": 1});"#).unwrap();

        let stmt = db.prepare("select * from t;").unwrap();
        let mut rows1 = db.execute_prepared(&stmt, &Params::none(), false).unwrap();
        assert!(rows1.next().unwrap().is_some());

        let mut rows2 = db.execute_prepared(&stmt, &Params::none(), false).unwrap();
        assert!(rows2.next().unwrap().is_some());
    }

    #[test]
    fn prepared_param_reuses_program_across_values() {
        // One compiled program (runtime param slot) serves different bound values.
        let stmt = {
            let db = MonaDB::memory().unwrap();
            db.prepare("select ?;").unwrap()
        };
        let mut db = MonaDB::memory().unwrap();

        let mut rows = db
            .execute_prepared(&stmt, &Params::positional(vec![Value::int(1)]), false)
            .unwrap();
        assert_eq!(rows.next().unwrap().unwrap(), Value::int(1));

        let mut rows = db
            .execute_prepared(&stmt, &Params::positional(vec![Value::int(2)]), false)
            .unwrap();
        assert_eq!(rows.next().unwrap().unwrap(), Value::int(2));
    }

    #[test]
    fn stale_compiled_after_drop() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t;").unwrap();
        let stmt = db.prepare("select * from t;").unwrap();
        db.execute("drop table t;").unwrap();
        let err = match db.execute_prepared(&stmt, &Params::none(), false) {
            Ok(_) => panic!("expected stale prepared statement error"),
            Err(e) => e,
        };
        assert_eq!(err, Error::StalePreparedStatement);
    }

    #[test]
    fn query_with_equivalent_to_prepare_execute() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t;").unwrap();
        db.execute(r#"insert into t ({"x": 1});"#).unwrap();

        let sql = "select * from t;";
        let mut direct = db.query(sql, false).unwrap();
        let direct_row = direct.next().unwrap().unwrap();

        let stmt = db.prepare(sql).unwrap();
        let mut prepared = db
            .execute_prepared(&stmt, &Params::none(), false)
            .unwrap();
        let prepared_row = prepared.next().unwrap().unwrap();

        assert_eq!(direct_row, prepared_row);
    }
}
