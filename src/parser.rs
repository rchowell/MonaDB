use std::vec;

use sqlparser::{ast::{self, ArrayElemTypeDef, DataType, ExactNumberInfo, Statement}, dialect::SQLiteDialect, parser::Parser};
use crate::{table::{Column, JType, Schema, Table}, unsupported, value::{JValue, Row}, Result, Vop};

pub fn parse(rql: &str) -> Result<Statement> {
    let dialect = SQLiteDialect {};
    let stmts = Parser::parse_sql(&dialect, &rql)?;
    let stmt = match stmts.len() {
        1 => stmts[0].clone(),
        _ => unsupported!("Expected single statement"),
    };
    Ok(stmt)
}

/// Parse a RQL query into a `table::Table`.
pub fn parse_table(rql: &str) -> Result<Table> {
    if let Statement::CreateTable(create_table) = parse(rql)? {
        parse_create_table(&create_table)
    } else {
        unsupported!("Expected CREATE TABLE statement")
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
pub fn parse_create_table(create_table: &ast::CreateTable) -> Result<Table> {
    // TODO assert unsupported features (or err)
    if create_table.columns.is_empty() {
        unsupported!("Table must have at least one column")
    }
    let name = create_table.name.to_string();
    let mut cols: Vec<Column> = vec![];
    for col in &create_table.columns {
        let name = col.name.to_string();
        let jtype = parse_type(&col.data_type)?;
        cols.push(Column { name, jtype });
    }
    let schema = Schema::new(cols);
    Ok(Table::new(name, schema))
}

/// Parse an INSERT statement into a `vm::Vop`.
/// 
/// SYNTAX:
/// 
///     INSERT INTO <table_name> VALUE <json>;
/// 
/// TEMPORARY SYNTAX:
/// 
///     INSERT INTO <table_name> VALUES ('<json>');
/// 
pub fn parse_insert(insert: &ast::Insert) -> Result<Vop> {
    // TODO assert unsupported features (or err)
    if !insert.columns.is_empty() {
        unsupported!("INSERT with columns not supported")
    }
    let table = insert.table_name.to_string();
    let value = match &insert.source {
        Some(query) => parse_row(query)?,
        _ => unsupported!("Expected VALUES source"),
    };
    // TODO parser should drive the compiler (that's sqlite/lua style).
    Ok(Vop::insert(table, value))
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
pub fn parse_type(data_type: &DataType) -> Result<JType> {
    let jtype = match data_type {
        DataType::Numeric(info) => {
            if *info != ExactNumberInfo::None {
                unsupported!("Unsupported numeric type: {:?}", info)
            }
            JType::Number
        }
        DataType::Bool | DataType::Boolean => {
            JType::Boolean
        },
        DataType::String(len) => {
            if len.is_some() {
                unsupported!("Unsupported string length: {:?}", len)
            }
            JType::String
        },
        DataType::Custom(name, _) => {
            unsupported!("Unsupported custom type: {:?}", name)
        },
        DataType::Array(info) => {
            if *info != ArrayElemTypeDef::None {
                unsupported!("Unsupported array parameters: {:?}", info)
            }
            JType::Array
        },
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
pub fn parse_row(query: &ast::Query) -> Result<Row> {
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
        parse_json(&row[0])
    } else {
        unsupported!("Expected VALUES")
    }
}

/// Parse a string literal into a `value::JValue`.
pub fn parse_json(expr: &ast::Expr) -> Result<JValue> {
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
