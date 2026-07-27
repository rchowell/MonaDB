//! Prepared statements: parse/bind/compile caching for repeated execution.
//!
//! Every statement — parameter-free or not — is bound and compiled once at
//! prepare time. Parameter placeholders compile to runtime slots (`Vop::LoadParam`),
//! so the same program serves any bound values: execution hands [`Params`] to the VM.

use std::rc::Rc;

use crate::MonaDB;
use crate::ast::Param;
use crate::error::{Error, Result};
use crate::params::{IntoParams, Params};
use crate::vm::{Program, Rows, VM, Vop};

/// A compiled, cacheable statement plan: program bytecode plus staleness metadata.
///
/// `Rc`-shared so the plan cache and [`Statement`] handles reuse one allocation.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    /// The catalog version this was compiled against.
    version: u64,
    /// The original sql string
    sql: String,
    /// The compiled program
    program: Rc<Program>,
    /// Distinct parameters the program reads (`Vop::LoadParam`), collected at
    /// prepare time so execution can reject a missing binding before the VM runs.
    params: Vec<Param>,
}

impl Plan {
    /// Returns the original SQL text (for error messages).
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

/// A prepared statement bound to a [`MonaDB`] handle for execution.
///
/// Mirrors rusqlite/duckdb `Statement<'_>`: `query` and `execute` take only
/// parameters because this type already holds the database reference.
pub struct Statement<'conn> {
    conn: &'conn mut MonaDB,
    plan: Rc<Plan>,
}

impl Statement<'_> {
    /// Returns the original SQL text (for error messages).
    pub fn sql(&self) -> &str {
        self.plan.sql()
    }

    /// Returns the number of distinct parameters the program binds.
    pub fn parameter_count(&self) -> usize {
        self.plan.params.len()
    }

    /// Runs the statement with bound `params`, returning a lazy row iterator.
    pub fn query(&mut self, params: impl IntoParams) -> Result<Rows> {
        self.conn
            .execute_plan(&self.plan, &params.into_params())
    }

    /// Runs the statement to completion and returns the row count.
    pub fn execute(&mut self, params: impl IntoParams) -> Result<u64> {
        self.query(params)?.finish()
    }
}

impl MonaDB {
    /// Parses, binds, and compiles `sql` into a reusable [`Statement`].
    pub fn prepare<'a>(&'a mut self, sql: &str) -> Result<Statement<'a>> {
        let plan = Rc::new(self.compile_plan(sql)?);
        Ok(Statement { conn: self, plan })
    }

    /// Returns a cached plan for `sql` when present, otherwise compiles and caches it.
    pub fn prepare_cached<'a>(&'a mut self, sql: &str) -> Result<Statement<'a>> {
        let plan = self.cached_plan(sql)?;
        Ok(Statement { conn: self, plan })
    }

    /// Returns the shared plan for `sql` from the cache, compiling and caching it
    /// on a miss. The plan-fetching core of [`prepare_cached`], shared with the
    /// Python binding so it reuses the connection's cache too. Keyed by the raw
    /// SQL text, consistent with [`MonaDB::query`] / [`MonaDB::query_with`].
    pub(crate) fn cached_plan(&mut self, sql: &str) -> Result<Rc<Plan>> {
        let cache = self.cache.clone();
        if let Some(plan) = cache.borrow_mut().get(sql) {
            return Ok(plan);
        }
        Ok(cache.borrow_mut().put(sql, self.compile_plan(sql)?))
    }

    /// Parses, binds, compiles, and resolves `sql` into a shareable plan (no
    /// execution).
    pub(crate) fn compile_plan(&self, sql: &str) -> Result<Plan> {
        let mut bound = Self::parse(sql)?;
        self.bind(&mut bound)?;
        let mut program = self.compile(bound)?;
        self.resolve_tables(&mut program)?;
        // Distinct placeholders, in first-seen order: a parameter referenced N
        // times compiles to N `LoadParam` ops but counts once (matching rusqlite).
        let mut required_params: Vec<Param> = Vec::new();
        for op in &program.instructions {
            if let Vop::LoadParam(p) = op
                && !required_params.contains(p) {
                    required_params.push(p.clone());
                }
        }
        Ok(Plan {
            version: self.catalog_version(),
            sql: sql.to_string(),
            program: Rc::new(program),
            params: required_params,
        })
    }

    /// Resolves the program's table handles into `program.tables` and rewrites
    /// each `Open.tbl` from the compiler-emitted table oid to its slot index, so
    /// dispatch is a plain array read (see [`Vop::Open`]). Deduplicates oids so
    /// repeated opens of one table share a slot.
    ///
    /// Handles are read through the session txn when a session is open — like the
    /// binder's `resolve_table`, it sees the session's own uncommitted DDL, so an
    /// in-session CREATE'd table resolves here and no second transaction is opened.
    /// Otherwise they are resolved in a committed **write** txn: a resolved handle
    /// outlives this call (it is reused across later execution txns), and LMDB only
    /// registers a named dbi into the shared env when the opening txn commits — a
    /// dbi first opened in an aborting read txn is unusable by later txns. After a
    /// reopen no write txn has touched a table's btree in the new env instance, so a
    /// read-txn resolve would hand back an unregistered handle that `EINVAL`s on use.
    /// Every `Open` targets a table the binder resolved, and CREATE makes a table's
    /// btree atomically with its catalog row, so a missing handle is a genuine error.
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
                // Commit the dbi opens so the handles survive into later
                // execution txns (see the doc comment); idempotent for tables
                // already registered in this env instance.
                let txn = self.storage.write_txn()?;
                let tables = oids
                    .iter()
                    .map(|&oid| self.storage.open_btree(&txn, oid))
                    .collect::<Result<_>>()?;
                txn.commit()?;
                tables
            }
        };
        Ok(())
    }

    /// Runs a compiled plan with bound `params`, returning a lazy row iterator.
    pub(crate) fn execute_plan(
        &mut self,
        plan: &Plan,
        params: &Params,
    ) -> Result<Rows> {
        if self.catalog_version() != plan.version {
            return Err(Error::StalePreparedStatement);
        }
        for p in &plan.params {
            let bound = match p {
                Param::Numbered(n) => params.get_numbered(*n).is_some(),
                Param::Named(name) => params.get_named(name).is_some(),
            };
            if !bound {
                return Err(Error::BindError(format!("missing parameter {p}")));
            }
        }
        if self.options.debug_enabled() {
            Self::trace_program(&plan.program);
        }
        let defer_commit = self.in_transaction();
        Ok(Rows::new(VM::init(
            self.storage.clone(),
            self.catalog_version.clone(),
            self.session_catalog_dirty.clone(),
            plan.program.clone(),
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

        let mut stmt = db.prepare("select * from t;").unwrap();
        assert!(stmt.query(()).unwrap().next().unwrap().is_some());
        assert!(stmt.query(()).unwrap().next().unwrap().is_some());
    }

    #[test]
    fn prepared_param_reuses_program_across_values() {
        let mut db = MonaDB::memory().unwrap();
        let mut stmt = db.prepare("select ?;").unwrap();

        let mut rows = stmt.query([Value::int(1)]).unwrap();
        assert_eq!(rows.next().unwrap().unwrap(), Value::int(1));

        let mut rows = stmt.query([Value::int(2)]).unwrap();
        assert_eq!(rows.next().unwrap().unwrap(), Value::int(2));
    }

    #[test]
    fn missing_param_fails_before_execution() {
        let mut db = MonaDB::memory().unwrap();
        let mut stmt = db.prepare("select $1;").unwrap();
        let err = stmt.query(());
        assert!(matches!(err, Err(Error::BindError(_))));
    }

    #[test]
    fn stale_compiled_after_drop() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t;").unwrap();
        {
            db.prepare_cached("select * from t;")
                .unwrap()
                .query(())
                .unwrap();
        }
        db.execute("drop table t;").unwrap();
        let mut stmt = db.prepare_cached("select * from t;").unwrap();
        let err = match stmt.query(()) {
            Ok(_) => panic!("expected stale prepared statement error"),
            Err(e) => e,
        };
        assert_eq!(err, Error::StalePreparedStatement);
    }

    #[test]
    fn reprepare_after_drop_recreate_binds_live_handle() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();

        {
            db.prepare_cached("select t[?];")
                .unwrap()
                .query([Value::int(1)])
                .unwrap()
                .next()
                .unwrap();
        }

        db.execute("drop table t;").unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();

        let mut stmt = db.prepare_cached("select t[?];").unwrap();
        assert_eq!(
            stmt.query([Value::int(1)]).err(),
            Some(Error::StalePreparedStatement),
        );

        drop(stmt);
        let mut stmt2 = db.prepare("select t[?];").unwrap();
        assert!(stmt2.query([Value::int(1)]).unwrap().next().unwrap().is_some());
    }

    #[test]
    fn resolves_handle_for_table_created_in_open_txn() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("begin;").unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 7});"#).unwrap();

        assert!(db.query("select t[7];").unwrap().next().unwrap().is_some());

        db.execute("commit;").unwrap();
    }

    #[test]
    fn query_with_equivalent_to_prepare_execute() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t;").unwrap();
        db.execute(r#"insert into t ({"x": 1});"#).unwrap();

        let sql = "select * from t;";
        let direct_row = db.query(sql).unwrap().next().unwrap().unwrap();

        let mut stmt = db.prepare(sql).unwrap();
        let prepared_row = stmt.query(()).unwrap().next().unwrap().unwrap();

        assert_eq!(direct_row, prepared_row);
    }

    #[test]
    fn prepare_cached_reuses_entry() {
        let mut db = MonaDB::memory().unwrap();
        {
            let mut s1 = db.prepare_cached("select ?;").unwrap();
            assert_eq!(
                s1.query([1i64]).unwrap().next().unwrap().unwrap(),
                Value::int(1)
            );
        }
        {
            let mut s2 = db.prepare_cached("select ?;").unwrap();
            assert_eq!(
                s2.query([2i64]).unwrap().next().unwrap().unwrap(),
                Value::int(2)
            );
        }
    }
}
