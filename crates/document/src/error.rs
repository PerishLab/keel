#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    Missing,
    Kind,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Model(String),
    Syntax,
    Collection(String),
    Value {
        parts: Vec<String>,
        failure: Failure,
    },
    Document(radicula_document::Error),
    Patch {
        index: usize,
        error: radicula_document::Error,
    },
    Query(String),
    Missing,
    Conflict {
        expected: String,
        actual: String,
    },
    Store(keel::adapt::Store),
    Engine(keel::adapt::Error),
    Carrier(String),
    Foreign(String),
    Binding(String),
    Io(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Model(_) => "model",
            Self::Syntax => "syntax",
            Self::Collection(_) => "collection",
            Self::Value { .. } => "value",
            Self::Document(_) => "document",
            Self::Patch { .. } => "patch",
            Self::Query(_) => "query",
            Self::Missing => "missing",
            Self::Conflict { .. } => "conflict",
            Self::Store(_) => "store",
            Self::Engine(_) => "engine",
            Self::Carrier(_) => "carrier",
            Self::Foreign(_) => "foreign",
            Self::Binding(_) => "binding",
            Self::Io(_) => "io",
        }
    }

    pub(crate) fn keel(held: keel::adapt::Error) -> Self {
        match held {
            keel::adapt::Error::Store(store) => Self::Store(store),
            held => Self::Engine(held),
        }
    }

    pub(crate) fn carrier(reason: impl Into<String>) -> Self {
        Self::Carrier(reason.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Model(reason) => write!(out, "invalid document model: {reason}"),
            Self::Syntax => out.write_str("invalid document model syntax"),
            Self::Collection(name) => write!(out, "unknown collection {name}"),
            Self::Value { parts, failure } => write!(out, "{failure:?} at segments {parts:?}"),
            Self::Document(held) => write!(out, "document body refused: {}", held.code()),
            Self::Patch { index, error } => {
                write!(out, "operation {index} refused: {}", error.code())
            }
            Self::Query(reason) => write!(out, "query refused: {reason}"),
            Self::Missing => out.write_str("document or payload is absent"),
            Self::Conflict { .. } => out.write_str("expected revision is not the visible head"),
            Self::Store(held) => write!(out, "store refused: {held}"),
            Self::Engine(held) => write!(out, "engine refused: {held}"),
            Self::Carrier(reason) => write!(out, "payload carrier refused: {reason}"),
            Self::Foreign(reason) => write!(out, "root binding refused: {reason}"),
            Self::Binding(reason) => write!(out, "document binding: {reason}"),
            Self::Io(_) => out.write_str("document I/O fault"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<keel::adapt::Error> for Error {
    fn from(error: keel::adapt::Error) -> Self {
        Self::keel(error)
    }
}

impl From<radicula_document::Fault> for Error {
    fn from(fault: radicula_document::Fault) -> Self {
        Self::Patch {
            index: fault.index,
            error: fault.error,
        }
    }
}
