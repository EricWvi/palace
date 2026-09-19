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
    pub session_ids: Vec<String>,
    pub path_count: i64,
    pub path_id: Uuid,
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
        let rows = sqlx::query("SELECT c.id,c.title,c.source,p.id AS path_id,p.head_message_id,(SELECT count(*) FROM message m WHERE m.conversation_id=c.id) AS message_count,(SELECT count(*) FROM conversation_path cp WHERE cp.conversation_id=c.id) AS path_count,(SELECT array_agg(session_id ORDER BY session_id) FROM conversation_path cp WHERE cp.conversation_id=c.id) AS session_ids,(SELECT (extract(epoch FROM max(occurred_at))*1000)::bigint FROM conversation_path cp WHERE cp.conversation_id=c.id) AS occurred_at FROM conversation c JOIN LATERAL (SELECT id,head_message_id FROM conversation_path WHERE owner_id=c.owner_id AND conversation_id=c.id ORDER BY updated_at DESC,id DESC LIMIT 1) p ON true WHERE c.owner_id=$1 ORDER BY occurred_at DESC,c.id DESC")
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
                    session_ids: row.try_get("session_ids")?,
                    path_count: row.try_get("path_count")?,
                    path_id: row.try_get("path_id")?,
                    occurred_at: row.try_get("occurred_at")?,
                    head_message_id: row.try_get("head_message_id")?,
                    message_count: row.try_get("message_count")?,
                })
            })
            .collect()
    }
}
