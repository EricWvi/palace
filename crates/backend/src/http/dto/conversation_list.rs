use super::common::Source;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// One page of the 摘星 conversation list.
#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationPage {
    /// Paths, most recently imported or appended first.
    pub items: Vec<ConversationListItem>,
    /// Opaque token for the next page; null on the last page.
    #[schema(required = true)]
    pub next_cursor: Option<String>,
    /// Number of paths the owner has, regardless of `q`.
    pub total: i64,
}
/// One path of a conversation, shown under its own title.
#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationListItem {
    /// Path id, equal to the id of the path's conversation moment.
    pub id: Uuid,
    pub conversation_id: Uuid,
    /// Title of this path.
    pub title: String,
    /// Source of the whole conversation.
    pub source: Source,
    /// Unix epoch milliseconds of the path's last import or append.
    pub updated_at: i64,
}
impl From<palace_db::ConversationPage> for ConversationPage {
    /// Encodes the cursor as the opaque token clients send back unchanged.
    fn from(value: palace_db::ConversationPage) -> Self {
        Self {
            items: value
                .items
                .into_iter()
                .map(|item| ConversationListItem {
                    id: item.id,
                    conversation_id: item.conversation_id,
                    title: item.title,
                    source: item.source.into(),
                    updated_at: item.updated_at,
                })
                .collect(),
            next_cursor: value.next_cursor.map(|cursor| cursor.to_string()),
            total: value.total,
        }
    }
}
