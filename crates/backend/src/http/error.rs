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
    Conflict,
    DuplicateSession,
    NotFound,
    Unavailable,
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
impl IntoResponse for ApiError {
    /// Returns stable machine-readable categories without sensitive implementation details.
    fn into_response(self) -> Response {
        let (status, code, detail) = match self {
            Self::Input(error) => {
                let status = match error.kind {
                    InputErrorKind::Limit => StatusCode::PAYLOAD_TOO_LARGE,
                    InputErrorKind::Syntax | InputErrorKind::Field => StatusCode::BAD_REQUEST,
                };
                return (status, Json(error)).into_response();
            }
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                "/auth/login",
            ),
            Self::Forbidden => (StatusCode::FORBIDDEN, "origin_rejected", ""),
            Self::Conflict => (StatusCode::CONFLICT, "identity_or_request_conflict", ""),
            Self::DuplicateSession => (StatusCode::CONFLICT, "session_already_exists", ""),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found", ""),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "identity_unavailable", ""),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "persistence_failed", ""),
        };
        (
            status,
            Json(serde_json::json!({"error":code,"login":detail})),
        )
            .into_response()
    }
}
