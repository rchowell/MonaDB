/// TODO DOCUMENTATION
#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
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
