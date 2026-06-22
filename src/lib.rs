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
use std::collections::{HashMap, VecDeque};
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

/// Upper bound on cached query plans; the least-recently-used plan is evicted on overflow.
const PLAN_CACHE_CAP: usize = 256;

/// A bounded LRU cache of prepared plans, keyed by normalized SQL template.
///
/// Holding only the hottest `cap` plans, an LRU keeps the working set resident
/// instead of dropping every plan at once — a wholesale flush would thrash any
/// workload cycling through more than `cap` distinct query shapes.
struct PlanCache {
    plans: HashMap<String, PreparedStatement>,
    /// Keys in access order, least-recently-used at the front.
    order: VecDeque<String>,
    cap: usize,
}

impl PlanCache {
    fn new(cap: usize) -> Self {
        PlanCache { plans: HashMap::new(), order: VecDeque::new(), cap }
    }

    /// Returns a clone of the plan for `key` (cheap — the program is `Rc`-shared),
    /// marking it most-recently-used. Cloning lets the caller drop the cache
    /// borrow before executing under `&mut self`.
    fn get(&mut self, key: &str) -> Option<PreparedStatement> {
        let plan = self.plans.get(key)?.clone();
        self.detach_order(key);
        self.order.push_back(key.to_owned());
        Some(plan)
    }

    /// Inserts or replaces `key`'s plan as most-recently-used, evicting the LRU
    /// entry past `cap`. `order` and `plans` stay in lockstep, so a non-empty
    /// `plans` over `cap` always has an entry to evict.
    fn insert(&mut self, key: String, plan: PreparedStatement) {
        self.detach_order(&key);
        self.plans.insert(key.clone(), plan);
        self.order.push_back(key); // reuse the owned key — no extra clone
        while self.plans.len() > self.cap {
            if let Some(lru) = self.order.pop_front() {
                self.plans.remove(&lru);
            }
        }
    }

    /// Evicts `key` (a plan invalidated by a catalog change).
    fn remove(&mut self, key: &str) {
        self.plans.remove(key);
        self.detach_order(key);
    }

    /// Drops `key`'s entry from the access order, if present.
    fn detach_order(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
    }
}

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
    /// plans. Keyed by [`MonaDB::normalize`]'s template (or the raw SQL, for the
    /// parameterized [`MonaDB::query_with`] path).
    plan_cache: Rc<RefCell<PlanCache>>,
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
            plan_cache: Rc::new(RefCell::new(PlanCache::new(PLAN_CACHE_CAP))),
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
        // A lex error, or SQL with an explicit placeholder, isn't auto-templated:
        // run it directly (uncached).
        let Some((key, vals)) = Self::normalize(sql) else {
            return self.run_uncached(sql, &Params::none(), debug);
        };
        let params = Params::positional(vals);
        match self.run_cached(&key, &key, &params, debug) {
            // The template didn't prepare (e.g. a literal in a non-expr position);
            // fall back to the concrete SQL, uncached, exactly as before. Only a
            // prepare failure reaches here — execution errors surface during
            // iteration, and a missing param can't occur on this auto-built path.
            Err(_) => self.run_uncached(sql, &Params::none(), debug),
            ok => ok,
        }
    }

    /// Executes through the plan cache: reuse the plan for `key`, else prepare
    /// `src`, cache it, and run. Returns the prepare error (so [`query`] can fall
    /// back) when `src` cannot be prepared.
    fn run_cached(&mut self, key: &str, src: &str, params: &Params, debug: bool) -> Result<Rows> {
        // A detached `Rc` handle, so a borrow does not alias `&mut self` below.
        let cache = self.plan_cache.clone();
        // Fast path: reuse a cached plan (cloned out, and bound to a local so the
        // cache borrow is released before the `&mut self` execute call).
        let cached = cache.borrow_mut().get(key);
        if let Some(stmt) = cached {
            match self.execute_prepared(&stmt, params, debug) {
                // A CREATE/DROP invalidated the plan — evict and rebuild below.
                Err(Error::StalePreparedStatement) => cache.borrow_mut().remove(key),
                other => return other,
            }
        }
        // Miss (or evicted stale): prepare and cache. A freshly prepared plan is
        // never stale, and the compiled program is valid regardless of this
        // execution's outcome, so cache it unconditionally.
        let stmt = self.prepare(src)?;
        cache.borrow_mut().insert(key.to_owned(), stmt.clone());
        self.execute_prepared(&stmt, params, debug)
    }

    /// Prepares and runs `sql` once without consulting or populating the plan
    /// cache — the fallback for SQL that can't be auto-parameterized.
    fn run_uncached(&mut self, sql: &str, params: &Params, debug: bool) -> Result<Rows> {
        let stmt = self.prepare(sql)?;
        self.execute_prepared(&stmt, params, debug)
    }

    /// Normalizes `sql` into an auto-parameterized template: every *numeric*
    /// literal token is replaced by a `?` placeholder and its [`Value`] collected,
    /// in source order. Inter-token text — and string literals — are copied
    /// verbatim, so the template is valid SQL whose `?`s bind back to the
    /// extracted values (matching the grammar's `number` → `Value::number` action).
    ///
    /// Only numbers in *expression* position are parameterized: a `?` there
    /// substitutes to an identical `Expr::Lit`. Numbers in a `LIMIT` clause are
    /// kept literal (they parse as compile-time counts, not expressions, so a `?`
    /// would fail to parse and defeat the cache). **String literals are left
    /// intact** because a string in a `FROM` source is lowered to a file scan
    /// based on the *literal* at parse time (see `looks_like_file`), which a `?`
    /// would silently defeat.
    ///
    /// Returns `None` — falling back to the direct path — on a lexer error, or
    /// when the SQL already contains an explicit `?`/`$N`/`$name` placeholder
    /// (which would collide with the auto-extracted positional values).
    fn normalize(sql: &str) -> Option<(String, Vec<Value>)> {
        let mut key = String::with_capacity(sql.len());
        let mut vals = Vec::new();
        let mut last = 0;
        // LIMIT operands are parsed as compile-time `number` tokens, not
        // expressions, so a `?` there fails to parse and would defeat the cache.
        // Keep them literal. The mode spans `limit N` / `limit N..` / `limit N..M`
        // and ends at the first token that is neither a number nor `..`.
        let mut in_limit = false;
        for item in SqlLexer::new(sql) {
            let (start, token, end) = item.ok()?;
            key.push_str(&sql[last..start]);
            // A LIMIT operand (`limit N` / `N..` / `N..M`) is kept literal; the
            // mode opens at `limit` and runs while numbers and `..` follow it.
            let limit_operand = in_limit && matches!(token, Token::Number(_) | Token::DotDot);
            in_limit = matches!(token, Token::Limit) || limit_operand;
            match token {
                // An explicit placeholder shares the positional index space with
                // the numbers we extract; mixing the two misnumbers bindings (and
                // a literal can silently satisfy a `$N`). Bail to the direct,
                // uncached path rather than build a corrupt parameter list.
                Token::Question | Token::NumberedParam(_) | Token::NamedParam(_) => {
                    return None;
                }
                Token::Number(s) if !limit_operand => {
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
    ///
    /// Cached by raw SQL: because parameters resolve to runtime slots, one
    /// compiled program serves every set of bound values, so a repeated
    /// parameterized statement reuses its plan instead of re-parsing each call.
    pub fn query_with(&mut self, sql: &str, params: &Params, debug: bool) -> Result<Rows> {
        self.run_cached(sql, sql, params, debug)
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
        assert!(db.plan_cache.borrow().plans.contains_key("select t[?];"));
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

    #[test]
    fn normalize_bails_on_explicit_placeholder() {
        // SQL that already carries a `?`/`$N` is not auto-templated — its
        // placeholders would collide with the values normalize extracts.
        assert!(MonaDB::normalize("select 1 + $1;").is_none());
        assert!(MonaDB::normalize("select t[?];").is_none());
        // A plain literal query still templates.
        assert!(MonaDB::normalize("select 1 + 2;").is_some());
    }

    #[test]
    fn query_with_explicit_param_and_literal_does_not_misbind() {
        // Regression: the literal must not silently satisfy the unbound `$1`;
        // query() (no params) surfaces a clean missing-parameter error instead.
        let mut db = MonaDB::memory().unwrap();
        assert!(db.query("select 1 + $1;", false).is_err());
    }

    #[test]
    fn limit_query_is_cached() {
        // A numeric LIMIT is kept literal so the template parses and caches,
        // instead of failing to parse and falling back uncached every call.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.query("select * from t limit 1;", false).unwrap().finish().unwrap();
        assert!(
            db.plan_cache.borrow().plans.contains_key("select * from t limit 1;"),
            "a LIMIT query should be cached, not silently fall back"
        );
    }

    #[test]
    fn query_with_caches_by_sql() {
        // The parameterized path reuses one compiled program across calls.
        let mut db = MonaDB::memory().unwrap();
        db.query_with("select $1;", &Params::positional(vec![Value::int(1)]), false)
            .unwrap()
            .finish()
            .unwrap();
        assert!(db.plan_cache.borrow().plans.contains_key("select $1;"));
    }

    #[test]
    fn param_keyed_from_source_runs() {
        // Regression: a parameter key in a keyed FROM source compiles (runtime
        // prefix encoding) rather than erroring with `Unsupported`.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table c (a string, b int);").unwrap();
        db.execute(r#"insert into c ({"a": "x", "b": 1}, {"a": "x", "b": 2});"#)
            .unwrap();
        let mut rows = db
            .query_with(
                "select r from c[$1] as r;",
                &Params::positional(vec![Value::String(std::rc::Rc::from("x"))]),
                false,
            )
            .unwrap();
        let mut n = 0;
        while rows.next().unwrap().is_some() {
            n += 1;
        }
        assert_eq!(n, 2, "parameterized prefix FROM source should stream its rows");
    }

    #[test]
    fn plan_cache_evicts_least_recently_used() {
        let db = MonaDB::memory().unwrap();
        let mut cache = PlanCache::new(2);
        cache.insert("a".to_owned(), db.prepare("select 1;").unwrap());
        cache.insert("b".to_owned(), db.prepare("select 2;").unwrap());
        // Touch "a", making "b" the least-recently-used.
        assert!(cache.get("a").is_some());
        cache.insert("c".to_owned(), db.prepare("select 3;").unwrap());
        assert!(cache.plans.contains_key("a"));
        assert!(cache.plans.contains_key("c"));
        assert!(
            !cache.plans.contains_key("b"),
            "the least-recently-used plan should be evicted"
        );
    }
}
