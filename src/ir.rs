use std::{ffi::CStr, vec};

use crate::value::Value;

pub use crate::display::ToSql;

#[derive(Debug)]
pub enum Statement {
    Create(Create),
    Delete(String),
    Drop(String),
    Insert(Insert),
    Select(Select),
}

#[derive(Debug)]
pub enum Create {
    Table(TableDefinition),
}

#[derive(Debug)]
pub struct Insert {
    pub target: String,
    pub source: Vec<Expr>,
}

//------------------------------
// Table Definition
//------------------------------

#[derive(Debug, PartialEq, Clone)]
pub struct TableDefinition {
    pub name: String,
    pub members: Vec<TableMember>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TableMember {
    pub name: String,
    pub ty: Type,
}

//------------------------------
// DQL
//------------------------------

#[derive(Debug)]
pub struct Select {
    pub from: From,
    // pub with: Option<Expr>,
    pub where_: Option<Where>,
    // group
    // having
    // order
    pub limit: Option<Limit>,
    pub select: Constructor,
}

#[derive(Debug)]
pub enum Constructor {
    None,
    Star,
    Expr(Expr),
    List(Vec<Member>),
}

#[derive(Debug)]
pub enum Source {
    Table(String),
    // Path(Path),
    // Value(Value),
}

#[derive(Debug)]
pub struct From {
    pub src: Source,
    pub var: String, // AS <var>
    pub csr: Option<usize>,
}

pub type Where = Expr;

#[derive(Debug)]
pub enum Limit {
    Skip(u64),
    Take(u64),
    Slice(u64, u64),
}

//------------------------------
// Types
//------------------------------

pub type TypeRef = Box<Type>;

#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Any,
    Bool,
    Int,
    Float,
    Number,
    String,
    Object(TObject),
    Array,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TObject {
    pub members: Vec<TMember>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TMember {
    pub name: String,
    pub ty: TypeRef,
}

//------------------------------
// JSONPath
//------------------------------

#[derive(Debug)]
pub struct Path {
    pub identifier: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug)]
pub enum Segment {
    Child(Vec<Selector>),
    Descd(Vec<Selector>),
}

#[derive(Debug)]
pub enum Selector {
    Name(String),
    Wildcard,
    Index(usize),
}

//------------------------------
// Expressions
//------------------------------

pub type ExprRef = Box<Expr>;

#[derive(Debug)]
pub enum Expr {
    Op(Op),
    Jpi(Jpi),
    Jpk(Jpk),
    Jpe(Jpe),
    Lit(Value),
    Obj(Obj),
    Var(Ref),
}

/// Represents an unbound our bound variable reference.
#[derive(Debug)]
pub struct Ref {
    /// The unboudn variable name
    pub name: String,
    /// The bound cursor slot
    pub cursor: Option<usize>,
}

pub type Obj = Vec<Member>;

// TODO unary operators
#[derive(Debug)]
pub struct Op {
    pub sym: String,
    pub lhs: ExprRef,
    pub rhs: ExprRef,
}

#[derive(Debug)]
pub enum Member {
    Assign(String, Expr),
    Spread(Expr),
}

#[derive(Debug)]
pub struct Jpi {
    pub inp: ExprRef,
    pub idx: usize,
}

#[derive(Debug)]
pub struct Jpk {
    pub inp: ExprRef,
    pub key: String,
}

#[derive(Debug)]
pub struct Jpe {
    pub inp: ExprRef,
    pub exp: ExprRef,
}

//------------------------------
// Parser Actions
//------------------------------

#[inline]
pub fn create_table(table: TableDefinition) -> Create {
    Create::Table(table)
}

#[inline]
pub fn table_definition(name: String, members: Vec<TableMember>) -> TableDefinition {
    TableDefinition { name, members }
}

#[inline]
pub fn table_member(name: String, ty: Type) -> TableMember {
    TableMember { name, ty }
}

#[inline]
pub fn insert(target: String, source: Vec<Expr>) -> Insert {
    Insert { target, source }
}

#[inline]
pub fn select(select: Constructor, block: Select) -> Select {
    Select {
        from: block.from,
        where_: block.where_,
        limit: block.limit,
        select,
    }
}

#[inline]
pub fn select_block(from: From, where_: Option<Where>, limit: Option<Limit>) -> Select {
    Select {
        from,
        where_,
        limit,
        select: Constructor::None,
    }
}

// select_item with alias.
#[inline]
pub fn select_item(expr: Expr, name: String) -> Member {
    Member::Assign(name, expr)
}

#[inline]
pub fn from_table(tbl: String) -> From {
    let src = Source::Table(tbl.clone());
    let var = tbl.clone();
    From { src, var, csr: None }
}

#[inline]
pub fn from_source(src: Source, var: String) -> From {
    From { src, var, csr: None }
}

#[inline]
pub fn from_source_table(tbl: String) -> Source {
    Source::Table(tbl.clone())
}

#[inline]
pub fn limit_skip(offset: f64) -> Limit {
    Limit::Skip(offset as u64)
}

#[inline]
pub fn limit_take(limit: f64) -> Limit {
    Limit::Take(limit as u64)
}

#[inline]
pub fn limit_slice(offset: f64, limit: f64) -> Limit {
    Limit::Slice(offset as u64, limit as u64)
}

// TODO { x, y } => { x: x, y: y } shorthand
// #[inline]
// pub fn member_var(expr: Expr) -> Member {
//     let name = match &expr {
//         Expr::Var(var) => var.clone(),
//         Expr::Jpk(jpk) => jpk.key.clone(),
//         _ => panic!("member_var: {:?}", expr),
//     };
//     Member::Assign(name, expr)
// }

#[inline]
pub fn member_assign(name: String, expr: Expr) -> Member {
    Member::Assign(name, expr)
}

#[inline]
pub fn member_spread(expr: Expr) -> Member {
    Member::Spread(expr)
}

//------------------------------
// Parser Actions: Types
//------------------------------

pub fn t_any() -> Type {
    Type::Any
}

pub fn t_bool() -> Type {
    Type::Bool
}

pub fn t_int() -> Type {
    Type::Int
}

pub fn t_float() -> Type {
    Type::Float
}

pub fn t_number() -> Type {
    Type::Number
}

pub fn t_string() -> Type {
    Type::String
}

pub fn t_object(members: Vec<TMember>) -> Type {
    Type::Object(TObject { members })
}

pub fn t_member(name: String, ty: Type) -> TMember {
    TMember {
        name,
        ty: Box::new(ty),
    }
}

pub fn t_array() -> Type {
    Type::Array
}

//------------------------------
// Parser Actions: JSONPath
//------------------------------

pub fn path(identifier: String, segments: Vec<Segment>) -> Path {
    Path {
        identifier,
        segments,
    }
}

pub fn segment_child(selectors: Vec<Selector>) -> Segment {
    Segment::Child(selectors)
}

pub fn segment_child_name(name: String) -> Segment {
    Segment::Child(vec![Selector::Name(name)])
}

pub fn segment_child_wildcard() -> Segment {
    Segment::Child(vec![Selector::Wildcard])
}

pub fn segment_descd(selectors: Vec<Selector>) -> Segment {
    Segment::Descd(selectors)
}

pub fn segment_descd_name(name: String) -> Segment {
    Segment::Descd(vec![Selector::Name(name)])
}

pub fn segment_descd_wildcard() -> Segment {
    Segment::Descd(vec![Selector::Wildcard])
}

pub fn selector_name(name: String) -> Selector {
    Selector::Name(name)
}

pub fn selector_wildcard() -> Selector {
    Selector::Wildcard
}

pub fn selector_index(idx: usize) -> Selector {
    Selector::Index(idx)
}

//------------------------------
// Parser Actions: Expressions
//------------------------------

#[inline]
pub fn expr_var(name: String) -> Expr {
    Expr::Var(Ref { name, cursor: None })
}

#[inline]
pub fn expr_call(name: String, _: Vec<Expr>) -> Expr {
    unimplemented!("function calls, found: {}", name)
}

#[inline]
pub fn expr_lit(val: Value) -> Expr {
    Expr::Lit(val)
}

#[inline]
pub fn expr_jpk(inp: Expr, key: String) -> Expr {
    Expr::Jpk(Jpk {
        inp: Box::new(inp),
        key,
    })
}

#[inline]
pub fn expr_jpe(inp: Expr, exp: Expr) -> Expr {
    Expr::Jpe(Jpe {
        inp: Box::new(inp),
        exp: Box::new(exp),
    })
}

#[inline]
pub fn expr_obj(obj: Obj) -> Expr {
    Expr::Obj(obj)
}

#[inline]
pub fn expr_op(sym: &str, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Op(Op {
        sym: sym.to_string(),
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

#[cfg(test)]
mod test {
    use crate::{display::ToSql, lexer::SqlLexer, parser::SqlParser};

    use super::*;

    fn parse(input: &str) -> Statement {
        let l = SqlLexer::new(input);
        let p = SqlParser::new();
        p.parse(l).unwrap()
    }

    #[test]
    fn test_acceptance_from() {
        let paths = vec![
            // Table
            "T",
            "Table",
            // // Basic paths
            // "T$.store.book.title",
            // "T$.store['book'].title",
            // "T$.store['book']['title']",
            // "T$.store.book.*",
            // "T$.store.book[0]",
            // "T$.store.book[0].title",
            // "T$.store.book[0]..title",
            // "T$.store.book[0]..*",
            // "T$.store.book[0]..*.*",
            // // Wildcard paths
            // "T$.store.*.title",
            // "T$.store.*[0]",
            // "T$.store.*[0].title",
            // "T$.store.*[0]..title",
            // "T$.store.*[0]..*",
            // "T$.store.*[0]..*.*",
            // // Array indices
            // "T$.store.book[0]",
            // "T$.store.book[1]",
            // "T$.store.book[-1]",
            // "T$.store.book[0,1]",
            // // Array slices
            // // "T$.store.book[0:2]",
            // // "T$.store.book[:2]",
            // // "T$.store.book[1:]",
            // // "T$.store.book[::2]",
            // // Recursive descent
            // "T$..book",
            // "T$..book.title",
            // "T$..book[0]",
            // "T$..book[0].title",
            // "T$..book[0]..title",
            // "T$..book[0]..*",
            // "T$..book[0]..*.*",
            // // Filters
            // "T$.store.book[?(@.price < 10)]",
            // "T$.store.book[?(@.price <= 10)]",
            // "T$.store.book[?(@.price > 10)]",
            // "T$.store.book[?(@.price >= 10)]",
            // "T$.store.book[?(@.price == 10)]",
            // "T$.store.book[?(@.price != 10)]",
            // "T$.store.book[?(@.author == 'John')]",
            // "T$.store.book[?(@.author != 'John')]",
        ];
        // Test each path with an alias
        for path in paths {
            let input = format!("select * from {path} as a;");
            let _ = parse(&input);
        }
        // ok, no panics
    }

    #[test]
    pub fn parse_acceptance_where() {
        let inputs = vec![
            "select * from T where 10;",
            "select * from T where a > 0;",
            // "select * from T where a > 0 and b = 10;",
        ];
        for input in inputs {
            let _ = parse(input);
        }
    }

    #[test]
    fn parse_acceptance_create_table() {
        let cases: &[(&str, &[(&str, Type)])] = &[
            ("create table points;", &[]),
            ("create table points (x int);", &[("x", Type::Int)]),
            (
                "create table points (x int, y int);",
                &[("x", Type::Int), ("y", Type::Int)],
            ),
            ("create table users (id string);", &[("id", Type::String)]),
        ];
        for (input, expected_cols) in cases {
            let stmt = parse(input);
            let Statement::Create(Create::Table(table)) = stmt else {
                panic!("expected create table for {input:?}");
            };
            assert_eq!(table.members.len(), expected_cols.len(), "input: {input:?}");
            for (actual, (name, ty)) in table.members.iter().zip(expected_cols.iter()) {
                assert_eq!(actual.name, *name, "input: {input:?}");
                assert_eq!(actual.ty, *ty, "input: {input:?}");
            }
        }
    }

    #[test]
    pub fn parse_acceptance_limit() {
        let inputs = vec![
            "select * from T limit 20;",
            "select * from T limit 10..;",
            "select * from T limit 10..20;",
        ];
        for input in inputs {
            let _ = parse(input);
        }
    }
}
