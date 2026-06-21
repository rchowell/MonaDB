//! MonaDB — an embedded database with a small SQL dialect compiled to bytecode.
//!
//! A statement flows through a fixed pipeline:
//!
//!   SQL text ─▶ lexer ─▶ parser ─▶ IR ─▶ binder ─▶ compiler ─▶ Vop ─▶ VM ─▶ LMDB
//!
//! [`MonaDB`] is the public handle; `query` and `execute` drive that pipeline.

pub mod error;
pub mod ir;
/// Order-preserving key encoding. Public so the order/round-trip invariants its
/// doc comments state can be exercised by the property-based conformance tests.
pub mod schema;

mod binder;
mod catalog;
mod compiler;
mod cursor;
mod display;
mod functions;
pub mod highlight;
pub mod lexer;
mod prepared;
mod read;
mod storage;
mod transaction;
mod value;
mod visitor;
mod vm;

/// Python bindings (pyo3). Compiled only with `--features python`; all pyo3
/// code is isolated here so the default build stays Python-free.
#[cfg(feature = "python")]
mod python;

// lalrpop module
lalrpop_mod!(
    #[allow(clippy::all)]
    #[rustfmt::skip]
    pub parser
);

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use compiler::Compiler;
use error::{Error, Result};
use lalrpop_util::lalrpop_mod;
use storage::Storage;
use tempfile::TempDir;

use crate::{
    binder::Binder,
    catalog::Catalog,
    ir::Statement,
    parser::SqlParser,
    vm::{Program, Rows},
};

pub use crate::lexer::{SqlLexer, Token};
pub use crate::prepared::PreparedStatement;
pub use crate::value::{Params, Value};

/// Upper bound on cached query plans; the cache is cleared wholesale on overflow.
const PLAN_CACHE_CAP: usize = 256;

/// The user-facing database handle.
pub struct MonaDB {
    /// The storage engine over LMDB.
    storage: Storage,
    /// The catalog reference for semantic analysis.
    catalog: Catalog,
    /// Incremented when CREATE/DROP changes catalog membership; compiled prepares
    /// snapshot this to detect staleness.
    catalog_generation: Rc<Cell<u64>>,
    /// Auto-parameterizing plan cache: a literal-normalized SQL template →
    /// its prepared statement. Lets repeated query *shapes* (the same SQL with
    /// different literals) skip the lexer/parser, the way a real engine caches
    /// plans. Keyed by [`MonaDB::normalize`]'s template.
    plan_cache: Rc<RefCell<HashMap<String, PreparedStatement>>>,
}

impl MonaDB {
    /// Open or create a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<MonaDB> {
        let storage = Storage::open(path)?;
        let catalog = Catalog::load(&storage)?;
        Ok(MonaDB {
            storage,
            catalog,
            catalog_generation: Rc::new(Cell::new(0)),
            plan_cache: Rc::new(RefCell::new(HashMap::new())),
        })
    }

    /// Open an in-memory database.
    pub fn memory() -> Result<MonaDB> {
        let tmp_dir = TempDir::new()?;
        let tmp_pth = tmp_dir.path().join("memory.db");
        Self::open(&tmp_pth)
    }

    /// Run a query, returning a lazy iterator over its result rows. The
    /// statement's transaction commits once the iterator is exhausted.
    /// Mirrors rusqlite's `Connection::query`.
    ///
    /// Ad-hoc SQL is run through the auto-parameterizing plan cache: literals are
    /// normalized to a `?`-templated key (see [`MonaDB::normalize`]) so repeated
    /// query *shapes* reuse a prepared statement and skip re-parsing. Anything
    /// that doesn't parameterize cleanly falls back to a direct parse+compile.
    pub fn query(&mut self, sql: &str, debug: bool) -> Result<Rows> {
        // A lex error (or nothing to normalize) → let the direct path surface it.
        let Some((key, vals)) = Self::normalize(sql) else {
            return self.query_with(sql, &Params::none(), debug);
        };
        let params = Params::positional(vals);
        // `cache` is a detached `Rc` handle, so a borrow held across the
        // `&mut self` execute call below does not alias `self`.
        let cache = self.plan_cache.clone();

        // Fast path: reuse a cached plan for this template.
        {
            let guard = cache.borrow();
            if let Some(stmt) = guard.get(&key) {
                match self.execute_prepared(stmt, &params, debug) {
                    // A CREATE/DROP invalidated a cached plan — evict and rebuild.
                    Err(Error::StalePreparedStatement) => {
                        drop(guard);
                        cache.borrow_mut().remove(&key);
                    }
                    other => return other,
                }
            }
        }

        // Miss: prepare the normalized template. If the `?`-substituted form
        // doesn't parse (a literal sat in a non-expr position), fall back to the
        // concrete SQL — which behaves exactly as before.
        let stmt = match self.prepare(&key) {
            Ok(stmt) => stmt,
            Err(_) => return self.query_with(sql, &Params::none(), debug),
        };
        let rows = self.execute_prepared(&stmt, &params, debug);
        if rows.is_ok() {
            let mut map = cache.borrow_mut();
            if map.len() >= PLAN_CACHE_CAP {
                map.clear();
            }
            map.insert(key, stmt);
        }
        rows
    }

    /// Normalizes `sql` into an auto-parameterized template: every *numeric*
    /// literal token is replaced by a `?` placeholder and its [`Value`] collected,
    /// in source order. Inter-token text — and string literals — are copied
    /// verbatim, so the template is valid SQL whose `?`s bind back to the
    /// extracted values (matching the grammar's `number` → `Value::number` action).
    ///
    /// Only numbers are parameterized: a `?` in a numeric *expression* position
    /// substitutes to an identical `Expr::Lit`, while a number in a non-expression
    /// position (`limit`, a selector index) fails to parse and falls back to the
    /// direct path. **String literals are left intact** because a string in a
    /// `FROM` source is lowered to a file scan based on the *literal* at parse
    /// time (see `looks_like_file`), which a `?` would silently defeat.
    ///
    /// Returns `None` on a lexer error, leaving the caller to surface it via the
    /// direct path.
    fn normalize(sql: &str) -> Option<(String, Vec<Value>)> {
        let mut key = String::with_capacity(sql.len());
        let mut vals = Vec::new();
        let mut last = 0;
        for item in SqlLexer::new(sql) {
            let (start, token, end) = item.ok()?;
            key.push_str(&sql[last..start]);
            match token {
                Token::Number(s) => {
                    vals.push(Value::number(&s));
                    key.push('?');
                }
                _ => key.push_str(&sql[start..end]),
            }
            last = end;
        }
        key.push_str(&sql[last..]);
        Some((key, vals))
    }

    /// Run a parameterized query, binding `?`/`$N`/`$name` placeholders against
    /// `params` before compilation. `query` is the no-parameter form.
    pub fn query_with(&mut self, sql: &str, params: &Params, debug: bool) -> Result<Rows> {
        let stmt = self.prepare(sql)?;
        self.execute_prepared(&stmt, params, debug)
    }

    /// Run a statement to completion, committing it, and return the number of
    /// rows produced. Mirrors rusqlite's `Connection::execute`.
    pub fn execute(&mut self, sql: &str) -> Result<u64> {
        self.query(sql, false)?.finish()
    }

    /// Run a parameterized statement to completion, returning its row count.
    pub fn execute_with(&mut self, sql: &str, params: &Params) -> Result<u64> {
        self.query_with(sql, params, false)?.finish()
    }

    /// Returns the current catalog generation (for stale-prepare detection).
    pub(crate) fn catalog_generation(&self) -> u64 {
        self.catalog_generation.get()
    }

    /// Phase 1: Parse input string into our AST (no binding or compilation).
    pub fn parse(sql: &str) -> Result<Statement> {
        let l = crate::lexer::SqlLexer::new(sql);
        let p = SqlParser::new();
        // Counts positional `?` placeholders, numbering them in source order.
        let param_pos = std::cell::Cell::new(0);
        Ok(p.parse(&param_pos, l)?)
    }

    /// Phase 2: Bind all tables and variable references in the AST. Parameter
    /// placeholders are left as runtime slots (resolved at execute time), so a
    /// statement binds and compiles once regardless of its parameter values.
    ///
    /// The binder opens a read transaction lazily — only if a catalog lookup
    /// misses the in-memory cache — so a warm bind touches no transaction.
    fn bind(&self, statement: &mut Statement) -> Result<()> {
        let mut binder = Binder::new(
            self.catalog.clone(),
            self.storage.clone(),
            self.catalog_generation(),
        );
        binder.bind(statement)
    }

    /// Phase 3: Compilation is pure bytecode generation.
    #[allow(clippy::unused_self)]
    fn compile(&self, statement: Statement) -> Result<Program> {
        let cc = Compiler::new();
        cc.compile(statement)
    }

    /// Prints a program's bytecode as an address/operation table (a debug aid).
    fn debug(program: &Program) {
        println!();
        println!("addr\toperation");
        println!("----\t---------");
        for (addr, op) in program.instructions.iter().enumerate() {
            println!("{addr:04}\t{op:?}");
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_select_bytecode() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = MonaDB::open(&db_path).unwrap();

        db.execute("create table t (id int);").unwrap();
        // let _ = db.query("select * from t;", true);
    }

    #[test]
    fn normalize_templates_numeric_literals() {
        // Varying numeric literals collapse to one template; the literals are
        // extracted in source order.
        let (k1, v1) = MonaDB::normalize("select docs[12345];").unwrap();
        let (k2, v2) = MonaDB::normalize("select docs[67890];").unwrap();
        assert_eq!(k1, k2, "varying numeric literals must share one template");
        assert_eq!(k1, "select docs[?];");
        assert_eq!(v1, vec![Value::int(12345)]);
        assert_eq!(v2, vec![Value::int(67890)]);
    }

    #[test]
    fn normalize_keeps_strings_verbatim() {
        // Strings are NOT parameterized (a string in a FROM source is lowered to
        // a file scan at parse time); only the numeric part of a composite key is.
        let (k, v) = MonaDB::normalize(r#"select docs["t042", 5700];"#).unwrap();
        assert_eq!(k, r#"select docs["t042", ?];"#);
        assert_eq!(v, vec![Value::int(5700)]);
    }

    #[test]
    fn normalize_no_literals_is_identity() {
        let (k, v) = MonaDB::normalize("select * from t;").unwrap();
        assert_eq!(k, "select * from t;");
        assert!(v.is_empty());
    }

    #[test]
    fn plan_cache_reuses_template_across_literals() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        db.execute(r#"insert into t ({"id": 2});"#).unwrap();

        // Two different literals, one template — each must still fetch its own row.
        let mut r1 = db.query("select t[1];", false).unwrap();
        assert_eq!(r1.next().unwrap().unwrap().jpk("id"), Some(Value::int(1)));
        let mut r2 = db.query("select t[2];", false).unwrap();
        assert_eq!(r2.next().unwrap().unwrap().jpk("id"), Some(Value::int(2)));

        // The shared template was cached and reused (not one entry per literal).
        assert!(db.plan_cache.borrow().contains_key("select t[?];"));
    }

    #[test]
    fn catalog_cache_invalidates_on_drop_and_recreate() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        // Bind a reference to `t`, populating the catalog cache.
        db.execute("select t[1];").unwrap();

        // Dropping bumps the generation; a stale cache would still resolve `t`,
        // so the now-unknown `t` must surface as an error (not a stale row).
        db.execute("drop table t;").unwrap();
        assert!(
            db.query("select t[1];", false).is_err(),
            "dropped table must not resolve from a stale catalog cache"
        );

        // Recreating with a different shape must be visible (cache re-scanned).
        db.execute("create table t (name string);").unwrap();
        db.execute(r#"insert into t ({"name": "x"});"#).unwrap();
        let mut rows = db.query(r#"select t["x"];"#, false).unwrap();
        assert_eq!(
            rows.next().unwrap().unwrap().jpk("name"),
            Some(Value::String(std::rc::Rc::from("x")))
        );
    }

    #[test]
    fn ctas_from_csv() {
        let sql = "create table people as select * from 'tests/fixtures/people.csv';";
        let mut db = MonaDB::memory().unwrap();
        db.execute(sql).unwrap();
        let mut rows = db
            .query("select * from people as r order by r.name;", false)
            .unwrap();
        let mut n = 0;
        while rows.next().unwrap().is_some() {
            n += 1;
        }
        assert_eq!(n, 2);
    }
}
