use logos::{Logos, SpannedIter};
use std::fmt;

use crate::error::Error;

pub type Spanned<Tok, Loc, Error> = Result<(Loc, Tok, Loc), Error>;

pub struct Lexer<'input> {
    tokens: SpannedIter<'input, Token>,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            tokens: Token::lexer(input).spanned(),
        }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Spanned<Token, usize, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.tokens
            .next()
            .map(|(token, span)| {
                Ok((span.start, token?, span.end))
            })
    }
}

#[derive(Logos, Clone, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+", error = Error)] // Ignore this regex pattern between tokens
pub enum Token {
    //-------------------------
    // Symbols
    //-------------------------
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(".")]
    Period,
    #[token(";")]
    SemiColon,
    #[token("*")]
    Star,
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
    #[token("as")]
    As,
    #[token("create")]
    Create,
    #[token("delete")]
    Delete,
    #[token("drop")]
    Drop,
    #[token("from")]
    From,
    #[token("insert")]
    Insert,
    #[token("into")]
    Into,
    #[token("select")]
    Select,
    #[token("table")]
    Table,
    //-------------------------
    // Types
    //-------------------------
    #[token("bool")]
    TypeBool,
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
    #[regex(r#""([^"\\]|\\["\\bnfrt]|u[a-fA-F0-9]{4})*""#, |lex| lex.slice().to_owned())]
    String(String),
    //-------------------------
    // Names and Identifiers
    //-------------------------
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().trim_matches('"').to_owned())]
    Identifier(String),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
