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
        let mut program = self.compile(bound)?;
        self.resolve_tables(&mut program)?;
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

    /// Resolves the program's table handles into `program.tables` and rewrites
    /// each `Open.tbl` from the compiler-emitted table oid to its slot index, so
    /// dispatch is a plain array read (see [`Vop::Open`]). Deduplicates oids so
    /// repeated opens of one table share a slot.
    ///
    /// Handles are read through the session txn when a session is open — like the
    /// binder's `resolve_table`, it sees the session's own uncommitted DDL, so an
    /// in-session CREATEd table resolves here and no second transaction is opened —
    /// otherwise through a throwaway read txn. Every `Open` targets a table the
    /// binder resolved, and CREATE makes a table's btree atomically with its
    /// catalog row, so a missing handle is a genuine error, surfaced here.
    fn resolve_tables(&self, program: &mut Program) -> Result<()> {
        let mut oids: Vec<u32> = Vec::new();
        for op in &mut program.instructions {
            if let Vop::Open { tbl, .. } = op {
                let oid = *tbl;
                let slot = match oids.iter().position(|&o| o == oid) {
                    Some(slot) => slot,
                    None => {
                        oids.push(oid);
                        oids.len() - 1
                    }
                };
                *tbl = u32::try_from(slot).expect("table slot fits in u32");
            }
        }
        let session = self.session_txn.borrow();
        program.tables = match session.as_ref() {
            Some(txn) => oids
                .iter()
                .map(|&oid| self.storage.open_btree(txn, oid))
                .collect::<Result<_>>()?,
            None => {
                let txn = self.storage.read_txn()?;
                oids.iter()
                    .map(|&oid| self.storage.open_btree(&txn, oid))
                    .collect::<Result<_>>()?
            }
        };
        Ok(())
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
    fn reprepare_after_drop_recreate_binds_live_handle() {
        // A prepared plan resolves the table's btree handle at prepare time. After
        // a DROP + CREATE the old plan is stale (its handle could point at a
        // cleared or reused dbi); a re-prepare must bind the *recreated* table's
        // handle, and the new keyed lookup must read the recreated row.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();

        let stmt = db.prepare("select t[?];").unwrap();
        assert!(
            db.execute_prepared(&stmt, &Params::positional(vec![Value::int(1)]), false)
                .unwrap()
                .next()
                .unwrap()
                .is_some()
        );

        db.execute("drop table t;").unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();

        // The old plan is invalidated by the catalog-generation bump.
        assert_eq!(
            db.execute_prepared(&stmt, &Params::positional(vec![Value::int(1)]), false)
                .err(),
            Some(Error::StalePreparedStatement),
        );

        // A fresh prepare resolves the live handle and reads the recreated row.
        let stmt2 = db.prepare("select t[?];").unwrap();
        assert!(
            db.execute_prepared(&stmt2, &Params::positional(vec![Value::int(1)]), false)
                .unwrap()
                .next()
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn resolves_handle_for_table_created_in_open_txn() {
        // A table created inside an open session is visible only through the
        // session's own write txn. `prepare` reuses that txn (rather than opening
        // a separate read snapshot, which couldn't see the uncommitted DDL), so
        // the handle resolves at prepare time and the keyed lookup reads the row.
        let mut db = MonaDB::memory().unwrap();
        db.execute("begin;").unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 7});"#).unwrap();

        assert!(db.query("select t[7];", false).unwrap().next().unwrap().is_some());

        db.execute("commit;").unwrap();
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
