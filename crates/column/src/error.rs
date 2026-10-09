use std::fmt;

#[derive(Debug)]
pub struct Error(String);

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<parquet::errors::ParquetError> for Error {
    fn from(error: parquet::errors::ParquetError) -> Self {
        Self(error.to_string())
    }
}

impl From<arrow_schema::ArrowError> for Error {
    fn from(error: arrow_schema::ArrowError) -> Self {
        Self(error.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}
