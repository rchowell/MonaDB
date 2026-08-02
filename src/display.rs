use std::ops::Add;

use crate::ast::{Create, Key, Statement, TMember, TObject, TableDefinition, Type};

/// Renders an AST node back to SQL text.
pub trait ToSql {
    /// Returns a block tree for formatting.
    fn block(&self) -> Block;

    /// Returns a SQL representation.
    fn sql(&self) -> String {
        self.block().sql()
    }
}

/// Block type so we can make a linked-list (tree) of tokens.
#[derive(Clone, Debug)]
pub struct Block {
    /// The block content token
    token: Token,
    /// Next token block (if any) in the list (tree).
    next: Option<Box<Block>>,
}

/// Tokens are the Wadler primitives for pretty-printing.
#[derive(Clone, Debug)]
enum Token {
    /// Raw text string.
    Text(String),
    /// The soft break token, more common hence 'line'.
    Line,
    /// The hard break token.
    LineBreak,
    /// Indent the whole block.
    Indent(Box<Block>),
}

impl Block {
    /// Wraps a single token as a one-element block.
    fn new(token: Token) -> Self {
        Self { token, next: None }
    }

    /// Renders the block tree as a SQL string. Iterative walk; no recursion.
    pub fn sql(&self) -> String {
        let mut out = String::new();
        let mut stack: Vec<(&Block, usize)> = vec![(self, 0)];
        while let Some((curr, depth)) = stack.pop() {
            // Push next prior to any subtree descending
            if let Some(next) = curr.next.as_deref() {
                stack.push((next, depth));
            }
            match &curr.token {
                Token::Text(text) => {
                    // Write the text string as-is.
                    out.push_str(text);
                }
                Token::LineBreak | Token::Line => {
                    // Write both line tokens as '\n' + indent.
                    out.push('\n');
                    for _ in 0..depth {
                        out.push_str("  ");
                    }
                }
                Token::Indent(child) => {
                    // Write the block subtree
                    stack.push((child, depth + 1));
                }
            }
        }
        out
    }

    /// Bumps the indent level for this whole chain.
    fn indent(self) -> Block {
        Block::new(Token::Indent(Box::new(self)))
    }

    /// Nests the chain inside a prefix/suffix pair like `{ }` or `( )`.
    fn nest(self, prefix: &'static str, postfix: &'static str) -> Block {
        text(prefix) + (line() + self).indent() + line() + text(postfix)
    }
}

/// Returns a text block.
fn text(s: impl Into<String>) -> Block {
    Block::new(Token::Text(s.into()))
}

/// Returns a soft-break block.
fn line() -> Block {
    Block::new(Token::Line)
}

/// Returns a hard-break block.
fn linebreak() -> Block {
    Block::new(Token::LineBreak)
}

/// Interleave `sep` between `items`. Empty iterator → empty `Text`.
#[allow(clippy::needless_pass_by_value)]
fn join(sep: Block, items: impl IntoIterator<Item = Block>) -> Block {
    let mut iter = items.into_iter();
    let Some(mut acc) = iter.next() else {
        return text("");
    };
    for item in iter {
        acc = acc + sep.clone() + item;
    }
    acc
}

/// Concatenate two Items (block + block).
impl Add<Block> for Block {
    type Output = Block;
    fn add(mut self, rhs: Block) -> Block {
        let mut tail = &mut self;
        while tail.next.is_some() {
            tail = tail.next.as_mut().unwrap();
        }
        tail.next = Some(Box::new(rhs));
        self
    }
}

/// Concatenate Item and &str (block + string literal).
impl Add<&str> for Block {
    type Output = Block;
    fn add(self, rhs: &str) -> Block {
        self + text(rhs)
    }
}

/// Concatenate &str and Item (string literal + block).
impl Add<Block> for &str {
    type Output = Block;
    fn add(self, rhs: Block) -> Block {
        text(self) + rhs
    }
}

/// Concatenate String and Item (owned string + block).
impl Add<Block> for String {
    type Output = Block;
    fn add(self, rhs: Block) -> Block {
        text(self) + rhs
    }
}

/// Convert &str into a `Token::Text` block.
impl From<&str> for Block {
    fn from(s: &str) -> Self {
        text(s)
    }
}

/// Convert String into a `Token::Text` block.
impl From<String> for Block {
    fn from(s: String) -> Self {
        text(s)
    }
}

impl ToSql for Statement {
    /// Only `create table` is rendered — the catalog stores each table's DDL as
    /// text (see [`crate::catalog`]) and nothing else round-trips through here.
    /// The arms stay exhaustive so a new [`Statement`] variant is a compile
    /// error rather than a runtime panic.
    fn block(&self) -> Block {
        match self {
            Statement::Create(c) => c.block(),
            Statement::Begin => Block::new(Token::Text("begin".into())),
            Statement::Clear(_) => unimplemented!("CLEAR formatting"),
            Statement::Commit => Block::new(Token::Text("commit".into())),
            Statement::Copy(_) => unimplemented!("COPY formatting"),
            Statement::Delete(_) => unimplemented!("DELETE formatting"),
            Statement::Drop(_) => unimplemented!("DROP formatting"),
            Statement::Insert(_) => unimplemented!("INSERT formatting"),
            Statement::Rollback => Block::new(Token::Text("rollback".into())),
            Statement::Select(_) => unimplemented!("SELECT formatting"),
        }
    }
}

impl ToSql for Create {
    fn block(&self) -> Block {
        let mut t = text("");
        match self {
            Create::Table(td) => {
                t = t + td.block();
            }
            Create::TableAs { table: def, .. } => {
                t = t + def.block();
                t = t + text(" as select ...");
            }
        }
        t + text(";")
    }
}

impl ToSql for TableDefinition {
    fn block(&self) -> Block {
        let mut t = text("create table ");
        t = t + text(&self.name);
        if self.keys.is_empty() {
            return t;
        }
        let body = join(text(",") + linebreak(), self.keys.iter().map(ToSql::block));
        t = t + " ";
        t = t + body.nest("(", ")");
        t
    }
}

impl ToSql for Key {
    fn block(&self) -> Block {
        let mut t = text(&self.name);
        t = t + " ";
        t = t + self.ty.block();
        t
    }
}

impl ToSql for Type {
    fn block(&self) -> Block {
        match self {
            Type::Any => text("any"),
            Type::Bool => text("bool"),
            Type::Int => text("int"),
            Type::Float => text("float"),
            Type::Number => text("number"),
            Type::String => text("string"),
            Type::Object(o) => o.block(),
            Type::Array => text("array"),
        }
    }
}

impl ToSql for TObject {
    fn block(&self) -> Block {
        if self.members.is_empty() {
            return text("object");
        }
        let body = join(
            text(",") + linebreak(),
            self.members.iter().map(ToSql::block),
        );
        body.nest("{", "}")
    }
}

impl ToSql for TMember {
    fn block(&self) -> Block {
        let mut t = text(&self.name);
        t = t + ": ";
        t = t + self.ty.block();
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn td(name: &str, members: Vec<(&str, Type)>) -> TableDefinition {
        TableDefinition {
            oid: None,
            name: name.into(),
            keys: members
                .into_iter()
                .map(|(n, ty)| Key { name: n.into(), ty })
                .collect(),
        }
    }

    #[test]
    fn create_table_empty() {
        let t = td("T", vec![]);
        assert_eq!(t.sql(), "create table T");
    }

    #[test]
    fn create_table_standard() {
        let t = td("users", vec![("id", Type::Int), ("name", Type::String)]);
        assert_eq!(t.sql(), "create table users (\n  id int,\n  name string\n)");
    }

    #[test]
    fn nested_object_type() {
        let inner = TObject {
            members: vec![TMember {
                name: "k".into(),
                ty: Box::new(Type::String),
            }],
        };
        let t = td("T", vec![("meta", Type::Object(inner))]);
        assert_eq!(t.sql(), "create table T (\n  meta {\n    k: string\n  }\n)");
    }

    #[test]
    fn dsl_concat() {
        let b = text("a") + " b" + text("c");
        assert_eq!(b.sql(), "a bc");
    }

    #[test]
    fn dsl_str_lhs() {
        let b = "kw " + text("ident");
        assert_eq!(b.sql(), "kw ident");
    }

    #[test]
    fn dsl_nest_outer_indent() {
        let b = text("body").nest("(", ")");
        assert_eq!(b.sql(), "(\n  body\n)");
    }

    #[test]
    fn join_empty_is_empty() {
        let b: Block = join(text(","), Vec::<Block>::new());
        assert_eq!(b.sql(), "");
    }
}
