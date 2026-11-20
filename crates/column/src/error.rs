use std::fmt;

#[derive(Debug)]
pub struct Error {
    message: String,
    refused: bool,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            refused: false,
        }
    }

    pub(crate) fn deny(message: &str) -> Self {
        Self {
            message: message.to_owned(),
            refused: true,
        }
    }

    pub fn refused(&self) -> bool {
        self.refused
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<parquet::errors::ParquetError> for Error {
    fn from(error: parquet::errors::ParquetError) -> Self {
        Self::new(error.to_string())
    }
}

impl From<arrow_schema::ArrowError> for Error {
    fn from(error: arrow_schema::ArrowError) -> Self {
        Self::new(error.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<keel::adapt::Error> for Error {
    fn from(error: keel::adapt::Error) -> Self {
        Self::new(error.to_string())
    }
}
