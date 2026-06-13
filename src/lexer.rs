//! Token definitions and the spanned lexer (logos).
//!
//! [`SqlLexer`] wraps a logos DFA and yields `(start, token, end)` triples for
//! the LALRPOP parser, which consumes them through its `extern` block.

use logos::{Logos, SpannedIter};
use std::fmt;

use crate::error::Error;

/// A lexer item: a token bracketed by its start/end byte offsets, or an error.
pub type Spanned<Tok, Loc, Error> = Result<(Loc, Tok, Loc), Error>;

/// The SQL lexer: a spanned token stream over the input.
pub struct SqlLexer<'input> {
    tokens: SpannedIter<'input, Token>,
}

impl<'input> SqlLexer<'input> {
    /// Creates a lexer over the given input.
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

/// A lexical token. Symbol, keyword, and type variants carry no data; literal
/// and identifier variants carry their source text.
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
    #[token("asc")]
    Asc,
    #[token("at")]
    At,
    #[token("between")]
    Between,
    #[token("by")]
    By,
    #[token("clear")]
    Clear,
    #[token("create")]
    Create,
    #[token("delete")]
    Delete,
    #[token("desc")]
    Desc,
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
    #[token("order")]
    Order,
    #[token("pivot")]
    Pivot,
    #[token("select")]
    Select,
    #[token("table")]
    Table,
    #[token("unknown")]
    Unknown,
    #[token("unpivot")]
    Unpivot,
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
    /// A numeric literal, as its raw source text.
    #[regex(r"-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?", |lex| lex.slice().to_owned())]
    Number(String),
    /// A quoted string literal, with quotes still attached.
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_owned())]
    #[regex(r#"'([^'\\]|\\.)*'"#, |lex| lex.slice().to_owned())]
    String(String),
    //-------------------------
    // Names and Identifiers
    //-------------------------
    /// An identifier (table or field name).
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_owned())]
    Identifier(String),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
