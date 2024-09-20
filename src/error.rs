use lalrpop_util::{lexer::Token, ParseError};

/// TODO DOCUMENTATION
#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    TableNotFound(String),
    SyntaxError(String),
    Unsupported(String),
    Unknown(String),
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return Err(crate::error::Error::Unknown(msg.to_string()))
    }}
}

impl From<&str> for Error {
    fn from(s: &str) -> Error {
        Error::Unknown(s.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IoError(e)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Error {
        Error::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::SyntaxError(e.to_string())
    }
}

impl From<ParseError<usize, Token<'_>, Error>> for Error {
    fn from(e: ParseError<usize, Token<'_>, Error>) -> Error {
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


fn err_syntax(message: &str) -> Error {
    Error::SyntaxError(message.to_string())
}
