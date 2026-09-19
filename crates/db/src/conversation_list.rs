use crate::{Database, DbError, OwnerScope};
use palace_domain::Source;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

/// Library metadata includes a path head so a branched conversation opens unambiguously.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub title: String,
    pub source: Source,
    pub session_id: String,
    pub occurred_at: i64,
    pub head_message_id: Uuid,
    pub message_count: i64,
}
impl Database {
    /// Selects the most recent occurrence per conversation inside the authenticated owner scope.
    pub async fn list_conversations(
        &self,
        owner: OwnerScope,
    ) -> Result<Vec<ConversationSummary>, DbError> {
        let rows = sqlx::query("SELECT c.id,c.title,c.source,c.session_id,i.head_message_id,i.message_count,(extract(epoch FROM i.occurred_at)*1000)::bigint AS occurred_at FROM conversation c JOIN LATERAL (SELECT occurred_at,head_message_id,message_count FROM conversation_import WHERE owner_id=c.owner_id AND conversation_id=c.id ORDER BY occurred_at DESC,id DESC LIMIT 1) i ON true WHERE c.owner_id=$1 ORDER BY i.occurred_at DESC,c.id DESC")
            .bind(owner.id()).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(ConversationSummary {
                    id: row.try_get("id")?,
                    title: row.try_get("title")?,
                    source: serde_json::from_value(serde_json::Value::String(
                        row.try_get("source")?,
                    ))
                    .map_err(|_| DbError::Conflict)?,
                    session_id: row.try_get("session_id")?,
                    occurred_at: row.try_get("occurred_at")?,
                    head_message_id: row.try_get("head_message_id")?,
                    message_count: row.try_get("message_count")?,
                })
            })
            .collect()
    }
}
