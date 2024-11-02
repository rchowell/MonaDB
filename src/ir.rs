use std::{fmt::Display, vec};

use crate::value::*;

//------------------------------
// Statements
//------------------------------

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
    Table(Table),
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
pub struct Table {
    pub name: String,
    pub schema: Type,
    // pub options: TableOptions,
}

impl Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "create table {} ({});", self.name, self.schema)
    }
}

//------------------------------
// DQL
//------------------------------

pub type Select = Block;

#[derive(Debug)]
pub struct Block {
    pub iter: Iter,
    pub pred: Option<Expr>,
    pub cons: Expr,
}

#[derive(Debug)]
pub enum Source {
    Table(String),
    Path(Path),
    Value(Value),
}

// this is a single source iter, consider joins
#[derive(Debug)]
pub struct Iter {
    pub src: Source,
    pub var: String, // AS <var>
}

impl Default for Iter {
    fn default() -> Self {
        Iter {
            src: Source::Value(Value::null()),
            var: "".to_string(),
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
    Number,
    String,
    Object(TObject),
    Array,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TObject {
    pub members: Vec<TMember>,
    pub open: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TMember {
    pub name: String,
    pub typ_: TypeRef,
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Any => write!(f, "any"),
            Type::Bool => write!(f, "bool"),
            Type::Number => write!(f, "number"),
            Type::String => write!(f, "string"),
            Type::Object(object) => object.fmt(f),
            Type::Array => write!(f, "array"),
        }
    }
}

impl Display for TObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.members.is_empty() {
            write!(f, "{{}}")
        } else {
            write!(f, "{{\n")?;
            for m in &self.members {
                write!(f, "  ")?;
                m.fmt(f)?;
                write!(f, ",\n")?;
            }
            write!(f, "}}")
        }
    }
}

impl Display for TMember {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.typ_)
    }
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
    Var(String),
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
pub fn table_definition(name: String, schema: Type) -> Table {
    Table { name, schema }
}

#[inline]
pub fn insert(target: String, source: Vec<Expr>) -> Insert {
    Insert { target, source }
}

#[inline]
pub fn select_star(from: Iter) -> Block {
    let var = Expr::Var(from.var.clone());
    let obj = vec![Member::Spread(var)];
    Block {
        iter: from.into(),
        pred: None,
        cons: Expr::Obj(obj),
    }
}

#[inline]
pub fn select_expr(expr: Expr, from: Option<Iter>) -> Block {
    Block {
        iter: from.unwrap_or_default().into(),
        pred: None,
        cons: expr,
    }
}

#[inline]
pub fn select_list(members: Vec<Member>, from: Option<Iter>) -> Block {
    Block {
        iter: from.unwrap_or_default().into(),
        pred: None,
        cons: Expr::Obj(members),
    }
}

// TODO select x.*, y.* ...
// select_item with .* (spread).
// #[inline]
// pub fn select_item_star(expr: Expr) -> Member {
//     member_spread(expr)
// }

// select_item with alias.
#[inline]
pub fn select_item(expr: Expr, name: String) -> Member {
    Member::Assign(name, expr)
}

#[inline]
pub fn from_table(tbl: String) -> Iter {
    let src = Source::Table(tbl.clone());
    let var = tbl.clone();
    Iter { src, var }
}

#[inline]
pub fn from_source(src: Source, var: String) -> Iter {
    Iter { src, var }
}

#[inline]
pub fn from_source_table(tbl: String) -> Source {
    Source::Table(tbl.clone())
}

#[inline]
pub fn from_source_path(identifier: String, segments: Vec<Segment>) -> Source {
    Source::Path(Path { identifier, segments })
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

pub fn t_number() -> Type {
    Type::Number
}

pub fn t_string() -> Type {
    Type::String
}

pub fn t_object(members: Vec<TMember>, open: bool) -> Type {
    Type::Object(TObject { members, open })
}

pub fn t_member(name: String, typ_: Type, _: bool) -> TMember {
    TMember { name, typ_: Box::new(typ_) }
}

pub fn t_array() -> Type {
    Type::Array
}

//------------------------------
// Parser Actions: JSONPath
//------------------------------

pub fn path(identifier: String, segments: Vec<Segment>) -> Path {
    Path { identifier, segments }
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
pub fn expr_var(var: String) -> Expr {
    Expr::Var(var)
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
    use crate::{lexer::RqlLexer, parser::RqlParser};

    use super::*;

    fn parse(input: &str) -> Statement {
        let rl = RqlLexer::new(input);
        let pp = RqlParser::new();
        pp.parse(rl).unwrap()
    }

    #[test]
    fn test_accept_from() {
        let paths = vec![
            // Table
            "T",
            "Table",
            // Basic paths
            "T$.store.book.title",
            "T$.store['book'].title",
            "T$.store['book']['title']",
            "T$.store.book.*",
            "T$.store.book[0]",
            "T$.store.book[0].title",
            "T$.store.book[0]..title",
            "T$.store.book[0]..*",
            "T$.store.book[0]..*.*",
            // Wildcard paths
            "T$.store.*.title",
            "T$.store.*[0]",
            "T$.store.*[0].title",
            "T$.store.*[0]..title",
            "T$.store.*[0]..*",
            "T$.store.*[0]..*.*",
            // Array indices
            "T$.store.book[0]",
            "T$.store.book[1]",
            "T$.store.book[-1]",
            "T$.store.book[0,1]",
            // Array slices
            // "T$.store.book[0:2]",
            // "T$.store.book[:2]",
            // "T$.store.book[1:]",
            // "T$.store.book[::2]",
            // Recursive descent
            "T$..book",
            "T$..book.title",
            "T$..book[0]",
            "T$..book[0].title",
            "T$..book[0]..title",
            "T$..book[0]..*",
            "T$..book[0]..*.*",
            // Filters
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
        for path in paths{
            let input = format!("select * from {} as a;", path);
            let _ = parse(&input);
        }
        // ok, no panics
    }
}
