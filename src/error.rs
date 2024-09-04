/// TODO DOCUMENTATION
#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    TableNotFound(String),
    SyntaxError(String),
    Unsupported(String),
    Unknown(String),
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

impl From<sqlparser::parser::ParserError> for Error {
    fn from(e: sqlparser::parser::ParserError) -> Error {
        match e {
            sqlparser::parser::ParserError::TokenizerError(e) => Error::SyntaxError(e.to_string()),
            sqlparser::parser::ParserError::ParserError(e) => Error::SyntaxError(e.to_string()),
            sqlparser::parser::ParserError::RecursionLimitExceeded => Error::Unknown("Recursion limit exceeded".to_string()),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::SyntaxError(e.to_string())
    }
}
