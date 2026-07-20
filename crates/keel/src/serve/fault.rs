use crate::adapt::Error;
use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

pub(crate) struct Fault {
    pub(crate) status: StatusCode,
    pub(crate) note: String,
}

impl Fault {
    pub(crate) fn miss(name: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            note: format!("missing resource: {name}"),
        }
    }

    pub(crate) fn bad(note: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            note,
        }
    }
}

impl From<Error> for Fault {
    fn from(err: Error) -> Self {
        match err {
            Error::Missing(name) => Self::miss(&name),
            Error::Adapt(note) if note.starts_with("missing row") => Self {
                status: StatusCode::NOT_FOUND,
                note,
            },
            Error::Adapt(note) if note.starts_with("missing tie") => Self {
                status: StatusCode::NOT_FOUND,
                note,
            },
            Error::Adapt(note) if note.starts_with("live ties remain") => Self {
                status: StatusCode::CONFLICT,
                note,
            },
            Error::Adapt(note) if note.starts_with("live ref exists") => Self {
                status: StatusCode::CONFLICT,
                note,
            },
            Error::Adapt(note) if note.ends_with(" taken") => Self {
                status: StatusCode::CONFLICT,
                note,
            },
            Error::Adapt(note) if note.starts_with("refused") => Self {
                status: StatusCode::FORBIDDEN,
                note,
            },
            Error::Adapt(note) if note.starts_with("left not live") => Self {
                status: StatusCode::BAD_REQUEST,
                note,
            },
            Error::Adapt(note) if note.starts_with("right not live") => Self {
                status: StatusCode::BAD_REQUEST,
                note,
            },
            Error::Adapt(note) => Self {
                status: StatusCode::BAD_REQUEST,
                note,
            },
        }
    }
}

impl IntoResponse for Fault {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(json!({ "error": self.note }))).into_response()
    }
}
