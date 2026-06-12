use std::vec;

use crate::value::Value;

pub use crate::display::ToSql;

#[derive(Debug)]
pub enum Statement {
    Clear(Clear),
    Create(Create),
    Delete(Delete),
    Drop(Drop),
    Insert(Insert),
    Select(Select),
}

#[derive(Debug)]
pub enum Create {
    Table(TableDefinition),
}

#[derive(Debug)]
pub struct Insert {
    pub target: TableDefinition,
    pub source: Vec<Expr>,
}

#[derive(Debug)]
pub struct Delete {
    pub from: From,
    pub where_: Option<Where>,
}

#[derive(Debug)]
pub struct Drop {
    pub name: String,
    pub oid: Option<u32>,
}

#[derive(Debug)]
pub struct Clear {
    pub name: String,
    pub oid: Option<u32>,
}

//------------------------------
// Table Definition
//------------------------------

#[derive(Debug, PartialEq, Clone)]
pub struct TableDefinition {
    pub oid: Option<u32>,
    pub name: String,
    pub keys: Vec<Key>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Key {
    pub name: String,
    pub ty: Type,
}

//------------------------------
// DQL
//------------------------------

#[derive(Debug)]
pub struct Select {
    pub from: Vec<From>,
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
    Value(Box<Expr>),
}

#[derive(Debug)]
pub struct From {
    pub src: Source,
    pub var: String, // AS <var>
    pub csr: Option<u32>,
    pub oid: Option<u32>, // set by binder for Table sources
}

pub type Where = Expr;

#[derive(Debug)]
pub enum Limit {
    Skip(u64),
    Take(u64),
    Slice(u64, u64),
}

#[derive(Debug)]
pub enum Scope {
    Table,
    Field,
}

/// Variable references are either to tables or a field at a cursor.
#[derive(Debug)]
pub struct Var {
    /// The reference name we get from parsing.
    pub name: String,
    /// The bind is either the bound stack slot or
    pub bind: Option<u32>,
}

impl Var {
    /// Creates an unbound field-first reference.
    pub fn unbound(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bind: None,
        }
    }
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
    Call(Call),
    Jpi(Jpi),
    Jpk(Jpk),
    Jpe(Jpe),
    Lit(Value),
    Obj(Obj),
    Array(Vec<Expr>),
    Var(Var),
    /// Raw multi-element subscript `base[a, b, ...]` (>= 2 args) straight from
    /// the parser; the binder lowers it (table receiver → `Get`, value → error).
    Subscript(Subscript),
    /// A bound keyed-table point lookup (`table[key, ...]`). The binder builds
    /// this once a subscript's base resolves to a catalog table with a full key.
    Get(Get),
}

pub type Obj = Vec<Member>;

#[derive(Debug)]
pub struct Call {
    pub name: String,
    pub args: Vec<Expr>,
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

/// The raw parser node for a multi-element subscript `base[args...]`. The base
/// kind is unknown at parse time; the binder decides table-get vs value access.
#[derive(Debug)]
pub struct Subscript {
    pub base: ExprRef,
    pub args: Vec<Expr>,
}

/// A bound keyed-table point lookup. `args` are the literal key values in key
/// column order; the compiler encodes them into the composite key (Task 3).
#[derive(Debug)]
pub struct Get {
    pub csr: u32,
    pub oid: u32,
    pub keys: Vec<Key>,
    pub args: Vec<Value>,
}

//------------------------------
// Parser Actions
//------------------------------

#[inline]
pub fn create_table(table: TableDefinition) -> Create {
    Create::Table(table)
}

#[inline]
pub fn table_definition(name: String, members: Vec<Key>) -> TableDefinition {
    TableDefinition { oid: None, name, keys: members }
}

#[inline]
pub fn table_key(name: String, ty: Type) -> Key {
    Key { name, ty }
}

#[inline]
pub fn insert(target: String, source: Vec<Expr>) -> Insert {
    Insert {
        target: TableDefinition {
            oid: None,
            name: target,
            keys: vec![],
        },
        source,
    }
}

#[inline]
pub fn delete(table: String, alias: Option<String>, where_: Option<Where>) -> Delete {
    let from = From {
        var: alias.unwrap_or_else(|| table.clone()),
        src: Source::Table(table),
        csr: None,
        oid: None,
    };
    Delete { from, where_ }
}

#[inline]
pub fn drop_table(name: String) -> Drop {
    Drop { name, oid: None }
}

#[inline]
pub fn clear_table(name: String) -> Clear {
    Clear { name, oid: None }
}

#[inline]
pub fn select_value(select: Constructor) -> Select {
    Select { from: vec![], where_: None, limit: None, select }
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
pub fn select_block(from: Vec<From>, where_: Option<Where>, limit: Option<Limit>) -> Select {
    Select {
        from,
        where_,
        limit,
        select: Constructor::None,
    }
}

#[inline]
pub fn select_item(expr: Expr, name: String) -> Member {
    Member::Assign(name, expr)
}

/// Variable without path is assumed to be a table reference. I should
/// probably fix up the grammar here, but this is a reasonable start.
#[inline]
pub fn from_item(src: Expr, alias: Option<String>) -> From {
    match src {
        Expr::Var(var) => From {
            var: alias.unwrap_or_else(|| var.name.clone()),
            src: Source::Table(var.name),
            csr: None,
            oid: None,
        },
        expr => From {
            var: alias.unwrap_or_default(),
            src: Source::Value(Box::new(expr)),
            csr: None,
            oid: None,
        },
    }
}

#[inline]
pub fn limit_skip(offset: u64) -> Limit {
    Limit::Skip(offset)
}

#[inline]
pub fn limit_take(limit: u64) -> Limit {
    Limit::Take(limit)
}

#[inline]
pub fn limit_slice(offset: u64, limit: u64) -> Limit {
    Limit::Slice(offset, limit)
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
    Expr::Var(Var::unbound(&name))
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

/// Builds a multi-element subscript `base[first, rest...]`. The grammar guards
/// arity >= 2 (a single index stays the `Jpe` path-navigation production).
#[inline]
pub fn expr_subscript(base: Expr, first: Expr, rest: Vec<Expr>) -> Expr {
    let mut args = Vec::with_capacity(rest.len() + 1);
    args.push(first);
    args.extend(rest);
    Expr::Subscript(Subscript {
        base: Box::new(base),
        args,
    })
}

#[inline]
pub fn expr_obj(obj: Obj) -> Expr {
    Expr::Obj(obj)
}

#[inline]
pub fn expr_array(items: Vec<Expr>) -> Expr {
    Expr::Array(items)
}

#[inline]
pub fn expr_binary(sym: &str, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Call(Call {
        name: sym.to_string(),
        args: vec![lhs, rhs],
    })
}

#[inline]
pub fn expr_call(name: String, args: Vec<Expr>) -> Expr {
    Expr::Call(Call { name, args })
}

#[inline]
pub fn expr_not(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "not".to_string(),
        args: vec![arg],
    })
}

#[inline]
pub fn expr_is_null(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "is_null".to_string(),
        args: vec![arg],
    })
}

#[inline]
pub fn expr_is_not_null(arg: Expr) -> Expr {
    expr_not(expr_is_null(arg))
}

#[inline]
pub fn expr_is_true(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "is_true".to_string(),
        args: vec![arg],
    })
}

#[inline]
pub fn expr_is_false(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "is_false".to_string(),
        args: vec![arg],
    })
}

#[inline]
pub fn expr_is_unknown(arg: Expr) -> Expr {
    Expr::Call(Call {
        name: "is_unknown".to_string(),
        args: vec![arg],
    })
}

#[inline]
pub fn expr_is_not_true(arg: Expr) -> Expr {
    expr_not(expr_is_true(arg))
}

#[inline]
pub fn expr_is_not_false(arg: Expr) -> Expr {
    expr_not(expr_is_false(arg))
}

#[inline]
pub fn expr_is_not_unknown(arg: Expr) -> Expr {
    expr_not(expr_is_unknown(arg))
}

#[inline]
pub fn expr_between(x: Expr, a: Expr, b: Expr) -> Expr {
    Expr::Call(Call {
        name: "between".to_string(),
        args: vec![x, a, b],
    })
}

#[inline]
pub fn expr_not_between(x: Expr, a: Expr, b: Expr) -> Expr {
    expr_not(expr_between(x, a, b))
}

#[inline]
pub fn expr_in_list(x: Expr, list: Vec<Expr>) -> Expr {
    let mut args = Vec::with_capacity(list.len() + 1);
    args.push(x);
    args.extend(list);
    Expr::Call(Call {
        name: "in_list".to_string(),
        args,
    })
}

#[inline]
pub fn expr_not_in_list(x: Expr, list: Vec<Expr>) -> Expr {
    expr_not(expr_in_list(x, list))
}

#[cfg(test)]
mod test {
    use crate::{lexer::SqlLexer, parser::SqlParser};

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
            assert_eq!(table.keys.len(), expected_cols.len(), "input: {input:?}");
            for (actual, (name, ty)) in table.keys.iter().zip(expected_cols.iter()) {
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
