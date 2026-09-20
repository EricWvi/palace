use super::common::{Role, Source};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub(crate) struct Owner {
    pub id: Uuid,
    pub email: String,
    pub identity_id: Uuid,
}
impl From<palace_db::Owner> for Owner {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_db::Owner) -> Self {
        Self {
            id: value.id,
            email: value.email,
            identity_id: value.identity_id,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct Conversation {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub source: Source,
}
impl From<palace_domain::Conversation> for Conversation {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_domain::Conversation) -> Self {
        Self {
            id: value.id,
            owner_id: value.owner_id,
            title: value.title,
            source: value.source.into(),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct Message {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub conversation_id: Uuid,
    /// Always present; root messages carry null.
    #[schema(required = true)]
    pub parent_message_id: Option<Uuid>,
    pub role: Role,
    pub content: String,
    /// Ordering integer, not a timestamp; i64 values are not generally lossless JS numbers.
    pub created_order: i64,
}
impl From<palace_domain::Message> for Message {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_domain::Message) -> Self {
        Self {
            id: value.id,
            owner_id: value.owner_id,
            conversation_id: value.conversation_id,
            parent_message_id: value.parent_message_id,
            role: value.role.into(),
            content: value.content,
            created_order: value.created_order,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationSummary {
    pub id: Uuid,
    pub title: String,
    pub source: Source,
    pub session_ids: Vec<String>,
    pub path_count: i64,
    pub path_id: Uuid,
    /// Unix epoch milliseconds selected by the user.
    pub occurred_at: i64,
    pub head_message_id: Uuid,
    pub message_count: i64,
}
impl From<palace_db::ConversationSummary> for ConversationSummary {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_db::ConversationSummary) -> Self {
        Self {
            id: value.id,
            title: value.title,
            source: value.source.into(),
            session_ids: value.session_ids,
            path_count: value.path_count,
            path_id: value.path_id,
            occurred_at: value.occurred_at,
            head_message_id: value.head_message_id,
            message_count: value.message_count,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ImportResult {
    pub import_id: Uuid,
    pub conversation_id: Uuid,
    pub path_id: Uuid,
    pub head_message_id: Uuid,
    pub created: usize,
    pub reused: usize,
}
impl From<palace_db::ImportResult> for ImportResult {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_db::ImportResult) -> Self {
        Self {
            import_id: value.import_id,
            conversation_id: value.conversation_id,
            path_id: value.path_id,
            head_message_id: value.head_message_id,
            created: value.created,
            reused: value.reused,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationPath {
    pub id: Uuid,
    pub session_id: String,
    pub head_message_id: Uuid,
    /// Unix epoch milliseconds selected by the user.
    pub occurred_at: i64,
    /// Unix epoch milliseconds assigned by the database.
    pub created_at: i64,
    /// Unix epoch milliseconds assigned by the database.
    pub updated_at: i64,
    pub message_count: i64,
    /// A controlled external link, never fetched by the server.
    pub original_link: String,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationDetail {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
    pub paths: Vec<ConversationPath>,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationMetadataResponse {
    pub id: Uuid,
    pub title: String,
    pub source: Source,
}
