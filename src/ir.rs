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

#[derive(Debug)]
pub enum Rex {
    Var(String),
    Lit(String),
    Obj(Obj),
    Jpi { inp: RexRef, idx: usize },
    Jpk { inp: RexRef, key: String },
    Spread(Vec<Obj>),
}

pub type RexRef = Box<Rex>;

/// Consider a map .. but also orderedness ??
pub type Obj = Vec<Member>;

#[derive(Debug)]
pub struct Member {
    pub key: String,
    pub val: Rex,
}

pub fn select_star(from: From) -> Select {
    let member = Member {
        key: from.var.clone(),
        val: Rex::Var(from.var.clone()),
    };
    Select {
        inp: from,
        sel: vec![member],
    }
}

pub fn select_list(from: From, members: Vec<Member>) -> Select {
    Select { inp: from, sel: members }
}
