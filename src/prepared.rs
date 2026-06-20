//! Prepared statements: parse/compile caching for repeated execution.
//!
//! Parameter-free statements are fully compiled at prepare time; parameterized
//! statements cache the parsed AST and re-bind on each execute.

use crate::MonaDB;
use crate::error::{Error, Result};
use crate::ir::Statement;
use crate::value::Params;
use crate::visitor::visit::{Visit, visit_expr};
use crate::vm::{Program, Rows, VM};

/// Returns true when the statement contains any `?`/`$N`/`$name` placeholder.
pub fn has_params(stmt: &Statement) -> bool {
    let mut scan = ParamScan(false);
    scan.visit_statement(stmt);
    scan.0
}

/// A prepared statement — either a compiled program or a parsed AST awaiting bind.
#[derive(Debug)]
pub struct PreparedStatement {
    sql: String,
    kind: PreparedKind,
}

#[derive(Debug)]
enum PreparedKind {
    /// Parameter-free: bound and compiled at prepare time.
    Compiled {
        program: Program,
        catalog_generation: u64,
    },
    /// Parameterized: parsed AST retained; bind + compile run on each execute.
    Parsed {
        stmt: Statement,
    },
}

impl PreparedStatement {
    /// Returns the original SQL text (for error messages).
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

impl MonaDB {
    /// Parses `sql` and caches whatever is safe to reuse across executions.
    pub fn prepare(&self, sql: &str) -> Result<PreparedStatement> {
        let stmt = Self::parse(sql)?;
        let kind = if has_params(&stmt) {
            PreparedKind::Parsed { stmt }
        } else {
            let mut bound = stmt;
            self.bind(&mut bound, &Params::none())?;
            let program = self.compile(bound)?;
            PreparedKind::Compiled {
                program,
                catalog_generation: self.catalog_generation(),
            }
        };
        Ok(PreparedStatement {
            sql: sql.to_string(),
            kind,
        })
    }

    /// Runs a prepared statement with bound `params`, returning a lazy row iterator.
    pub fn execute_prepared(
        &mut self,
        stmt: &PreparedStatement,
        params: &Params,
        debug: bool,
    ) -> Result<Rows> {
        match &stmt.kind {
            PreparedKind::Compiled {
                program,
                catalog_generation,
            } => {
                if self.catalog_generation() != *catalog_generation {
                    return Err(Error::StalePreparedStatement);
                }
                if debug {
                    Self::debug(program);
                }
                Ok(Rows::new(VM::init(
                    self.storage.clone(),
                    self.catalog_generation.clone(),
                    program.clone(),
                )))
            }
            PreparedKind::Parsed { stmt } => {
                let mut bound = stmt.clone();
                self.bind(&mut bound, params)?;
                let program = self.compile(bound)?;
                if debug {
                    Self::debug(&program);
                }
                Ok(Rows::new(VM::init(
                    self.storage.clone(),
                    self.catalog_generation.clone(),
                    program,
                )))
            }
        }
    }
}

/// A `Visit` that trips on the first `Expr::Param`.
struct ParamScan(bool);

impl<'ast> Visit<'ast> for ParamScan {
    fn visit_expr(&mut self, e: &'ast crate::ir::Expr) {
        if self.0 {
            return;
        }
        if matches!(e, crate::ir::Expr::Param(_)) {
            self.0 = true;
            return;
        }
        visit_expr(self, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn has_params_detects_placeholders() {
        let stmt = MonaDB::parse("select ?;").unwrap();
        assert!(has_params(&stmt));
        let stmt = MonaDB::parse("select 1;").unwrap();
        assert!(!has_params(&stmt));
    }

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
    fn parsed_prepare_binds_different_params() {
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
