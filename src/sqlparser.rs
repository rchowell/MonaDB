use std::vec;

use crate::{
    compiler::Compiler,
    table::{Column, JType, Schema, Table},
    unsupported,
    value::{JValue, Row},
    Result,
};

use sqlparser::{
    ast::{
        self, ArrayElemTypeDef, BinaryOperator, DataType, ExactNumberInfo, ObjectName, Select,
        SelectItem, Statement, TableFactor, TableWithJoins,
    },
    dialect::GenericDialect,
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
    return_: bool,
}

impl<'comp, 'cat> Parser<'comp, 'cat> {
    /// Create a new parser with a reference to the compiler.
    pub fn new(compiler: &'comp mut Compiler<'cat>) -> Parser<'comp, 'cat> {
        Parser {
            compiler,
            return_: true,
        }
    }

    /// Parse the RQL query and invoke the compiler routines to build the program.
    pub fn parse(&mut self, rql: &str) -> Result<()> {
        match Self::parse_statement(rql)? {
            Statement::CreateTable(create_table) => self.parse_create_table(create_table),
            Statement::Drop { names, .. } => {
                // DROP TABLE ...
                if names.len() == 1 {
                    self.parse_drop_table(names[0].clone())
                } else {
                    unsupported!("Expected single table name");
                }
            }
            Statement::Delete(delete) => self.parse_delete(delete),
            Statement::Insert(insert) => self.parse_insert(insert),
            Statement::Query(query) => self.parse_query(&query),
            _ => unsupported!("Unsupported statement"),
        }
    }

    fn parse_statement(rql: &str) -> Result<Statement> {
        let dialect = GenericDialect {};
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

    /// Parse a DELETE statement.
    ///
    /// SYNTAX:
    ///    DELETE FROM <table> WHERE <condition>;
    ///
    fn parse_delete(&mut self, delete: ast::Delete) -> Result<()> {
        println!("DELTE: {:?}", delete);

        if delete.selection.is_some() {
            unsupported!("DELETE with <where>")
        }
        if delete.returning.is_some() {
            unsupported!("DELETE with <returning>")
        }

        // extract single table.
        let table = match &delete.from {
            ast::FromTable::WithoutKeyword(_) => unsupported!("FROM keyword is required"),
            ast::FromTable::WithFromKeyword(tables) => {
                if tables.len() > 1 {
                    unsupported!("multiple tables in DELETE FROM")
                }
                &tables[0]
            }
        };
        let (table, _) = self.parse_table(table)?;
        self.compiler.clear(&table);

        Ok(())
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
        match set_expr {
            ast::SetExpr::Select(select) => self.parse_select(select),
            _ => unsupported!("set_expr {:?}", set_expr),
        }
    }

    /// Parse a SELECT statement.
    ///
    /// SYNTAX:
    ///     SELECT <select list> FROM <table>
    ///
    fn parse_select(&mut self, select: &ast::Select) -> Result<()> {
        // check context
        let return_ = self.return_;
        self.return_ = false;

        // process FROM before SELECT
        let jmp = self.parse_from(&select.from)?;

        // TODO other clauses...

        // process SELECT list
        let dest = if is_select_star(select) {
            self.compiler.spread()
        } else {
            self.parse_select_list(&select.projection)?
        };

        // If at root ctx, then add a return instruction.
        if return_ {
            self.compiler.return_(dest);
        }

        // emit next and patch the loop
        self.compiler.next(jmp)
    }

    /// Parse the SELECT list into an object.
    ///
    /// SYNTAX:
    ///     <expr> AS <name> [, <expr> AS <name>]*
    ///     
    fn parse_select_list(&mut self, items: &Vec<SelectItem>) -> Result<usize> {
        let mut members: Vec<(String, usize)> = vec![];
        for item in items {
            let (expr, alias) = match item {
                SelectItem::ExprWithAlias { expr, alias } => (expr, alias),
                SelectItem::UnnamedExpr(expr) => derive_alias(expr)?,
                SelectItem::QualifiedWildcard(_, _) => unsupported!("qualified wildcard"),
                SelectItem::Wildcard(_) => unreachable!("wildcard"),
            };
            let k = alias.value.to_string();
            let v = self.parse_expr(expr)?;
            members.push((k, v));
        }
        Ok(self.compiler.obj(members))
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

        // open table and bind row into alias.
        let (table, alias) = self.parse_table(from)?;
        self.compiler.open(&table, &alias)
    }

    /// Parse a [TableWithJoins] to a (name,alias) pair;
    fn parse_table(&mut self, table: &TableWithJoins) -> Result<(String, String)> {
        if let TableFactor::Table { name, alias, .. } = &table.relation {
            let table = name.to_string();
            let alias = match alias {
                Some(alias) => alias.name.value.clone(),
                None => table.clone(),
            };
            Ok((table, alias))
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
        self.compiler.drop(table)
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

    /// Parse an expression – placing the result in the dest.
    fn parse_expr(&mut self, expr: &ast::Expr) -> Result<usize> {
        use ast::Expr::*;
        let dest = match expr {
            Identifier(id) => {
                //
                self.compiler.var(&id.value)
            }
            CompoundIdentifier(ids) => {
                let mut dst = self.compiler.var(&ids[0].value);
                for key in ids[1..].iter() {
                    dst = self.compiler.json_path_key(dst, &key.value)
                }
                dst
            }
            Nested(expr) => self.parse_expr(expr)?,
            MapAccess { column, keys } => {
                let column = self.parse_expr(column)?;
                todo!("map access({}, {:?})", column, keys)
            }
            Subscript { expr, subscript } => {
                let operand = self.parse_expr(expr)?;
                let index = match subscript.as_ref() {
                    ast::Subscript::Index { index } => parse_lit(index)?,
                    _ => unsupported!("Unsupported subscript, expected index"),
                };
                self.compiler.json_path_index(operand, index)
            }
            BinaryOp { left, op, right } => {
                let lhs = self.parse_expr(left)?;
                let rhs = self.parse_expr(right)?;
                if *op != BinaryOperator::Plus {
                    unsupported!("binary operators")
                }
                // self.compiler.plus(lhs, rhs)
                todo!("plus({}, {})", lhs, rhs)
            }
            _ => unsupported!("Unsupported expression {:?}", expr),
        };
        Ok(dest)
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

/// Derive an alias
/// 
/// 1. Single identifier, then the alias is that identifier
/// 2. Path expression, then the alias the final identifier
/// 
fn derive_alias(expr: &ast::Expr) -> Result<(&ast::Expr, &ast::Ident)> {
    let alias = match expr {
        ast::Expr::Identifier(alias) => alias,
        ast::Expr::CompoundIdentifier(ids) => ids.last().unwrap(),
        _ => unsupported!("SELECT item must have an AS alias")
    };
    Ok((expr, alias))
}

/// Parse a literal into a usize, else error.
fn parse_lit(expr: &ast::Expr) -> Result<usize> {
    use ast::Expr::*;
    match expr {
        Value(value) => match value {
            ast::Value::Number(n, _) => Ok(n.parse().unwrap()),
            _ => unsupported!("Expected number literal"),
        },
        _ => unsupported!("Expected literal"),
    }
}
