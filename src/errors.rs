/// TODO DOCUMENTATION
pub type RhoResult<T> = Result<T, RhoError>;

/// TODO DOCUMENTATION
#[derive(Debug)]
pub enum RhoError {
    IoError(std::io::Error),
}

impl From<std::io::Error> for RhoError {
    fn from(e: std::io::Error) -> RhoError {
        RhoError::IoError(e)
    }
}