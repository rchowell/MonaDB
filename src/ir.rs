use std::fmt::Display;

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub enum Statement {
    Create(Create),
    Delete(String),
    Drop(String),
    Insert(()),
    Select(Select),
}

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub enum Create {
    Table(Table),
}

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
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

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub struct TableMember {
    pub name: String,
    pub typ_: Type,
}

/// SELECT <sel> ...
#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub struct Select {
    pub inp: From,
    pub sel: Vec<Member>,
}

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub struct Member {
    pub key: String,
    pub val: Expr,
}

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub struct From {
    pub tbl: String, // table name
    pub var: String, // AS <var>
}

pub type ExprRef = Box<Expr>;

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub enum Expr {
    Var(String),
    Lit(String),
    Obj(Obj),
    Jpi(Jpi),
    Jpk(Jpk),
    Spread(Vec<Obj>),
}

/// JSON data types.
#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
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

/// Consider a map .. but also orderedness ??
pub type Obj = Vec<Member>;

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub struct Jpi {
    pub inp: ExprRef,
    pub idx: usize,
}

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub struct Jpk {
    pub inp: ExprRef,
    pub key: String,
}

#[inline]
pub fn create_table(name: String, members: Vec<TableMember>) -> Create {
    Create::Table(Table { name, members })
}

#[inline]
pub fn select_star(from: From) -> Select {
    let member = Member {
        key: from.var.clone(),
        val: Expr::Var(from.var.clone()),
    };
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

#[inline]
pub fn member(key: String, val: Expr) -> Member {
    Member { key, val }
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

// pub fn expr_lit(lit: &str) -> Expr {
//     Expr::Lit(lit.to_string())
// }

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
pub fn expr_jpk(inp: Expr, key: &str) -> Expr {
    Expr::Jpk(Jpk {
        inp: Box::new(inp),
        key: key.to_string(),
    })
}
