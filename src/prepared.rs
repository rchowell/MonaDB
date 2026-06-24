//! Prepared statements: parse/bind/compile caching for repeated execution.
//!
//! Every statement — parameter-free or not — is bound and compiled once at
//! prepare time. Parameter placeholders compile to runtime slots (`Vop::LoadParam`),
//! so the same program serves any bound values: `execute_prepared` just hands the
//! `Params` to the VM.

use std::rc::Rc;

use crate::MonaDB;
use crate::error::{Error, Result};
use crate::ir::Param;
use crate::value::Params;
use crate::vm::{Program, Rows, VM, Vop};

/// A prepared statement: a compiled program plus the catalog generation it was
/// bound against (to detect a CREATE/DROP that would invalidate it). The program
/// is `Rc`-shared so re-executing a cached plan (or caching a clone) is a
/// refcount bump, not a copy.
#[derive(Debug, Clone)]
pub struct PreparedStatement {
    sql: String,
    program: Rc<Program>,
    catalog_generation: u64,
    /// The parameters the program reads (`Vop::LoadParam`), collected at prepare
    /// time so `execute_prepared` can reject a missing binding up front — before
    /// the VM runs and emits rows or side effects — rather than mid-iteration.
    required_params: Vec<Param>,
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
        let required_params = program
            .instructions
            .iter()
            .filter_map(|op| match op {
                Vop::LoadParam(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        Ok(PreparedStatement {
            sql: sql.to_string(),
            program: Rc::new(program),
            catalog_generation: self.catalog_generation(),
            required_params,
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
        // Fail fast on a missing binding, before the VM runs — so a half-executed
        // statement can't emit rows or side effects ahead of the error.
        for p in &stmt.required_params {
            let bound = match p {
                Param::Numbered(n) => params.get_numbered(*n).is_some(),
                Param::Named(name) => params.get_named(name).is_some(),
            };
            if !bound {
                return Err(Error::BindError(format!("missing parameter {p}")));
            }
        }
        if debug {
            Self::debug(&stmt.program);
        }
        let defer_commit = self.in_transaction();
        Ok(Rows::new(VM::init(
            self.storage.clone(),
            self.catalog_generation.clone(),
            self.session_catalog_dirty.clone(),
            stmt.program.clone(),
            params.clone(),
            self.session_txn.clone(),
            defer_commit,
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
    fn missing_param_fails_before_execution() {
        // A missing binding is rejected up front by execute_prepared (before the
        // VM runs), not deferred to mid-iteration.
        let mut db = MonaDB::memory().unwrap();
        let stmt = db.prepare("select $1;").unwrap();
        let err = db.execute_prepared(&stmt, &Params::none(), false);
        assert!(matches!(err, Err(Error::BindError(_))));
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
