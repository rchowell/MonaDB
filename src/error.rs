use lalrpop_util::ParseError;
use crate::lexer::Token;

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Error {
    IoError(String),
    InternalError(String),
    Storage(String),
    SyntaxError(Hint),
    Unsupported(String),
    #[default]
    Unknown,
    UnknownTable(String),
    UnknownFunction(String),
}

impl Error {

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
            _ => format!("{:?}", self),
        }

    }
    
}

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
            },
            ParseError::ExtraToken { token } => {
                let hint = Hint {
                    message: "extra token".to_string(),
                    location: token.0,
                    expected: vec![],
                };
                Error::SyntaxError(hint)
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub message: String,
    pub location: usize,
    pub expected: Vec<String>,
}
