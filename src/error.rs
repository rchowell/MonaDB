//! The crate-wide error and result types, plus the `error!` early-return macro.

use crate::lexer::Token;
use lalrpop_util::ParseError;

/// Top-level result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Top-level error type — every fallible path returns one of these.
#[derive(Default, Debug, Clone, PartialEq)]
pub enum Error {
    /// JSON (de)serialization failed.
    JsonError(String),
    /// An I/O failure.
    IoError(String),
    /// An internal invariant was violated (a bug, not bad input).
    InternalError(String),
    /// The underlying LMDB storage returned an error.
    Storage(String),
    /// The input failed to lex or parse.
    SyntaxError(Hint),
    /// A recognized but unimplemented feature.
    Unsupported(String),
    /// A transaction could not be started, committed, or aborted.
    Transaction(String),
    /// An unspecified error (the `Default`).
    #[default]
    Unknown,
    /// A referenced table does not exist.
    UnknownTable(String),
    /// A called function/operator is undefined or has the wrong arity.
    UnknownFunction(String),
    /// A table reference was used before being bound.
    UnboundTable(String),
    /// Name resolution / binding failed.
    BindError(String),
    /// A value violated the schema (missing/mistyped key, non-object row).
    Schema(String),
}

impl Error {
    /// Renders the error against the source `input`. A syntax error shows the
    /// offending line with a caret and an "expected" hint; others use `Debug`.
    pub fn pretty(&self, input: &str) -> String {
        match self {
            Error::SyntaxError(hint) => {
                let mut result = String::new();
                let lines: Vec<&str> = input.lines().collect();
                let line_number = input[..hint.location].lines().count();
                let column_number = hint.location - input[..hint.location].rfind('\n').unwrap_or(0);

                // header
                result.push('\n');
                result.push_str("error: ");
                result.push_str(&hint.message);
                result.push('\n');
                result.push_str("  │\n");

                // location
                if let Some(line) = lines.get(line_number.saturating_sub(1)) {
                    result.push_str("  │ ");
                    result.push_str(line);
                    result.push('\n');
                    result.push_str("  │ ");
                    result.push_str(&" ".repeat(column_number));
                    result.push('^');
                    result.push('\n');
                }

                // expected
                if !hint.expected.is_empty() {
                    result.push_str("  └─ hint: expected ");
                    result.push_str(&hint.expected.join(", "));
                }
                result.push('\n');
                result
            }
            _ => format!("{self:?}"),
        }
    }
}

/// Returns early with an [`Error::InternalError`] formatted from the arguments.
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return Err($crate::error::Error::InternalError(msg.to_string()))
    }}
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IoError(e.to_string())
    }
}

impl From<std::fmt::Error> for Error {
    fn from(e: std::fmt::Error) -> Self {
        Error::InternalError(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::IoError(e.to_string())
    }
}

impl From<heed::Error> for Error {
    fn from(e: heed::Error) -> Error {
        Error::Storage(e.to_string())
    }
}

impl From<ParseError<usize, Token, Error>> for Error {
    fn from(e: ParseError<usize, Token, Error>) -> Error {
        match e {
            ParseError::User { error } => error,
            ParseError::UnrecognizedEof { location, expected } => {
                let hint = Hint {
                    message: "unexpected EOF".to_string(),
                    location,
                    expected,
                };
                Error::SyntaxError(hint)
            }
            ParseError::InvalidToken { location } => {
                let hint = Hint {
                    message: "unexpected token".to_string(),
                    location,
                    expected: vec![],
                };
                Error::SyntaxError(hint)
            }
            ParseError::UnrecognizedToken { token, expected } => {
                let hint = Hint {
                    message: "unrecognized token".to_string(),
                    location: token.0,
                    expected,
                };
                Error::SyntaxError(hint)
            }
            ParseError::ExtraToken { token } => {
                let hint = Hint {
                    message: "extra token".to_string(),
                    location: token.0,
                    expected: vec![],
                };
                Error::SyntaxError(hint)
            }
        }
    }
}

/// A syntax-error hint: the message, the byte offset in the input, and the set
/// of tokens the parser expected there.
#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub message: String,
    pub location: usize,
    pub expected: Vec<String>,
}
