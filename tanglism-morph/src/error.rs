#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", &self.0)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error {
        Error(format!("{}", err))
    }
}
