use std::{fmt::Display, mem};

use crate::value::*;

//------------------------------
// Statements
//------------------------------

#[derive(Clone, Debug, Hash, PartialEq)]
pub enum Statement {
    Create(Create),
    Delete(String),
    Drop(String),
    Insert(Insert),
    Select(Select),
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub enum Create {
    Table(Table),
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Insert {
    pub target: String,
    pub source: Vec<Obj>,
}

//------------------------------
// Table
//------------------------------

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Table {
    pub name: String,
    pub members: Vec<TableMember>,
}

impl Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "create table {} (", self.name)?;
        for m in &self.members {
            writeln!(f, "  {} {},", m.name, m.typ_)?;
        }
        writeln!(f, ");")?;
        Ok(())
    }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct TableMember {
    pub name: String,
    pub typ_: Type,
}

/// SELECT <sel> ...
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Select {
    pub inp: From,
    pub sel: Vec<Member>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub enum Member {
    Assign(String, Expr),
    Spread(Expr),
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct From {
    pub tbl: String, // table name
    pub var: String, // AS <var>
}

pub type ExprRef = Box<Expr>;

#[derive(Clone, Debug, Hash, PartialEq)]
pub enum Expr {
    Var(String),
    Lit(Value),
    Obj(Obj),
    Jpi(Jpi),
    Jpk(Jpk),
    Jpe(Jpe),
}

/// JSON data types.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Type {
    Bool,
    Number,
    String,
    Object,
    Array,
    Any,
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Bool => write!(f, "bool"),
            Type::Number => write!(f, "number"),
            Type::String => write!(f, "string"),
            Type::Object => write!(f, "object"),
            Type::Array => write!(f, "array"),
            Type::Any => write!(f, "any"),
        }
    }
}

/// Consider a map .. but also orderedness ?? (indexmap)
/// Also this is an object _expression_ not a value.
pub type Obj = Vec<Member>;

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Jpi {
    pub inp: ExprRef,
    pub idx: usize,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Jpk {
    pub inp: ExprRef,
    pub key: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Jpe {
    pub inp: ExprRef,
    pub exp: ExprRef,
}

//------------------------------
// Parser Actions
//------------------------------

#[inline]
pub fn create_table(name: String, members: Vec<TableMember>) -> Create {
    Create::Table(Table { name, members })
}

#[inline]
pub fn insert(target: String, source: Vec<Obj>) -> Insert {
    Insert { target, source }
}

#[inline]
pub fn select_star(from: From) -> Select {
    let var = Expr::Var(from.var.clone());
    let member = Member::Spread(var);
    Select {
        inp: from,
        sel: vec![member],
    }
}

#[inline]
pub fn select_list(members: Vec<Member>, from: From) -> Select {
    Select {
        inp: from,
        sel: members,
    }
}

pub fn select_path(expr: Expr) -> Member {
    let name = match &expr {
        Expr::Var(var) => var.clone(),
        Expr::Jpk(jpk) => jpk.key.clone(),
        _ => panic!("select_path: {:?}", expr),
    };
    select_item(expr, name)
}

#[inline]
pub fn select_item(expr: Expr, name: String) -> Member {
    Member::Assign(name, expr)
}

#[inline]
pub fn from_no_alias(tbl: String) -> From {
    let var = tbl.clone();
    From { tbl, var }
}

#[inline]
pub fn from(tbl: String, var: String) -> From {
    From { tbl, var }
}

#[inline]
pub fn member_path(expr: Expr) -> Member {
    let name = match &expr {
        Expr::Var(var) => var.clone(),
        Expr::Jpk(jpk) => jpk.key.clone(),
        _ => panic!("select_item_no_alias: {:?}", expr),
    };
    Member::Assign(name, expr)
}

#[inline]
pub fn member_assign(name: String, expr: Expr) -> Member {
    Member::Assign(name, expr)
}

#[inline]
pub fn member_spread(expr: Expr) -> Member {
    Member::Spread(expr)
}

#[inline]
pub fn table_member(name: String, typ_: Type) -> TableMember {
    TableMember { name, typ_ }
}

//------------------------------
// EXPRESSIONS
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
pub fn expr_obj(obj: Obj) -> Expr {
    Expr::Obj(obj)
}

#[inline]
pub fn expr_jpi(inp: Expr, idx: usize) -> Expr {
    Expr::Jpi(Jpi {
        inp: Box::new(inp),
        idx,
    })
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
