use super::dto::{ErrorCode, ErrorResponse, InputErrorResponse};
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use palace_db::{DbError, ProviderError, SessionError};
use palace_domain::{InputError, InputErrorKind};

pub(super) enum ApiError {
    Input(InputError),
    Unauthorized,
    Forbidden,
    SessionRequired,
    Conflict,
    DuplicateSession,
    StaleHead,
    NotFound,
    Unavailable,
    SearchUnavailable,
    MediaUnavailable,
    Internal,
}
impl From<InputError> for ApiError {
    /// Preserves actionable input category and location in the public error contract.
    fn from(value: InputError) -> Self {
        Self::Input(value)
    }
}
impl From<DbError> for ApiError {
    /// Does not leak SQL or credentials through database error messages.
    fn from(value: DbError) -> Self {
        match value {
            DbError::Input(e) => Self::Input(e),
            DbError::Conflict => Self::Conflict,
            DbError::DuplicateSession => Self::DuplicateSession,
            DbError::StaleHead => Self::StaleHead,
            DbError::NotFound => Self::NotFound,
            DbError::Disabled => Self::Unauthorized,
            DbError::Storage(_) | DbError::Migration(_) => Self::Internal,
        }
    }
}
impl From<SessionError> for ApiError {
    /// Keeps unavailable identity verification distinct from an invalid browser credential.
    fn from(value: SessionError) -> Self {
        match value {
            SessionError::Database(e) => e.into(),
            SessionError::Unauthorized | SessionError::Integrity => Self::Unauthorized,
            SessionError::Unavailable => Self::Unavailable,
        }
    }
}
impl From<ProviderError> for ApiError {
    /// Preserves temporary provider outages for clients to retry.
    fn from(value: ProviderError) -> Self {
        match value {
            ProviderError::Unavailable => Self::Unavailable,
            ProviderError::Rejected => Self::Unauthorized,
        }
    }
}
impl From<palace_notes::NotesError> for ApiError {
    /// A failed search is the server's fault, never the reader's input; partial results are not sent.
    fn from(value: palace_notes::NotesError) -> Self {
        match value {
            palace_notes::NotesError::Input(e) => Self::Input(e),
            palace_notes::NotesError::NotFound => Self::NotFound,
            palace_notes::NotesError::SearchFailed => Self::SearchUnavailable,
        }
    }
}
impl From<palace_media::MediaError> for ApiError {
    /// Object storage outages and missing configuration are retryable unavailability, not input errors.
    fn from(value: palace_media::MediaError) -> Self {
        match value {
            palace_media::MediaError::Database(e) => e.into(),
            palace_media::MediaError::Store(_) => Self::MediaUnavailable,
        }
    }
}
impl IntoResponse for ApiError {
    /// Returns stable machine-readable categories without sensitive implementation details.
    fn into_response(self) -> Response {
        let (status, code, detail) = match self {
            Self::Input(error) => {
                let status = match error.kind {
                    InputErrorKind::Limit => StatusCode::PAYLOAD_TOO_LARGE,
                    InputErrorKind::Syntax | InputErrorKind::Field => StatusCode::BAD_REQUEST,
                };
                return (status, Json(InputErrorResponse::from(error))).into_response();
            }
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                ErrorCode::AuthenticationRequired,
                "/auth/login",
            ),
            Self::Forbidden => (StatusCode::FORBIDDEN, ErrorCode::OriginRejected, ""),
            Self::SessionRequired => (StatusCode::FORBIDDEN, ErrorCode::SessionRequired, ""),
            Self::Conflict => (
                StatusCode::CONFLICT,
                ErrorCode::IdentityOrRequestConflict,
                "",
            ),
            Self::DuplicateSession => (StatusCode::CONFLICT, ErrorCode::SessionAlreadyExists, ""),
            Self::StaleHead => (StatusCode::CONFLICT, ErrorCode::PathHeadMoved, ""),
            Self::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound, ""),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::IdentityUnavailable,
                "",
            ),
            Self::SearchUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::SearchUnavailable,
                "",
            ),
            Self::MediaUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::MediaStorageUnavailable,
                "",
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::PersistenceFailed,
                "",
            ),
        };
        (
            status,
            Json(ErrorResponse {
                error: code,
                login: detail,
            }),
        )
            .into_response()
    }
}
