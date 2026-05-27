use logos::{Logos, SpannedIter};
use std::fmt;

use crate::error::Error;

pub type Spanned<Tok, Loc, Error> = Result<(Loc, Tok, Loc), Error>;

pub struct SqlLexer<'input> {
    tokens: SpannedIter<'input, Token>,
}

impl<'input> SqlLexer<'input> {
    #[must_use]
    pub fn new(input: &'input str) -> Self {
        Self {
            tokens: Token::lexer(input).spanned(),
        }
    }
}

impl Iterator for SqlLexer<'_> {
    type Item = Spanned<Token, usize, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.tokens
            .next()
            .map(|(token, span)| Ok((span.start, token?, span.end)))
    }
}

#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+", error = Error)] // Ignore this regex pattern between tokens
pub enum Token {
    //-------------------------
    // Symbols
    //-------------------------
    #[token("*")]
    Ast,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("$")]
    Dollar,
    #[token("..")]
    DotDot,
    #[token("...")]
    Ellipsis,
    #[token("=")]
    Eq,
    #[token(">")]
    Gt,
    #[token(">=")]
    Ge,
    #[token("<")]
    Lt,
    #[token("<=")]
    Le,
    #[token("-")]
    Minus,
    #[token("!=")]
    Ne,
    #[token("%")]
    Percent,
    #[token("+")]
    Plus,
    #[token(".")]
    Period,
    #[token("|")]
    Pipe,
    #[token(";")]
    SemiColon,
    #[token("/")]
    Sol,
    #[token("(")]
    ParenL,
    #[token(")")]
    ParenR,
    #[token("{")]
    BraceL,
    #[token("}")]
    BraceR,
    #[token("[")]
    BracketL,
    #[token("]")]
    BracketR,
    //-------------------------
    // Keywords
    //-------------------------
    #[token("and")]
    And,
    #[token("as")]
    As,
    #[token("between")]
    Between,
    #[token("create")]
    Create,
    #[token("delete")]
    Delete,
    #[token("drop")]
    Drop,
    #[token("in")]
    In,
    #[token("is")]
    Is,
    #[token("limit")]
    Limit,
    #[token("from")]
    From,
    #[token("insert")]
    Insert,
    #[token("into")]
    Into,
    #[token("not")]
    Not,
    #[token("or")]
    Or,
    #[token("select")]
    Select,
    #[token("table")]
    Table,
    #[token("unknown")]
    Unknown,
    #[token("where")]
    Where,
    //-------------------------
    // Types
    //-------------------------
    #[token("bool")]
    TypeBool,
    #[token("int")]
    TypeInt,
    #[token("float")]
    TypeFloat,
    #[token("number")]
    TypeNumber,
    #[token("string")]
    TypeString,
    #[token("object")]
    TypeObject,
    #[token("array")]
    TypeArray,
    #[token("any")]
    TypeAny,
    //-------------------------
    // Literals
    //-------------------------
    #[token("null")]
    Null,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[regex(r"-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?", |lex| lex.slice().parse::<f64>().unwrap())]
    Number(f64),
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_owned())]
    #[regex(r#"'([^'\\]|\\.)*'"#, |lex| lex.slice().to_owned())]
    String(String),
    //-------------------------
    // Names and Identifiers
    //-------------------------
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_owned())]
    Identifier(String),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
