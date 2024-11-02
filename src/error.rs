use lalrpop_util::ParseError;
use crate::lexer::Token;

/// TODO DOCUMENTATION
#[derive(Default, Debug, Clone, PartialEq)]
pub enum Error {
    IoError(String),
    InternalError(String),
    SyntaxError(String),
    Unsupported(String),
    #[default]
    Unknown,
    UnknownTable(String),
    UnknownRoutine(String),
}

// TODO impl Error

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return Err(crate::error::Error::Unknown(msg.to_string()))
    }}
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IoError(e.to_string())
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Error {
        Error::InternalError(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::SyntaxError(e.to_string())
    }
}

impl From<ParseError<usize, Token, Error>> for Error {
    fn from(e: ParseError<usize, Token, Error>) -> Error {
        match e {
            ParseError::User { error } => error,
            ParseError::UnrecognizedEof { .. } => err_syntax("unexpected EOF"),
            ParseError::InvalidToken { location } => {
                // unexpected
                err_syntax(&format!("unexpected token at {:?}", location))
            }
            ParseError::UnrecognizedToken { token, expected } => {
                // expected something different
                let expected = expected.join(", ");
                err_syntax(&format!("unexpected token at {:?}, expected {}", token.0, expected))
            },
            ParseError::ExtraToken { token } => {
                // unexpected
                err_syntax(&format!("unexpected token {:?} at {:?}", token.1, token.0))
            },
        }
    }
}

pub fn err_syntax(message: &str) -> Error {
    Error::SyntaxError(message.to_string())
}

pub fn err_unknown_routine(sym: &str) -> Error {
    Error::UnknownRoutine(format!("unknown routine: {}", sym))
}