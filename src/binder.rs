use crate::catalog::Catalog;
use crate::error::{Error, Result};
use crate::ir::{Expr, From, Insert, Source, Statement};
use crate::transaction::Transaction;
use crate::visitor::visit_mut::{VisitMut, visit_expr_mut, visit_insert_mut};

/// The binder assigns cursor slots and resolves variable references.
pub struct Binder<'txn> {
    /// The binder needs a transaction to do catalog lookups.
    txn: &'txn Transaction,
    /// Catalog for table lookups, gets us the table 'oid'.
    catalog: Catalog,
    scope: Scope,
    //
    next_cursor: u32,
    // Collected errors
    errors: Vec<Error>,
}

impl<'txn> Binder<'txn> {
    /// Create a new binder with a catalog reference.
    pub fn new(catalog: Catalog, txn: &'txn Transaction) -> Self {
        Binder {
            txn,
            catalog,
            scope: Scope::new(),
            next_cursor: 0,
            errors: vec![],
        }
    }

    /// Binds all table and
    pub fn bind(&mut self, statement: &mut Statement) -> Result<()> {
        self.visit_statement_mut(statement);
        if let Some(err) = self.errors.first() {
            Err(err.clone())
        } else {
            Ok(())
        }
    }

    /// Allocate a new cursor ID and increment the counter.
    fn next_cursor(&mut self) -> u32 {
        let id = self.next_cursor;
        self.next_cursor += 1;
        id
    }
}

impl VisitMut for Binder<'_> {
    /// Allocates a new cursor slot, resolving tables and adding bindings.
    fn visit_from_mut(&mut self, i: &mut From) {
        let var = i.var.clone();
        let csr = self.next_cursor();
        i.csr = Some(csr);
        match &i.src {
            Source::Table(name) => {
                let name = name.clone();
                match self.catalog.get_table(self.txn, &name) {
                    Ok(oid) => i.oid = Some(oid),
                    Err(err) => self.errors.push(err),
                }
                self.scope.push(var, csr);
            }
            Source::Value(_) => {
                self.errors.push(Error::BindError(
                    "value/lateral sources are not yet supported".to_string(),
                ));
            }
        }
    }

    fn visit_insert_mut(&mut self, i: &mut Insert) {
        if i.target.bind.is_none() {
            match self.catalog.get_table(self.txn, &i.target.name) {
                Ok(oid) => i.target.bind = Some(oid),
                Err(err) => self.errors.push(err),
            }
        }
        visit_insert_mut(self, i);
    }

    fn visit_expr_mut(&mut self, i: &mut Expr) {
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
    /// Create a new empty scope.
    fn new() -> Self {
        Scope {
            bindings: Vec::new(),
        }
    }

    /// Add a new cursor alias to the scope.
    fn push(&mut self, name: String, csr: u32) {
        self.bindings.push(Binding { name, csr });
    }

    /// Resolve a variable, returning either the bound variable or a resolution error.
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
        ir::{Constructor, Expr, Statement},
    };

    fn db_fixture() -> MonaDB {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table users (id int, name string);").unwrap();
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

    // Lateral / value sources are deferred (Deliverable 2). For now the binder
    // rejects them with a static BindError; these tests guard that deferral and
    // become the starting spec when lateral sources are reintroduced.
    #[test]
    fn test_bind_lateral_source_is_deferred() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table T;").unwrap();
        let mut stmt = MonaDB::parse("select * from T as t, t.items as item;").unwrap();
        let result = db.bind(&mut stmt);
        assert!(matches!(result, Err(Error::BindError(_))));
    }

    #[test]
    fn test_bind_array_literal_source_is_deferred() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table T;").unwrap();
        let mut stmt = MonaDB::parse("select x from [1, 2, 3] as x;").unwrap();
        let result = db.bind(&mut stmt);
        assert!(matches!(result, Err(Error::BindError(_))));
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
        assert!(ins.target.bind.is_some());
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
}
