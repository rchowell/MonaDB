use std::vec;

use crate::{
    compiler::Compiler,
    table::{Column, JType, Schema, Table},
    unsupported,
    value::{JValue, Row},
    Result,
};

use sqlparser::{
    ast::{self, ArrayElemTypeDef, DataType, ExactNumberInfo, ObjectName, Select, SelectItem, Statement, TableFactor, TableWithJoins},
    dialect::SQLiteDialect,
};

/// This currently delegates to the sqlparser crate.
///
/// TODO:
///  - Implement a custom parser.
///
/// NOTES:
///  - Like sqlite/lua, the parser should drive the compiler.
///  - Compiler should have NO knowledge of sqlparser.
///
pub struct Parser<'comp, 'cat> {
    compiler: &'comp mut Compiler<'cat>,
}

impl<'comp, 'cat> Parser<'comp, 'cat> {
    /// Create a new parser with a reference to the compiler.
    pub fn new(compiler: &'comp mut Compiler<'cat>) -> Parser<'comp, 'cat> {
        Parser { compiler }
    }

    /// Parse the RQL query and invoke the compiler routines to build the program.
    pub fn parse(&mut self, rql: &str) -> Result<()> {
        match Self::parse_statement(rql)? {
            Statement::CreateTable(create_table) => {
                // CREATE TABLE ...
                self.parse_create_table(create_table)?;
            }
            Statement::Insert(insert) => {
                // INSERT INTO ...
                self.parse_insert(insert)?;
            }
            Statement::Drop { names, .. } => {
                // DROP TABLE ...
                if names.len() == 1 {
                    self.parse_drop_table(names[0].clone())?;
                } else {
                    unsupported!("Expected single table name");
                }
            }
            Statement::Query(query) => {
                // SELECT ...
                self.parse_query(&query)?;
            }
            _ => unsupported!("Unsupported statement"),
        }
        // Return ownership of the compiler.
        Ok(())
    }

    fn parse_statement(rql: &str) -> Result<Statement> {
        let dialect = SQLiteDialect {};
        let stmts = sqlparser::parser::Parser::parse_sql(&dialect, &rql)?;
        let stmt = match stmts.len() {
            1 => stmts[0].clone(),
            _ => unsupported!("Expected single statement"),
        };
        Ok(stmt)
    }

    fn parse_create_table(&mut self, create_table: ast::CreateTable) -> Result<()> {
        // TODO assert unsupported features (or err)
        if create_table.columns.is_empty() {
            unsupported!("Table must have at least one column")
        }
        let name = create_table.name.to_string();
        let mut cols: Vec<Column> = vec![];
        for col in &create_table.columns {
            let name = col.name.to_string();
            let jtype = Self::parse_type(&col.data_type)?;
            cols.push(Column { name, jtype });
        }
        let schema = Schema::new(cols);
        let table = Table::new(name, schema);
        self.compiler.create_table(table)
    }

    fn parse_query(&mut self, query: &ast::Query) -> Result<()> {
        if query.with.is_some() {
            unsupported!("WITH Clause is not supported")
        }
        self.parse_set_expr(&query.body)?;
        Ok(())
    }

    /// Parse a set expression – i.e. SELECT or UNION|INTERSECT|EXCEPT.
    /// 
    fn parse_set_expr(&mut self, set_expr: &ast::SetExpr) -> Result<()> {
        use ast::SetExpr::*;
        match set_expr {
            Select(select) => self.parse_select(select),
            _ => unsupported!("set_expr {:?}", set_expr),
        }
    }

    /// Parse a SELECT statement.
    /// 
    /// SYNTAX:
    ///     SELECT <select list> FROM <table>
    /// 
    fn parse_select(&mut self, select: &ast::Select) -> Result<()> {

        // process FROM before SELECT
        let offset = self.parse_from(&select.from)?;
        if select.projection.len() != 1 {
            unsupported!("Expected SELECT *")
        }

        // TODO other clauses...

        // process SELECT list
        if is_select_star(select) {
            self.compiler.spread();
        } else {
            self.parse_select_list(&select.projection)?;
        }

        // emit next and patch the loop
        self.compiler.next(offset)
    }

    /// Parse the SELECT list.
    /// 
    /// SYNTAX:
    ///     <expr> AS <name> [, <expr> AS <name>]*
    ///     
    /// 
    fn parse_select_list(&mut self, items: &Vec<SelectItem>) -> Result<()> {
        // open
        let n = items.len();
        let ptr = self.compiler.alloc(n);
        let mut keys:  Vec<String> = vec![];

        // compile expressions
        for item in items {
            match item {
                SelectItem::UnnamedExpr(_) => unsupported!("SELECT item must have an AS alias"),
                SelectItem::QualifiedWildcard(_, _) => unsupported!("qualified wildcard"),
                SelectItem::Wildcard(_) => unreachable!("wildcard should be handled by is_select_star"),
                SelectItem::ExprWithAlias { expr: _, alias } => {

                    let dest = self.compiler.alloc(1);
                    self.compiler.var(dest, "test");
                    keys.push(alias.to_string());
                },
            }
        }

        // close
        self.compiler.obj(ptr, keys);
        self.compiler.free(n);

        Ok(())
    }

    /// Parse the FROM clause.
    /// 
    /// SYNTAX:
    ///     FROM <table> [, <table>]*
    /// 
    /// This returns the pc offset to patch.
    /// 
    fn parse_from(&mut self, from: &Vec<TableWithJoins>) -> Result<usize> {
        // assert unsupported features
        if from.len() != 1 {
            unsupported!("Multi-FROM source")
        }
        // single from
        let from = &from[0];
        if !from.joins.is_empty() {
            unsupported!("JOIN statements")
        }
        if let TableFactor::Table { name, alias, .. } = &from.relation {
            let table = name.to_string();
            let alias = match alias {
                Some(alias) => alias.to_string(),
                None => table.clone(),
            };
            // open table and bind row into alias.
            self.compiler.open_scan(&table, &alias)
        } else {
            unsupported!("Expected a table")
        }
    }

    /// Parse a DROP TABLE statement.
    ///
    /// SYNTAX:
    ///
    ///     DROP TABLE <table_name>;
    ///
    fn parse_drop_table(&mut self, name: ObjectName) -> Result<()> {
        let table = name.to_string();
        self.compiler.drop_table(table)
    }

    /// Parse an INSERT statement.
    ///
    /// SYNTAX:
    ///
    ///     INSERT INTO <table_name> VALUE <json>;
    ///
    /// TEMPORARY SYNTAX:
    ///
    ///     INSERT INTO <table_name> VALUES ('<json>');
    ///
    fn parse_insert(&mut self, insert: ast::Insert) -> Result<()> {
        // TODO assert unsupported features (or err)
        if !insert.columns.is_empty() {
            unsupported!("INSERT with columns not supported")
        }
        let table = insert.table_name.to_string();
        let value = match &insert.source {
            Some(query) => Self::parse_row(query)?,
            _ => unsupported!("Expected VALUES source"),
        };
        self.compiler.insert(table, value)
    }

    /// Parse an expression
    fn parse_expr(&mut self, expr: &ast::Expr, dest: usize) -> Result<()> {
        todo!()
    }

    /// Parse a data type into a Rho Type, see `value::Type`.
    ///
    /// SYNTAX:
    ///
    ///     ANY
    ///     BOOL/BOOLEAN
    ///     STRING
    ///     NUMBER
    ///     ARRAY
    ///     OBJECT
    ///
    /// TEMPORARY SYNTAX:
    ///
    ///     ANY
    ///     BOOL/BOOLEAN
    ///     NUMERIC
    ///
    /// NOTES:
    ///   - Other types are not supported/tested.
    ///
    fn parse_type(data_type: &DataType) -> Result<JType> {
        let jtype = match data_type {
            DataType::Numeric(info) => {
                if *info != ExactNumberInfo::None {
                    unsupported!("Unsupported numeric type: {:?}", info)
                }
                JType::Number
            }
            DataType::Bool | DataType::Boolean => JType::Boolean,
            DataType::String(len) => {
                if len.is_some() {
                    unsupported!("Unsupported string length: {:?}", len)
                }
                JType::String
            }
            DataType::Custom(name, _) => {
                unsupported!("Unsupported custom type: {:?}", name)
            }
            DataType::Array(info) => {
                if *info != ArrayElemTypeDef::None {
                    unsupported!("Unsupported array parameters: {:?}", info)
                }
                JType::Array
            }
            _ => unsupported!("Unsupported data type: {:?}", data_type),
        };
        Ok(jtype)
    }

    /// Parse a string into a `value::Row`.
    ///
    /// SYNTAX:
    ///
    ///     VALUE <json>
    ///
    /// TEMPORARY SYNTAX:
    ///
    ///     VALUES ('<json>')
    ///
    /// NOTES:
    ///  - Make it work with multiple values/rows.
    ///
    fn parse_row(query: &ast::Query) -> Result<Row> {
        // TODO assert unsupported features (or err)
        if let ast::SetExpr::Values(values) = query.body.as_ref() {
            // unpack the single row
            if values.rows.len() != 1 {
                unsupported!("Expected single row")
            }
            // unpack the single value
            let row = &values.rows[0];
            if row.len() != 1 {
                unsupported!("Expected single value in row")
            }
            // parse a string literal
            Self::parse_json(&row[0])
        } else {
            unsupported!("Expected VALUES")
        }
    }

    /// Parse a string literal into a `value::JValue`.
    ///
    /// !! TEMPORARY !!
    ///
    fn parse_json(expr: &ast::Expr) -> Result<JValue> {
        if let ast::Expr::Value(value) = expr {
            if let ast::Value::SingleQuotedString(s) = value {
                JValue::from_str(s)
            } else {
                unsupported!("Expected single quoted string")
            }
        } else {
            unsupported!("Expected literal value")
        }
    }
}

/// Parse a CREATE TABLE statement into a `table::Table`.
///
/// SYNTAX:
///
///     CREATE TABLE <table_name> (
///         <column_name> <data_type>,
///         ...
///     );
///
pub fn parse_table(rql: &str) -> Result<Table> {
    if let Statement::CreateTable(create_table) = Parser::parse_statement(rql)? {
        if create_table.columns.is_empty() {
            unsupported!("Table must have at least one column")
        }
        let name = create_table.name.to_string();
        let mut cols: Vec<Column> = vec![];
        for col in &create_table.columns {
            let name = col.name.to_string();
            let jtype = Parser::parse_type(&col.data_type)?;
            cols.push(Column { name, jtype });
        }
        let schema = Schema::new(cols);
        let table = Table::new(name, schema);
        Ok(table)
    } else {
        unsupported!("Expected CREATE TABLE statement")
    }

}

/// Returns true on SELECT *
fn is_select_star(select: &Select) -> bool {
    select.projection.len() == 1 && matches!(select.projection[0], SelectItem::Wildcard(_))
}