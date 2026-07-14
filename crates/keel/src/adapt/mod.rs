pub mod db;
pub mod http;

pub use db::Db;
pub use http::Http;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Missing(String),
    Adapt(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "missing resource: {name}"),
            Self::Adapt(note) => write!(f, "adapt: {note}"),
        }
    }
}

impl std::error::Error for Error {}
