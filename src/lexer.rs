//! Token definitions and the spanned lexer (logos).
//!
//! [`SqlLexer`] wraps a logos DFA and yields `(start, token, end)` triples for
//! the LALRPOP parser, which consumes them through its `extern` block.

use logos::{Logos, SpannedIter};
use std::fmt;

use crate::error::{Error, Hint};

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
    #[token("?")]
    Question,
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
    #[token("all")]
    All,
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
    #[token("copy")]
    Copy,
    #[token("create")]
    Create,
    #[token("delete")]
    Delete,
    #[token("desc")]
    Desc,
    #[token("drop")]
    Drop,
    #[token("exists")]
    Exists,
    #[token("group")]
    Group,
    #[token("having")]
    Having,
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
    #[token("to")]
    To,
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
    /// A quoted string literal — `'…'` or `"…"` — decoded to its content: the
    /// delimiters are stripped and backslash escapes resolved (see
    /// [`decode_string_literal`]). An unknown escape is a syntax error.
    #[regex(r#""([^"\\]|\\.)*""#, |lex| decode_string_literal(lex.slice(), lex.span().start))]
    #[regex(r#"'([^'\\]|\\.)*'"#, |lex| decode_string_literal(lex.slice(), lex.span().start))]
    String(String),
    //-------------------------
    // Query Parameters
    //-------------------------
    /// A numbered parameter `$N` (1-based), carrying its raw digits (without the
    /// `$`). The index is parsed later so an out-of-range value surfaces as a
    /// bind error rather than an opaque lexer failure. Logos prefers this over
    /// the bare `$`/`Dollar` token by longest match.
    #[regex(r"\$[0-9]+", |lex| lex.slice()[1..].to_owned())]
    NumberedParam(String),
    /// A named parameter `$name`, carrying the name (without the `$`).
    #[regex(r"\$[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice()[1..].to_owned())]
    NamedParam(String),
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

/// Builds the syntax error raised for a malformed escape at byte offset `at`.
fn bad_escape(at: usize) -> Error {
    Error::SyntaxError(Hint {
        message: "invalid string escape".to_string(),
        location: at,
        expected: vec![],
    })
}

/// Decodes a quoted string literal token to its content.
///
/// Strips the surrounding `'`/`"` delimiters (the regex guarantees a matching
/// pair) and resolves backslash escapes: `\" \' \\ \/ \n \t \r \b \f` and
/// `\uXXXX` (four hex digits). Any other escape — or a truncated `\u` — is a
/// syntax error located at the token's start offset `at`.
fn decode_string_literal(raw: &str, at: usize) -> Result<String, Error> {
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next().ok_or_else(|| bad_escape(at))? {
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{08}'),
            'f' => out.push('\u{0C}'),
            'u' => {
                let mut code = 0u32;
                for _ in 0..4 {
                    let digit = chars
                        .next()
                        .and_then(|h| h.to_digit(16))
                        .ok_or_else(|| bad_escape(at))?;
                    code = code * 16 + digit;
                }
                out.push(char::from_u32(code).ok_or_else(|| bad_escape(at))?);
            }
            _ => return Err(bad_escape(at)),
        }
    }
    Ok(out)
}
