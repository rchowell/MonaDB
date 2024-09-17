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

/// FROM <tbl> AS <var>
#[derive(Debug)]
pub struct From {
    pub tbl: String, // table name
    pub var: String, // AS <var>
}

/// <rex> AS <var>
#[derive(Debug)]
pub struct Member {
    pub rex: Rex,
    pub var: String,
}

#[derive(Debug)]
pub enum Rex {
    Col(String),
    Lit(String),
    Obj(Vec<Member>),
    Jpi { inp: RexRef, idx: usize },
    Jpk { inp: RexRef, key: String },
}

pub type RexRef = Box<Rex>;
