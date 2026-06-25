pub mod error;
pub mod ir;
pub mod schema;

mod binder;
mod cache;
mod catalog;
mod compiler;
mod cursor;
mod display;
mod functions;
pub mod highlight;
pub mod lexer;
mod config;
mod params;
mod prepared;
mod query_options;
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
    parser::SqlParser,
    prepared::StatementPlan,
    transaction::Transaction,
    vm::Program,
};

pub use crate::cache::Cache;
pub use crate::lexer::{SqlLexer, Token};
pub use crate::config::Config;
pub use crate::params::{IntoParams, Params};
pub use crate::query_options::QueryOptions;
pub use crate::prepared::Statement;
pub use crate::value::Value;
pub use crate::vm::Rows;


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
    plans: Rc<RefCell<Cache<StatementPlan>>>,
    /// An explicit write transaction opened by `begin;`, held across statements
    /// until `commit;` or `rollback;`. The slot goes *temporarily* empty while a
    /// lazy [`Rows`] borrows the txn for an in-flight statement, so it is not a
    /// reliable indicator of whether a session is open — [`session_active`] is.
    session_txn: Rc<RefCell<Option<Transaction>>>,
    /// Whether an explicit session is open, tracked independently of
    /// [`session_txn`] occupancy (which dips to `None` while a lazy result borrows
    /// the txn mid-statement). Drives `in_transaction`, the double-`begin` guard,
    /// and the "statement in progress" guard so a held result can't be mistaken
    /// for a closed session.
    session_active: Cell<bool>,
    /// Set when an in-session statement mutates the catalog (a deferred CREATE/
    /// DROP records its change here instead of bumping `catalog_generation`).
    /// `commit_transaction` consumes it to bump the generation exactly once;
    /// `rollback_transaction` clears it without bumping, so a rolled-back DDL
    /// leaves earlier prepared statements valid.
    session_catalog_dirty: Rc<Cell<bool>>,
    /// Runtime options applied to every query until changed.
    query_options: QueryOptions,
}

impl MonaDB {
    /// Open or create a database at the given path.
    ///
    /// Equivalent to `open_with_config(path, Config::default())`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<MonaDB> {
        Self::open_with_config(path, Config::default())
    }

    /// Open or create a database at the given path with the given configuration.
    pub fn open_with_config<P: AsRef<Path>>(path: P, config: Config) -> Result<MonaDB> {
        let storage = Storage::open_with_config(path, &config)?;
        let catalog = Catalog::load(&storage)?;
        Ok(MonaDB {
            storage,
            catalog,
            catalog_generation: Rc::new(Cell::new(0)),
            plans: Rc::new(RefCell::new(Cache::<StatementPlan>::new(256))),
            session_txn: Rc::new(RefCell::new(None)),
            session_active: Cell::new(false),
            session_catalog_dirty: Rc::new(Cell::new(false)),
            query_options: QueryOptions::default(),
        })
    }

    /// Returns the connection's query options.
    pub fn query_options(&self) -> &QueryOptions {
        &self.query_options
    }

    /// Replaces the connection's query options.
    pub fn set_query_options(&mut self, opts: QueryOptions) {
        self.query_options = opts;
    }

    /// Sets query options, returning `self` for chaining at open time.
    #[must_use]
    pub fn with_query_options(mut self, opts: QueryOptions) -> Self {
        self.query_options = opts;
        self
    }

    /// Enables or disables bytecode tracing for subsequent queries.
    #[must_use]
    pub fn debug(mut self, enabled: bool) -> Self {
        self.query_options.set_debug(enabled);
        self
    }

    /// Enables or disables bytecode tracing for subsequent queries.
    pub fn set_debug(&mut self, enabled: bool) {
        self.query_options.set_debug(enabled);
    }

    /// Open an in-memory database.
    ///
    /// Equivalent to `memory_with_config(Config::default())`.
    pub fn memory() -> Result<MonaDB> {
        Self::memory_with_config(Config::default())
    }

    /// Open an in-memory database with the given configuration.
    pub fn memory_with_config(config: Config) -> Result<MonaDB> {
        let tmp_dir = TempDir::new()?;
        let tmp_pth = tmp_dir.path().join("memory.db");
        Self::open_with_config(&tmp_pth, config)
    }

    /// Run a query, returning a lazy iterator over its result rows. The
    /// statement's transaction commits once the iterator is exhausted.
    /// Mirrors rusqlite's `Connection::query`.
    ///
    /// Ad-hoc SQL is run through the auto-parameterizing plan cache: literals are
    /// normalized to a `?`-templated key (see [`MonaDB::normalize`]) so repeated
    /// query *shapes* reuse a prepared statement and skip re-parsing. Anything
    /// that doesn't parameterize cleanly falls back to a direct parse+compile.
    pub fn query(&mut self, sql: &str) -> Result<Rows> {
        if let Some(res) = self.route_session(sql, &Params::none()) {
            return res;
        }
        // A lex error, or SQL with an explicit placeholder, isn't auto-templated:
        // run it directly (uncached).
        let Some((key, vals)) = Self::normalize(sql) else {
            return self.run_uncached(sql, &Params::none());
        };
        let params = Params::positional(vals);
        match self.run_cached(&key, &key, &params) {
            // The template didn't prepare (e.g. a literal in a non-expr position);
            // fall back to the concrete SQL, uncached, exactly as before. Only a
            // prepare failure reaches here — execution errors surface during
            // iteration, and a missing param can't occur on this auto-built path.
            Err(_) => self.run_uncached(sql, &Params::none()),
            ok => ok,
        }
    }

    /// Executes through the plan cache: reuse the plan for `key`, else prepare
    /// `src`, cache it, and run. Returns the prepare error (so [`query`] can fall
    /// back) when `src` cannot be prepared.
    fn run_cached(&mut self, key: &str, src: &str, params: &Params) -> Result<Rows> {
        // A detached `Rc` handle, so a borrow does not alias `&mut self` below.
        let cache = self.plans.clone();
        // Fast path: reuse a cached plan (Rc handle, borrow released before execute).
        let cached = cache.borrow_mut().get(key);
        if let Some(plan) = cached {
            match self.execute_plan(&plan, params) {
                // A CREATE/DROP invalidated the plan — evict and rebuild below.
                Err(Error::StalePreparedStatement) => cache.borrow_mut().del(key),
                other => return other,
            }
        }
        // Miss (or evicted stale): prepare and cache. A freshly prepared plan is
        // never stale, and the compiled program is valid regardless of this
        // execution's outcome, so cache it unconditionally.
        let plan = self.compile_plan(src)?;
        let plan = cache.borrow_mut().put(key, plan);
        self.execute_plan(&plan, params)
    }

    /// Prepares and runs `sql` once without consulting or populating the plan
    /// cache — the fallback for SQL that can't be auto-parameterized.
    fn run_uncached(&mut self, sql: &str, params: &Params) -> Result<Rows> {
        let plan = self.compile_plan(sql)?;
        self.execute_plan(&plan, params)
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
    pub fn query_with(
        &mut self,
        sql: &str,
        params: impl IntoParams,
    ) -> Result<Rows> {
        let params = params.into_params();
        if let Some(res) = self.route_session(sql, &params) {
            return res;
        }
        self.run_cached(sql, sql, &params)
    }

    /// Run a statement to completion, committing it, and return the number of
    /// rows produced. Mirrors rusqlite's `Connection::execute`.
    pub fn execute(&mut self, sql: &str) -> Result<u64> {
        self.query(sql)?.finish()
    }

    /// Run a parameterized statement to completion, returning its row count.
    pub fn execute_with(&mut self, sql: &str, params: impl IntoParams) -> Result<u64> {
        self.query_with(sql, params)?.finish()
    }

    /// Intercepts an explicit transaction-control statement (`begin;`/`commit;`/
    /// `rollback;`) before the plan cache, running it and returning an empty
    /// [`Rows`]. Returns `None` for any other statement so the caller compiles
    /// and runs it normally.
    ///
    /// Detection peeks the leading token: `begin`/`commit`/`rollback` are reserved
    /// keyword tokens that only ever start a control statement, so the common
    /// (non-control) case returns after a single token and stays off the
    /// parser/plan-cache hot path that [`MonaDB::query`] relies on. Only once a
    /// control keyword is seen does it scan the rest of the stream — and a control
    /// statement with a *trailing* statement (e.g. `commit; insert …`) falls
    /// through to the normal path (which rejects multi-statement input) rather than
    /// silently running the control and discarding the rest. These statements carry
    /// no literals, so they are never normalized, templated, or cached. Every entry
    /// path — `execute`, `execute_with`, `query`, `query_with`, and so the Python
    /// `Connection::run` — routes here.
    fn try_txn_control(&mut self, sql: &str) -> Option<Result<Rows>> {
        use crate::lexer::{SqlLexer, Token};
        let mut lexer = SqlLexer::new(sql);
        let first = lexer.next()?.ok()?.1;
        if !matches!(first, Token::Begin | Token::Commit | Token::Rollback) {
            return None;
        }
        // A bare control statement is only `<keyword> ;?` — anything else after it
        // is a second statement we must not silently drop.
        for item in lexer {
            match item {
                Ok((_, Token::SemiColon, _)) => continue,
                _ => return None,
            }
        }
        let res = match first {
            Token::Begin => self.begin_transaction(),
            Token::Commit => self.commit_transaction(),
            Token::Rollback => self.rollback_transaction(),
            _ => unreachable!("first token matched a control keyword above"),
        };
        Some(res.map(|()| Rows::empty()))
    }

    /// Handles the statement routing shared by `query` and `query_with`, before
    /// the plan cache: intercept transaction control, reject a statement issued
    /// while another is still in progress, and — inside a session — bypass the
    /// plan cache (an in-session CREATE/DROP mutates the catalog without bumping
    /// the generation the cache keys staleness on, so a cached plan can't be
    /// trusted; re-bind every statement against the session). Returns `Some` when
    /// the statement is fully handled here, or `None` to continue on the cached path.
    fn route_session(&mut self, sql: &str, params: &Params) -> Option<Result<Rows>> {
        if let Some(res) = self.try_txn_control(sql) {
            return Some(res);
        }
        if let Err(e) = self.guard_statement_in_progress() {
            return Some(Err(e));
        }
        if self.in_transaction() {
            return Some(self.run_uncached(sql, params));
        }
        None
    }

    /// Errors if an in-flight statement still holds the session txn out of its
    /// slot — issuing another statement (or `commit;`/`rollback;`) before the prior
    /// result is consumed or dropped would otherwise mis-bind against a fresh
    /// non-session txn or strand the session. Safe no-op outside a session and
    /// while the txn sits in its slot.
    fn guard_statement_in_progress(&self) -> Result<()> {
        if self.session_active.get() && self.session_txn.borrow().is_none() {
            return Err(Error::Transaction(
                "a previous statement is still in progress; consume or drop its result \
                 before continuing the transaction"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Opens an explicit write transaction (`begin;`).
    pub fn begin_transaction(&mut self) -> Result<()> {
        // Guard on the session flag, not slot occupancy: a held lazy result can
        // leave the slot momentarily empty even though a session is open, and
        // opening a second LMDB write txn there would block the writer.
        if self.session_active.get() {
            return Err(Error::Transaction("transaction already active".into()));
        }
        *self.session_txn.borrow_mut() = Some(Transaction::write(&self.storage)?);
        self.session_active.set(true);
        Ok(())
    }

    /// Commits the active explicit transaction (`commit;`).
    ///
    /// If the session mutated the catalog, the generation is bumped exactly once
    /// here — committed DDL becomes visible just as a non-session CREATE does,
    /// and only on success.
    pub fn commit_transaction(&mut self) -> Result<()> {
        if !self.session_active.get() {
            return Err(Error::Transaction("no active transaction".into()));
        }
        let Some(txn) = self.session_txn.borrow_mut().take() else {
            return Err(Error::Transaction(
                "cannot commit while a statement is in progress".into(),
            ));
        };
        txn.commit()?;
        self.session_active.set(false);
        if self.session_catalog_dirty.replace(false) {
            let generation = self.catalog_generation.get();
            self.catalog_generation.set(generation + 1);
        }
        Ok(())
    }

    /// Aborts the active explicit transaction (`rollback;`).
    ///
    /// The generation is *not* bumped (so previously prepared statements stay
    /// valid), but the catalog cache is flushed: any entry learned through the
    /// now-aborted txn — e.g. a table created and rolled back mid-session — must
    /// not linger.
    pub fn rollback_transaction(&mut self) -> Result<()> {
        if !self.session_active.get() {
            return Err(Error::Transaction("no active transaction".into()));
        }
        let Some(txn) = self.session_txn.borrow_mut().take() else {
            return Err(Error::Transaction(
                "cannot roll back while a statement is in progress".into(),
            ));
        };
        txn.abort();
        self.session_active.set(false);
        self.session_catalog_dirty.set(false);
        self.catalog.flush();
        Ok(())
    }

    /// Returns whether an explicit transaction is active.
    pub fn in_transaction(&self) -> bool {
        self.session_active.get()
    }

    /// Aborts any open explicit session on drop so uncommitted writes are not flushed.
    fn abort_session_if_open(&mut self) {
        if let Some(txn) = self.session_txn.borrow_mut().take() {
            txn.abort();
        }
        self.session_active.set(false);
        self.session_catalog_dirty.set(false);
    }

    /// Returns the current catalog generation (for stale-prepare detection).
    pub(crate) fn catalog_generation(&self) -> u64 {
        self.catalog_generation.get()
    }

    /// Phase 1: Parse input string into our AST (no binding or compilation).
    pub fn parse(sql: &str) -> Result<ir::Statement> {
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
    /// misses the in-memory cache — so a warm bind touches no transaction. When
    /// an explicit session is open, cold lookups instead scan through the session
    /// txn, so a table CREATEd earlier in the session is visible to a later
    /// statement that references it.
    fn bind(&self, statement: &mut ir::Statement) -> Result<()> {
        let mut binder = Binder::new(
            self.catalog.clone(),
            self.storage.clone(),
            self.catalog_generation(),
            self.session_txn.clone(),
        );
        binder.bind(statement)
    }

    /// Phase 3: Compilation is pure bytecode generation.
    #[allow(clippy::unused_self)]
    fn compile(&self, statement: ir::Statement) -> Result<Program> {
        let cc = Compiler::new();
        cc.compile(statement)
    }

    /// Prints a program's bytecode as an address/operation table (a debug aid).
    fn trace_program(program: &Program) {
        println!();
        println!("addr\toperation");
        println!("----\t---------");
        for (addr, op) in program.instructions.iter().enumerate() {
            println!("{addr:04}\t{op:?}");
        }
        println!();
    }
}

impl Drop for MonaDB {
    fn drop(&mut self) {
        self.abort_session_if_open();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use crate::transaction::Transaction;
    use tempfile::TempDir;

    #[test]
    fn test_select_bytecode() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let mut db = MonaDB::open(&db_path).unwrap();

        db.execute("create table t (id int);").unwrap();
        // let _ = db.debug(true).query("select * from t;");
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
        let mut r1 = db.query("select t[1];").unwrap();
        assert_eq!(r1.next().unwrap().unwrap().jpk("id"), Some(Value::int(1)));
        let mut r2 = db.query("select t[2];").unwrap();
        assert_eq!(r2.next().unwrap().unwrap().jpk("id"), Some(Value::int(2)));

        // The shared template was cached and reused (not one entry per literal).
        assert!(db.plans.borrow().exists("select t[?];"));
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
            db.query("select t[1];").is_err(),
            "dropped table must not resolve from a stale catalog cache"
        );

        // Recreating with a different shape must be visible (cache re-scanned).
        db.execute("create table t (name string);").unwrap();
        db.execute(r#"insert into t ({"name": "x"});"#).unwrap();
        let mut rows = db.query(r#"select t["x"];"#).unwrap();
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
            .query("select * from people as r order by r.name;")
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
        assert!(db.query("select 1 + $1;").is_err());
    }

    #[test]
    fn limit_query_is_cached() {
        // A numeric LIMIT is kept literal so the template parses and caches,
        // instead of failing to parse and falling back uncached every call.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.query("select * from t limit 1;").unwrap().finish().unwrap();
        assert!(
            db.plans.borrow().exists("select * from t limit 1;"),
            "a LIMIT query should be cached, not silently fall back"
        );
    }

    #[test]
    fn query_with_caches_by_sql() {
        // The parameterized path reuses one compiled program across calls.
        let mut db = MonaDB::memory().unwrap();
        db.query_with("select $1;", &Params::positional(vec![Value::int(1)]))
            .unwrap()
            .finish()
            .unwrap();
        assert!(db.plans.borrow().exists("select $1;"));
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
            )
            .unwrap();
        let mut n = 0;
        while rows.next().unwrap().is_some() {
            n += 1;
        }
        assert_eq!(n, 2, "parameterized prefix FROM source should stream its rows");
    }

    #[test]
    fn explicit_transaction_batches_inserts() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.begin_transaction().unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        db.execute(r#"insert into t ({"id": 2});"#).unwrap();
        db.commit_transaction().unwrap();
        assert!(!db.in_transaction());
        let ro = Transaction::read(&db.storage).unwrap();
        assert!(keyed_row_exists(&db, &ro, 1));
        assert!(keyed_row_exists(&db, &ro, 2));
    }

    #[test]
    fn parse_transaction_control() {
        use crate::lexer::{SqlLexer, Token};
        let tokens: Vec<_> = SqlLexer::new("begin;")
            .map(|r| r.map(|(_, t, _)| t))
            .collect();
        assert_eq!(tokens, vec![Ok(Token::Begin), Ok(Token::SemiColon)]);
        assert!(matches!(MonaDB::parse("begin;"), Ok(ir::Statement::Begin)));
        assert!(matches!(MonaDB::parse("commit;"), Ok(ir::Statement::Commit)));
        assert!(matches!(MonaDB::parse("rollback;"), Ok(ir::Statement::Rollback)));
    }

    #[test]
    fn explicit_transaction_sql_syntax() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute("begin;").unwrap();
        assert!(db.in_transaction());
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        db.execute("commit;").unwrap();
        assert!(!db.in_transaction());
        let ro = Transaction::read(&db.storage).unwrap();
        assert!(keyed_row_exists(&db, &ro, 1));
    }

    #[test]
    fn multi_value_insert_batches_commit() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(
            r#"insert into t ({"id": 1}, {"id": 2}, {"id": 3});"#,
        )
        .unwrap();
        let ro = Transaction::read(&db.storage).unwrap();
        assert!(keyed_row_exists(&db, &ro, 1));
        assert!(keyed_row_exists(&db, &ro, 3));
    }

    #[test]
    fn rollback_ends_session() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.begin_transaction().unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        db.rollback_transaction().unwrap();
        assert!(!db.in_transaction());
    }

    #[test]
    fn empty_keyed_table_subscript_returns_none() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        let mut rows = db.query("select * from t;").unwrap();
        assert!(rows.next().unwrap().is_none());
    }

    fn keyed_row_exists(db: &MonaDB, txn: &Transaction, id: i64) -> bool {
        let def = db.catalog.scan_and_cache(txn, "t").unwrap();
        let oid = def.oid.unwrap();
        let btree = db.storage.open_btree(txn, oid).unwrap();
        let key = schema::encode_key(&Value::from_json(serde_json::json!({"id": id})), &def.keys)
            .unwrap();
        btree.get(txn.as_ro(), &key).unwrap().is_some()
    }

    #[test]
    fn rollback_discards_session_writes() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.begin_transaction().unwrap();
        assert_eq!(db.execute(r#"insert into t ({"id": 1});"#).unwrap(), 1);
        {
            let mut session = db.session_txn.borrow_mut();
            let txn = session.as_mut().unwrap();
            assert!(
                keyed_row_exists(&db, txn, 1),
                "row should be visible in the session write txn"
            );
        }
        db.rollback_transaction().unwrap();
        let ro = Transaction::read(&db.storage).unwrap();
        assert!(
            !keyed_row_exists(&db, &ro, 1),
            "rolled-back row should not be visible after abort"
        );
        let mut rows = db.query("select * from t;").unwrap();
        assert!(rows.next().unwrap().is_none());
    }

    #[test]
    fn rollback_then_commit_keeps_only_later_writes() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.begin_transaction().unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        db.rollback_transaction().unwrap();
        db.begin_transaction().unwrap();
        db.execute(r#"insert into t ({"id": 2});"#).unwrap();
        db.commit_transaction().unwrap();
        let ro = Transaction::read(&db.storage).unwrap();
        assert!(keyed_row_exists(&db, &ro, 2));
        assert!(!keyed_row_exists(&db, &ro, 1));
    }

    #[test]
    fn prepared_insert_reuses_plan() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        let mut stmt = db.prepare("insert into t ($1);").unwrap();
        for id in 1..=3 {
            let val = Value::from_json(serde_json::json!({"id": id}));
            stmt.execute([val]).unwrap();
        }
        let ro = Transaction::read(&db.storage).unwrap();
        assert!(keyed_row_exists(&db, &ro, 3));
    }

    #[test]
    fn nosync_opens() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nosync.db");
        let db = MonaDB::open_with_config(&path, Config::default().nosync());
        assert!(db.is_ok());
    }

    // ---- Explicit-transaction correctness (docs/plans/10-transactions.md) ----

    #[test]
    fn session_read_sees_own_writes() {
        // #1: a SELECT in a session must read the session's uncommitted insert.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.begin_transaction().unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        let mut rows = db.query("select * from t;").unwrap();
        assert!(
            rows.next().unwrap().is_some(),
            "session select must see its own uncommitted insert"
        );
        drop(rows); // restore the session txn to the slot before committing
        db.commit_transaction().unwrap();
    }

    #[test]
    fn mid_statement_error_keeps_session_open() {
        // #2: a runtime error mid-statement must not silently close the session.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (x int);").unwrap();
        db.begin_transaction().unwrap();
        let res = db.execute(r#"insert into t ({"z": 9});"#); // missing key field
        assert!(res.is_err(), "inserting without the key field should error");
        assert!(
            db.in_transaction(),
            "session must stay open after a mid-statement error"
        );
        db.rollback_transaction().unwrap();
    }

    #[test]
    fn deferred_error_restores_session_txn() {
        // #2 (focused): a deferred write returning Err leaves session_txn populated.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (x int);").unwrap();
        db.begin_transaction().unwrap();
        let mut stmt = db.prepare(r#"insert into t ({"z": 9});"#).unwrap();
        let res = stmt.query(()).and_then(|rows| rows.finish());
        assert!(res.is_err());
        assert!(
            db.session_txn.borrow().is_some(),
            "deferred txn must be restored to the session slot after an error"
        );
        db.rollback_transaction().unwrap();
    }

    #[test]
    fn rollback_of_ddl_keeps_prepared_valid() {
        // #3: a rolled-back DDL must not advance the generation and brick prepares.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        {
            let mut stmt = db.prepare("select * from t;").unwrap();
            stmt.query(()).unwrap();
        }
        db.begin_transaction().unwrap();
        db.execute("create table u (id int);").unwrap();
        db.rollback_transaction().unwrap();
        let mut stmt = db.prepare("select * from t;").unwrap();
        let n = stmt.query(()).unwrap().finish().unwrap();
        assert_eq!(n, 1, "earlier prepared statement must survive a rolled-back DDL");
    }

    #[test]
    fn create_then_use_in_session() {
        // #4: a table created mid-session must be bindable later in that session.
        let mut db = MonaDB::memory().unwrap();
        db.begin_transaction().unwrap();
        db.execute("create table u (id int);").unwrap();
        db.execute(r#"insert into u ({"id": 1});"#).unwrap();
        let mut rows = db.query("select * from u;").unwrap();
        assert!(
            rows.next().unwrap().is_some(),
            "must see rows in a table created in this session"
        );
        drop(rows);
        db.commit_transaction().unwrap();
    }

    #[test]
    fn session_state_correct_while_result_held() {
        // Review finding #1: a partially-consumed lazy result borrows the session
        // txn out of its slot. Session state must stay correct and every operation
        // must stay safe (clear errors, never corruption or a second write txn).
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.begin_transaction().unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        db.execute(r#"insert into t ({"id": 2});"#).unwrap();
        let mut rows = db.query("select * from t;").unwrap();
        rows.next().unwrap().unwrap(); // borrow the session txn out of the slot

        assert!(db.in_transaction(), "session still active while a result is held");
        assert!(
            db.begin_transaction().is_err(),
            "double begin must be rejected, not open a second write txn"
        );
        assert!(
            db.query("select * from t;").is_err(),
            "a second statement must be rejected while one is in progress"
        );
        assert!(
            db.commit_transaction().is_err(),
            "commit must be rejected while a statement is in progress"
        );

        drop(rows); // returns the txn to the slot
        db.commit_transaction().unwrap(); // now it succeeds
        assert!(!db.in_transaction());
    }

    #[test]
    fn in_session_drop_then_reference_is_unbound() {
        // Review finding #2: an in-session DROP must invalidate a cached positive
        // hit, so a later reference in the same session does not resolve the
        // dropped table.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        db.query("select * from t;").unwrap().finish().unwrap(); // warm positive cache
        db.begin_transaction().unwrap();
        db.execute("drop table t;").unwrap();
        assert!(
            db.query("select * from t;").is_err(),
            "a table dropped in-session must not resolve from a stale cache hit"
        );
        db.rollback_transaction().unwrap();
        // The DROP was rolled back, so `t` resolves again.
        db.query("select * from t;").unwrap().finish().unwrap();
    }

    #[test]
    fn control_statement_with_trailing_is_not_silently_dropped() {
        // Review finding #3: `commit; insert ...` must not run the commit and
        // silently discard the trailing statement.
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        db.begin_transaction().unwrap();
        db.execute(r#"insert into t ({"id": 1});"#).unwrap();
        assert!(
            db.execute(r#"commit; insert into t ({"id": 2});"#).is_err(),
            "a control statement with a trailing statement must error, not partially run"
        );
        assert!(
            db.in_transaction(),
            "the rejected multi-statement must not have committed the session"
        );
        db.rollback_transaction().unwrap();
    }

    #[test]
    fn plan_cache_evicts_least_recently_used() {
        let mut db = MonaDB::memory().unwrap();
        let mut cache: Cache<StatementPlan> = Cache::new(2);
        cache.put("a", db.compile_plan("select 1;").unwrap());
        cache.put("b", db.compile_plan("select 2;").unwrap());
        // Touch "a", making "b" the least-recently-used.
        assert!(cache.get("a").is_some());
        cache.put("c", db.compile_plan("select 3;").unwrap());
        assert!(cache.exists("a"));
        assert!(cache.exists("c"));
        assert!(
            !cache.exists("b"),
            "the least-recently-used plan should be evicted"
        );
    }
}
