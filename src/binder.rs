use crate::catalog::Catalog;
use crate::error::{Error, Result};
use crate::ir::{Expr, From, Insert, Source, Statement};
use crate::transaction::Transaction;
use crate::visitor::visit_mut::{VisitMut, visit_expr_mut, visit_from_mut, visit_insert_mut};

/// The binder assigns cursor slots and resolves variable references.
pub struct Binder<'txn> {
    /// The binder needs a transaction to do catalog lookups.
    txn: &'txn Transaction,
    /// Catalog for table lookups, gets us the table 'oid'.
    catalog: Catalog,
    ///
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
    /// The 'from' clause introduces bindings in its source + var
    fn visit_from_mut(&mut self, i: &mut From) {
        let from = i;
        // Allocate the new cursor slot.
        let csr = self.next_cursor();
        from.csr = Some(csr);
        // Extract the table name, no visit here.
        let table_name = match &from.src {
            Source::Table(name) => name,
            Source::Unnest(_) => unimplemented!(),
        };
        // Look up the table OID in the catalog
        match self.catalog.get_table(self.txn, table_name) {
            Ok(oid) => {
                from.oid = Some(oid);
            }
            Err(err) => {
                self.errors.push(err);
            }
        };
        // Add the cursor alias to the scope, then descend.
        self.scope.push(from.var.clone(), csr);
        visit_from_mut(self, from);
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
        if let Expr::Var(var) = i && var.bind.is_none() {
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
                return Some(binding.csr)
            }
        }
        None
    }

    /// Check if a name exists in the scope.
    fn contains(&self, name: &str) -> bool {
        self.bindings.iter().any(|e| e.name == name)
    }
}

#[cfg(test)]
mod test {
    use crate::{MonaDB, error::Error, ir::{Constructor, Expr, Statement}};

    fn db_fixture() -> MonaDB {
        let mut db = MonaDB::memory().unwrap();
        let mut rows = db
            .exec("create table users (id int, name string);", false)
            .unwrap();
        // TODO: fix this, no need to consume rows to ensure transaction is committed
        loop {
            match rows.next() {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => break,
            }
        }
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
        assert_eq!(sel.from.csr, Some(0));
        assert!(sel.from.oid.is_some());
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
        let mut create_rows = db.exec("create table items (id int);", false).unwrap();
        // Consume rows to ensure transaction is committed
        loop {
            match create_rows.next() {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => break,
            }
        }
        let mut insert_rows = db.exec("insert into items ({id: 1});", false).unwrap();
        // Consume rows
        loop {
            match insert_rows.next() {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => break,
            }
        }
        let mut rows = db.exec("select u.id from items as u;", false).unwrap();
        let row = rows.next().unwrap();
        assert!(row.is_some());
    }
}
