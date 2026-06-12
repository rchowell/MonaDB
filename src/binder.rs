//! The binder: name resolution and cursor-slot assignment.
//!
//! A `VisitMut` pass over the IR that assigns each from-source a cursor slot,
//! resolves table names to oids and variables to their binding, and lowers a
//! keyed-table subscript (`t[k]`) to an `Expr::Get`. Errors are collected; the
//! first encountered is returned.

use crate::catalog::Catalog;
use crate::error::{Error, Result};
use crate::ir::{Clear, Drop, Expr, From, Get, Insert, Source, Statement, TableDefinition};
use crate::transaction::Transaction;
use crate::value::Value;
use crate::visitor::visit_mut::{VisitMut, visit_expr_mut, visit_insert_mut};

/// The binder assigns cursor slots and resolves variable references.
pub struct Binder<'txn> {
    /// The binder needs a transaction to do catalog lookups.
    txn: &'txn Transaction,
    /// Catalog for table lookups, gets us the table 'oid'.
    catalog: Catalog,
    scope: Scope,
    /// The next cursor index
    next_cursor: u32,
    /// Collected errors encountered during binding
    errors: Vec<Error>,
}

impl<'txn> Binder<'txn> {
    /// Creates a new binder with a catalog reference.
    pub fn new(catalog: Catalog, txn: &'txn Transaction) -> Self {
        Binder {
            txn,
            catalog,
            scope: Scope::new(),
            next_cursor: 0,
            errors: vec![],
        }
    }

    /// Binds the statement in place, returning the first error encountered.
    pub fn bind(&mut self, statement: &mut Statement) -> Result<()> {
        self.visit_statement_mut(statement);
        if let Some(err) = self.errors.first() {
            Err(err.clone())
        } else {
            Ok(())
        }
    }

    /// Allocates and returns the next cursor slot.
    fn next_cursor(&mut self) -> u32 {
        let id = self.next_cursor;
        self.next_cursor += 1;
        id
    }

    /// Resolves a table name to its definition, recording a bind error on failure.
    fn get_table(&mut self, name: &str) -> Option<TableDefinition> {
        match self.catalog.get_table(self.txn, name) {
            Ok(def) => Some(def),
            Err(err) => {
                self.errors.push(err);
                None
            }
        }
    }
}

impl VisitMut for Binder<'_> {
    /// Allocates a new cursor slot, resolving tables and adding bindings.
    fn visit_from_mut(&mut self, i: &mut From) {
        // Assign the next cursor index for this source
        let csr = self.next_cursor();
        i.csr = Some(csr);

        let var = i.var.clone();
        match &mut i.src {
            Source::Table(name) => {
                // Bind the table to its oid via catalog lookup
                i.oid = self.get_table(name).and_then(|d| d.oid);
                self.scope.push(var, csr);
            }
            Source::Value(expr) => {
                // TODO derived binding names?
                if var.is_empty() {
                    self.errors.push(Error::BindError(
                        "value source requires an alias".to_string(),
                    ));
                    return;
                }
                // Bind the expression against the current scope (lateral refs).
                self.visit_expr_mut(expr);
                self.scope.push(var, csr);
            }
        }
    }

    /// Resolves the insert target table to its oid and key columns.
    fn visit_insert_mut(&mut self, i: &mut Insert) {
        if i.target.oid.is_none()
            && let Some(def) = self.get_table(&i.target.name)
        {
            i.target.oid = def.oid;
            i.target.keys = def.keys;
        }
        visit_insert_mut(self, i);
    }

    /// Resolves the drop target table to its oid.
    fn visit_drop_mut(&mut self, i: &mut Drop) {
        i.oid = self.get_table(&i.name).and_then(|d| d.oid);
    }

    /// Resolves the clear target table to its oid.
    fn visit_clear_mut(&mut self, i: &mut Clear) {
        i.oid = self.get_table(&i.name).and_then(|d| d.oid);
    }

    /// Lowers a keyed-table subscript to a `Get`, else resolves a variable.
    fn visit_expr_mut(&mut self, i: &mut Expr) {
        // A subscript whose base is a bare name that resolves to a catalog
        // table (and is NOT shadowed by a binding) is a keyed table lookup.
        // Try that lowering first; if it doesn't apply, fall through so the
        // base resolves (or errors) as ordinary value path-navigation.
        if self.try_lower_get(i) {
            return;
        }
        if let Expr::Var(var) = i
            && var.bind.is_none()
        {
            var.bind = self.scope.resolve(&var.name);
            if var.bind.is_none() {
                let err = Error::BindError(format!("unresolved variable: {}", &var.name));
                self.errors.push(err);
            }
        } else {
            visit_expr_mut(self, i);
        }
    }
}

impl Binder<'_> {
    /// If `i` is a subscript (`base[exp]` or `base[a, b, ...]`) whose base is a
    /// bare unbound name that names a catalog table, lower it to a keyed-table
    /// access (`Expr::Get`) or record a classification error. Returns `true`
    /// when the subscript was handled here, `false` to fall through to ordinary
    /// value path-navigation binding (a binding shadows the table; a non-table
    /// name resolves-or-errors as a variable as before).
    fn try_lower_get(&mut self, i: &mut Expr) -> bool {
        // Extract (name, arity) without consuming the node — all validation runs
        // before ownership is taken so *i is never left as a spurious Null on
        // an error path.
        let Some((name, arity)) = Self::subscript_parts(i) else {
            return false;
        };
        // A binding shadows the table: resolve scope first, catalog second.
        if self.scope.resolve(&name).is_some() {
            return false;
        }
        // Quiet existence check — a non-table name falls through to the normal
        // unresolved-variable path (no extra error pushed here).
        let Some(def) = self.catalog.get_table(self.txn, &name).ok() else {
            return false;
        };

        let n = def.keys.len();

        // Keyless table, or a key tuple longer than the key → static error.
        if n == 0 {
            self.errors.push(Error::BindError(format!(
                "table '{name}' has no key columns to index"
            )));
            return true;
        }
        if arity > n {
            self.errors.push(Error::BindError(format!(
                "key tuple of {arity} exceeds {n} key column(s) of table '{name}'"
            )));
            return true;
        }
        // A leading prefix (0 < arity < n) is a partial key: it lowers to the
        // same `Expr::Get`, and the compiler distinguishes full (point lookup)
        // from partial (prefix range → array) by comparing `args` to `keys`.

        // v1 accepts literal keys only. Check before taking ownership so that
        // non-literal args are visited for their own BindErrors (e.g. an
        // unresolved variable inside t[ghost]).
        let all_literal = Self::args_all_literal(i);
        if !all_literal {
            self.visit_subscript_args(i);
            self.errors.push(Error::Unsupported(
                "keyed access requires literal keys".to_string(),
            ));
            return true;
        }

        // All validations passed — NOW take ownership of the node.
        let args = Self::take_args(i);

        let Some(oid) = def.oid else {
            self.errors
                .push(Error::InternalError(format!("table '{name}' has no oid")));
            return true;
        };
        let values: Vec<Value> = args
            .into_iter()
            .map(|a| {
                let Expr::Lit(v) = a else { unreachable!() };
                v
            })
            .collect();
        let csr = self.next_cursor();
        *i = Expr::Get(Get {
            csr,
            oid,
            keys: def.keys,
            args: values,
        });
        true
    }

    /// Extracts `(base_name, arity)` from a subscript-shaped `Expr` without
    /// consuming it. Returns `None` if the expression is not a subscript whose
    /// base is a bare unbound variable.
    fn subscript_parts(i: &Expr) -> Option<(String, usize)> {
        match i {
            Expr::Jpe(jpe) => {
                let name = Self::table_base_name(&jpe.inp)?;
                Some((name, 1))
            }
            Expr::Subscript(sub) => {
                let name = Self::table_base_name(&sub.base)?;
                Some((name, sub.args.len()))
            }
            _ => None,
        }
    }

    /// Returns true iff every key argument in the subscript is a literal.
    fn args_all_literal(i: &Expr) -> bool {
        match i {
            Expr::Jpe(jpe) => matches!(*jpe.exp, Expr::Lit(_)),
            Expr::Subscript(sub) => sub.args.iter().all(|a| matches!(a, Expr::Lit(_))),
            _ => true,
        }
    }

    /// Visits each key argument through the normal expression-binding path so
    /// that unresolved variables and other bind errors are collected.
    fn visit_subscript_args(&mut self, i: &mut Expr) {
        match i {
            Expr::Jpe(jpe) => self.visit_expr_mut(&mut jpe.exp),
            Expr::Subscript(sub) => {
                for arg in &mut sub.args {
                    self.visit_expr_mut(arg);
                }
            }
            _ => {}
        }
    }

    /// Consumes the subscript node and returns its argument expressions.
    /// Must only be called after all validation passes — replaces `*i` with a
    /// placeholder only on the success path.
    fn take_args(i: &mut Expr) -> Vec<Expr> {
        match std::mem::replace(i, Expr::Lit(Value::Null)) {
            Expr::Jpe(jpe) => vec![*jpe.exp],
            Expr::Subscript(sub) => sub.args,
            other => {
                *i = other;
                vec![]
            }
        }
    }

    /// Returns the name of a subscript base iff it is a bare unbound variable
    /// (`Expr::Var` with no binding yet). Any nested base (`Jpk`, `Jpe`, …)
    /// returns `None`, keeping it on the value path-navigation track.
    fn table_base_name(base: &Expr) -> Option<String> {
        match base {
            Expr::Var(var) if var.bind.is_none() => Some(var.name.clone()),
            _ => None,
        }
    }
}

/// A single name binding in a scope (e.g. a cursor alias).
#[derive(Debug, Clone)]
struct Binding {
    /// The binding name.
    name: String,
    /// The cursor slot.
    csr: u32,
}

/// Scope maintains the set of active cursor aliases in the current query.
#[derive(Debug, Clone)]
struct Scope {
    bindings: Vec<Binding>,
}

impl Scope {
    /// Creates a new empty scope.
    fn new() -> Self {
        Scope {
            bindings: Vec::new(),
        }
    }

    /// Adds a cursor alias to the scope.
    fn push(&mut self, name: String, csr: u32) {
        self.bindings.push(Binding { name, csr });
    }

    /// Resolves a name to its cursor slot, innermost binding first.
    fn resolve(&self, name: &str) -> Option<u32> {
        for binding in self.bindings.iter().rev() {
            if binding.name == name {
                return Some(binding.csr);
            }
        }
        None
    }
}

#[cfg(test)]
mod test {
    use crate::{
        MonaDB,
        error::Error,
        ir::{Constructor, Expr, Source, Statement},
    };

    fn db_fixture() -> MonaDB {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table users (id int, name string);")
            .unwrap();
        db
    }

    #[test]
    fn test_bind_table_assigns_cursor_and_oid() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("select * from users;").unwrap();
        db.bind(&mut stmt).unwrap();

        // Assert
        let Statement::Select(sel) = stmt else {
            panic!("Expected a select statement")
        };
        assert_eq!(sel.from[0].csr, Some(0));
        assert!(sel.from[0].oid.is_some());
    }

    #[test]
    fn test_bind_cross_join_assigns_distinct_cursors() {
        let mut db = MonaDB::memory().unwrap();
        for ddl in ["create table A;", "create table B;"] {
            db.execute(ddl).unwrap();
        }
        let mut stmt = MonaDB::parse("select * from A as a, B as b;").unwrap();
        db.bind(&mut stmt).unwrap();
        let Statement::Select(sel) = stmt else {
            panic!("expected Select")
        };
        assert_eq!(sel.from[0].csr, Some(0));
        assert_eq!(sel.from[1].csr, Some(1));
        assert!(sel.from[0].oid.is_some() && sel.from[1].oid.is_some());
    }

    // A lateral source binds its expression against the prior scope, so a
    // reference to an earlier binding (`t` in `t.items`) resolves to that
    // cursor and the source allocates its own cursor (no table oid).
    #[test]
    fn test_bind_lateral_source_resolves_outer_binding() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table T;").unwrap();
        let mut stmt = MonaDB::parse("select * from T as t, t.items as item;").unwrap();
        db.bind(&mut stmt).unwrap();
        let Statement::Select(sel) = stmt else {
            panic!("expected Select")
        };
        assert_eq!(sel.from[1].csr, Some(1));
        assert!(sel.from[1].oid.is_none());
        let Source::Value(expr) = &sel.from[1].src else {
            panic!("expected value source")
        };
        let Expr::Jpk(jpk) = expr.as_ref() else {
            panic!("expected Jpk")
        };
        let Expr::Var(var) = jpk.inp.as_ref() else {
            panic!("expected Var")
        };
        assert_eq!(var.bind, Some(0));
    }

    #[test]
    fn test_bind_array_literal_source_binds_cursor() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table T;").unwrap();
        let mut stmt = MonaDB::parse("select x from [1, 2, 3] as x;").unwrap();
        db.bind(&mut stmt).unwrap();
        let Statement::Select(sel) = stmt else {
            panic!("expected Select")
        };
        assert_eq!(sel.from[0].csr, Some(0));
        assert!(sel.from[0].oid.is_none());
    }

    #[test]
    fn test_bind_lateral_self_reference_errors() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table T;").unwrap();
        let mut stmt = MonaDB::parse("select * from T as t, item.x as item;").unwrap();
        assert!(matches!(db.bind(&mut stmt), Err(Error::BindError(_))));
    }

    #[test]
    fn test_bind_resolves_var_in_projection() {
        let db = db_fixture();
        // "select u.id from users as u"
        // u.id parses as Jpk { inp: Var(Unresolved("u")), key: "id" }
        let mut stmt = MonaDB::parse("select u.id from users as u;").unwrap();
        db.bind(&mut stmt).unwrap();
        let Statement::Select(sel) = stmt else {
            panic!("expected Select")
        };
        let Constructor::Expr(Expr::Jpk(jpk)) = sel.select else {
            panic!("expected Jpk")
        };
        let Expr::Var(var) = *jpk.inp else {
            panic!("expected Var")
        };
        assert_eq!(var.bind, Some(0));
    }

    #[test]
    fn test_bind_alias_not_found() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("select y.id from users as u;").unwrap();
        let result = db.bind(&mut stmt);

        assert!(matches!(result, Err(Error::BindError(_))));
    }

    #[test]
    fn test_bind_unknown_table_errors() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("select * from nonexistent;").unwrap();
        let result = db.bind(&mut stmt);
        // catalog.get_table returns UnboundTable
        assert!(matches!(result, Err(Error::UnboundTable(_))));
    }

    #[test]
    fn test_bind_insert_target_oid() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("insert into users ({id: 1});").unwrap();
        db.bind(&mut stmt).unwrap();
        let Statement::Insert(ins) = stmt else {
            panic!("expected Insert")
        };
        assert!(ins.target.oid.is_some());
    }

    #[test]
    fn test_bind_delete_target_and_predicate() {
        let db = db_fixture();
        // Binding succeeds only if `users.id` in the predicate resolves against
        // the target cursor's scope (an unresolved var is a BindError).
        let mut stmt = MonaDB::parse("delete from users where users.id = 1;").unwrap();
        db.bind(&mut stmt).unwrap();
        let Statement::Delete(del) = stmt else {
            panic!("expected Delete")
        };
        assert_eq!(del.from.csr, Some(0));
        assert!(del.from.oid.is_some());
        assert!(del.where_.is_some());
    }

    #[test]
    fn test_bind_drop_resolves_oid() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("drop table users;").unwrap();
        db.bind(&mut stmt).unwrap();
        let Statement::Drop(drop) = stmt else {
            panic!("expected Drop")
        };
        assert!(drop.oid.is_some());
    }

    #[test]
    fn test_bind_drop_unknown_table_errors() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("drop table ghost;").unwrap();
        assert!(matches!(db.bind(&mut stmt), Err(Error::UnboundTable(_))));
    }

    #[test]
    fn test_bind_clear_resolves_oid() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("clear table users;").unwrap();
        db.bind(&mut stmt).unwrap();
        let Statement::Clear(clear) = stmt else {
            panic!("expected Clear")
        };
        assert!(clear.oid.is_some());
    }

    #[test]
    fn test_bind_clear_unknown_table_errors() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("clear table ghost;").unwrap();
        assert!(matches!(db.bind(&mut stmt), Err(Error::UnboundTable(_))));
    }

    #[test]
    fn test_bind_delete_unresolved_predicate_errors() {
        let db = db_fixture();
        // `ghost` is not a binding in scope, so the predicate fails to resolve.
        let mut stmt = MonaDB::parse("delete from users where ghost.id = 1;").unwrap();
        assert!(matches!(db.bind(&mut stmt), Err(Error::BindError(_))));
    }

    #[test]
    fn test_bind_delete_unknown_table_errors() {
        let db = db_fixture();
        let mut stmt = MonaDB::parse("delete from nonexistent;").unwrap();
        assert!(matches!(db.bind(&mut stmt), Err(Error::UnboundTable(_))));
    }

    #[test]
    fn test_exec_select_with_explicit_alias() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table items (id int);").unwrap();
        db.execute("insert into items ({id: 1});").unwrap();
        let mut rows = db.query("select u.id from items as u;", false).unwrap();
        let row = rows.next().unwrap();
        assert!(row.is_some());
    }

    //------------------------------
    // Keyed table subscript (get)
    //------------------------------

    /// Pull the single projected expression out of a bound `select <expr>;`.
    fn projected_expr(stmt: Statement) -> Expr {
        let Statement::Select(sel) = stmt else {
            panic!("expected Select")
        };
        let Constructor::Expr(expr) = sel.select else {
            panic!("expected a single projected expression")
        };
        expr
    }

    #[test]
    fn test_bind_get_int_key() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        let mut stmt = MonaDB::parse("select t[1];").unwrap();
        db.bind(&mut stmt).unwrap();
        let Expr::Get(get) = projected_expr(stmt) else {
            panic!("expected Get, not Jpe — a bare table subscript is a key lookup")
        };
        assert_eq!(get.keys.len(), 1);
        assert_eq!(get.args.len(), 1);
        assert!(
            get.oid > 0,
            "oid should be derived from the table definition"
        );
    }

    #[test]
    fn test_bind_get_wrong_type_still_binds() {
        // A type mismatch (string key against an int key column) is NOT a bind
        // error — it is deferred to the compiler's encoder (a schema error).
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        let mut stmt = MonaDB::parse("select t[\"a\"];").unwrap();
        db.bind(&mut stmt).unwrap();
        let Expr::Get(get) = projected_expr(stmt) else {
            panic!("expected Get even for a type-mismatched literal key")
        };
        assert_eq!(get.keys.len(), 1);
        assert_eq!(get.args.len(), 1);
    }

    #[test]
    fn test_bind_get_composite_key() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table c (a string, b int);").unwrap();
        let mut stmt = MonaDB::parse("select c[\"x\", 7];").unwrap();
        db.bind(&mut stmt).unwrap();
        let Expr::Get(get) = projected_expr(stmt) else {
            panic!("expected Get for a composite full-key subscript")
        };
        assert_eq!(get.keys.len(), 2);
        assert_eq!(get.args.len(), 2);
    }

    #[test]
    fn test_bind_get_arity_too_long_errors() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table c (a int, b int);").unwrap();
        let mut stmt = MonaDB::parse("select c[1, 2, 3];").unwrap();
        assert!(matches!(db.bind(&mut stmt), Err(Error::BindError(_))));
    }

    #[test]
    fn test_bind_get_keyless_errors() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table k;").unwrap();
        let mut stmt = MonaDB::parse("select k[1];").unwrap();
        assert!(matches!(db.bind(&mut stmt), Err(Error::BindError(_))));
    }

    #[test]
    fn test_bind_get_partial_key_lowers_to_get() {
        // A leading prefix (0 < arity < key count) lowers to the same `Get`,
        // carrying fewer args than key columns — the compiler reads that as a
        // prefix range lookup (→ an array of matching rows).
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table c (a int, b int);").unwrap();
        let mut stmt = MonaDB::parse("select c[1];").unwrap();
        db.bind(&mut stmt).unwrap();
        let Expr::Get(get) = projected_expr(stmt) else {
            panic!("expected Get for a partial-key (leading-prefix) subscript")
        };
        assert_eq!(get.keys.len(), 2);
        assert_eq!(
            get.args.len(),
            1,
            "a partial key carries fewer args than key columns"
        );
    }

    #[test]
    fn test_bind_get_unknown_name_errors() {
        let db = db_fixture();
        // `ghost` is neither a binding nor a table → falls through to the
        // existing unresolved-variable bind error.
        let mut stmt = MonaDB::parse("select ghost[1];").unwrap();
        assert!(matches!(db.bind(&mut stmt), Err(Error::BindError(_))));
    }

    #[test]
    fn test_bind_subscript_binding_shadows_table_stays_jpe() {
        // `t` resolves to the FROM binding, which shadows the table `t`, so
        // `t[1]` stays value path-navigation (Jpe), not a key lookup (Get).
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        let mut stmt = MonaDB::parse("select t[1] from t as t;").unwrap();
        db.bind(&mut stmt).unwrap();
        let Expr::Jpe(jpe) = projected_expr(stmt) else {
            panic!("expected Jpe — a bound name shadows the table")
        };
        let Expr::Var(var) = jpe.inp.as_ref() else {
            panic!("expected Var base")
        };
        assert_eq!(var.bind, Some(0));
    }

    #[test]
    fn test_bind_get_nonliteral_unresolved_arg_reports_bind_error() {
        // t[ghost] — base is a table, arg is an unresolved variable.
        // The binder must visit the arg and push a BindError for `ghost`
        // (not just Unsupported for "requires literal key").
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t (id int);").unwrap();
        let mut stmt = MonaDB::parse("select t[ghost];").unwrap();
        assert!(
            matches!(db.bind(&mut stmt), Err(Error::BindError(_))),
            "unresolved arg inside a table subscript should produce BindError"
        );
    }

    #[test]
    fn test_bind_value_index_stays_jpe() {
        // An index into a literal array value is ordinary path-navigation.
        let db = db_fixture();
        let mut stmt = MonaDB::parse("select [1, 2, 3][0];").unwrap();
        db.bind(&mut stmt).unwrap();
        assert!(matches!(projected_expr(stmt), Expr::Jpe(_)));
    }
}
