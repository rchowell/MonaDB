#[derive(Debug)]
pub enum Statement {
    Delete(String),
    Drop(String),
    Insert(()),
    Select(Select),
}

/// SELECT <sel> ...
#[derive(Debug)]
pub struct Select {
    pub inp: From,
    pub sel: Vec<Member>,
}

#[derive(Debug)]
pub struct Member {
    pub key: String,
    pub val: Expr,
}

/// FROM <tbl> AS <var>
#[derive(Debug)]
pub struct From {
    pub tbl: String, // table name
    pub var: String, // AS <var>
}

pub type ExprRef = Box<Expr>;

#[derive(Debug)]
pub enum Expr {
    Var(String),
    Lit(String),
    Obj(Obj),
    Jpi(Jpi),
    Jpk(Jpk),
    Spread(Vec<Obj>),
}

/// Consider a map .. but also orderedness ??
pub type Obj = Vec<Member>;

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

pub fn select_list(from: From, members: Vec<Member>) -> Select {
    Select { inp: from, sel: members }
}

pub fn member(key: String, val: Expr) -> Member {
    Member { key, val }
}

//------------------------------
// EXPRESSIONS
//------------------------------

pub fn expr_var(var: String) -> Expr {
    Expr::Var(var)
}

// pub fn expr_lit(lit: &str) -> Expr {
//     Expr::Lit(lit.to_string())
// }

pub fn expr_obj(obj: Obj) -> Expr {
    Expr::Obj(obj)
}

pub fn expr_jpi(inp: Expr, idx: usize) -> Expr {
    Expr::Jpi(Jpi { 
        inp: Box::new(inp),
        idx,
    })
}

pub fn expr_jpk(inp: Expr, key: &str) -> Expr {
    Expr::Jpk(Jpk { 
        inp: Box::new(inp),
        key: key.to_string(),
    })
}
