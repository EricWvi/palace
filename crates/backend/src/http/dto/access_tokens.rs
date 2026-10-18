use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// An access token as listed; only the masked form ever appears after creation.
#[derive(Serialize, ToSchema)]
pub(crate) struct AccessToken {
    pub id: Uuid,
    pub note: String,
    /// `palace_pat_`, the first and last four random characters, and `*` for every other one.
    pub masked: String,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    /// Unix epoch milliseconds of the last recorded use, at most a minute stale; null if never used.
    #[schema(required = true)]
    pub last_used_at: Option<i64>,
}
impl From<palace_db::AccessToken> for AccessToken {
    /// Lists every wire field so a new persisted field cannot leak by accident.
    fn from(value: palace_db::AccessToken) -> Self {
        Self {
            id: value.id,
            note: value.note,
            masked: value.masked,
            created_at: value.created_at,
            last_used_at: value.last_used_at,
        }
    }
}

/// A newly created token, the only response that ever carries its full value.
#[derive(Serialize, ToSchema)]
pub(crate) struct CreatedAccessToken {
    pub access_token: AccessToken,
    /// The full token for `Authorization: Bearer`; it cannot be retrieved again.
    pub token: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AccessTokenRequest {
    /// What the token is used for; trimmed, 1 to 100 characters.
    pub note: String,
}
