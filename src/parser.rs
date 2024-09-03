use std::vec;

use sqlparser::{ast::{ArrayElemTypeDef, DataType, ExactNumberInfo, Statement}, dialect::GenericDialect, parser::Parser};
use crate::{table::{Column, JType, Schema, Table}, unsupported, Result};

pub fn parse(rql: &str) -> Result<Statement> {
    let dialect = GenericDialect {};
    let stmts = Parser::parse_sql(&dialect, &rql)?;
    let stmt = match stmts.len() {
        1 => stmts[0].clone(),
        _ => unsupported!("Expected single statement"),
    };
    Ok(stmt)
}

/// Parse a RQL query into a `table::Table`.
pub fn parse_table(rql: &str) -> Result<Table> {
    let stmt = parse(rql)?;
    match stmt {
        Statement::CreateTable(create_table) => parse_create_table(&create_table),
        _ => unsupported!("Expected CREATE TABLE statement"),
    }
}

/// Parse a CREATE TABLE statement into a `table::Table`.
pub fn parse_create_table(create_table: &sqlparser::ast::CreateTable) -> Result<Table> {
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

/// Parse a data type into a Rho Type, see `value::Type`.
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
