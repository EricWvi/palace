use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Source {
    Chatgpt,
    Gemini,
    Grok,
}
impl From<palace_domain::Source> for Source {
    /// Keeps the public vocabulary exhaustive when the domain gains a source.
    fn from(value: palace_domain::Source) -> Self {
        match value {
            palace_domain::Source::Chatgpt => Self::Chatgpt,
            palace_domain::Source::Gemini => Self::Gemini,
            palace_domain::Source::Grok => Self::Grok,
        }
    }
}
impl From<Source> for palace_domain::Source {
    /// Leaves source-specific business validation in the domain.
    fn from(value: Source) -> Self {
        match value {
            Source::Chatgpt => Self::Chatgpt,
            Source::Gemini => Self::Gemini,
            Source::Grok => Self::Grok,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    User,
    Assistant,
}
impl From<palace_domain::Role> for Role {
    /// Preserves role spelling without accepting unsupported domain variants silently.
    fn from(value: palace_domain::Role) -> Self {
        match value {
            palace_domain::Role::User => Self::User,
            palace_domain::Role::Assistant => Self::Assistant,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct Deleted {
    pub id: Uuid,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct LogoutResponse {
    /// True after the local revocation has committed, including when external revocation is pending.
    pub logged_out: bool,
}

/// Canonical decimal string in 0..=9223372036854775807; the domain checks the numeric upper bound.
#[derive(Deserialize, Serialize, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, pattern = "^(0|[1-9][0-9]*)$")]
pub(crate) struct Cursor(pub palace_domain::ServerVersion);
