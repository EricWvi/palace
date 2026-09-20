use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorCode {
    AuthenticationRequired,
    OriginRejected,
    IdentityOrRequestConflict,
    SessionAlreadyExists,
    NotFound,
    IdentityUnavailable,
    PersistenceFailed,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct ErrorResponse {
    pub error: ErrorCode,
    /// /auth/login for authentication_required; the empty string for other errors. Always present.
    pub login: &'static str,
}
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputErrorKind {
    Syntax,
    Field,
    Limit,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct InputErrorResponse {
    pub kind: InputErrorKind,
    pub path: String,
    pub message: String,
}
impl From<palace_domain::InputError> for InputErrorResponse {
    /// Keeps actionable error categories and locations without exposing internal errors.
    fn from(value: palace_domain::InputError) -> Self {
        let kind = match value.kind {
            palace_domain::InputErrorKind::Syntax => InputErrorKind::Syntax,
            palace_domain::InputErrorKind::Field => InputErrorKind::Field,
            palace_domain::InputErrorKind::Limit => InputErrorKind::Limit,
        };
        Self {
            kind,
            path: value.path,
            message: value.message,
        }
    }
}
