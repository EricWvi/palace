use crate::{Database, DbError, OwnerScope};
use palace_domain::{Conversation, Message, Role, SessionId, Source, read_path};
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

/// A source session remains identifiable even when it ends inside another path.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ConversationPath {
    pub id: Uuid,
    pub session_id: SessionId,
    pub head_message_id: Uuid,
    pub occurred_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: i64,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ConversationDetail {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
    pub paths: Vec<ConversationPath>,
}
impl Database {
    /// Replaces editable metadata while preserving every Palace-owned tree identity.
    pub async fn update_conversation_metadata(
        &self,
        owner: OwnerScope,
        id: Uuid,
        title: &str,
        source: Source,
    ) -> Result<(), DbError> {
        palace_domain::validate_title(title)?;
        let mut tx = self.begin_write().await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM conversation WHERE owner_id=$1 AND id=$2)",
        )
        .bind(owner.id())
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(DbError::NotFound);
        }
        // The advisory write lock also covers imports, so this explicit check can return the
        // domain conflict without racing a new path before the cascading update commits.
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM conversation_path owned JOIN conversation_path occupied ON occupied.owner_id=owned.owner_id AND occupied.source=$3 AND occupied.session_id=owned.session_id WHERE owned.owner_id=$1 AND owned.conversation_id=$2 AND occupied.conversation_id<>$2)",
        )
        .bind(owner.id())
        .bind(id)
        .bind(source.as_str())
        .fetch_one(&mut *tx)
        .await?;
        if duplicate {
            return Err(DbError::DuplicateSession);
        }
        sqlx::query("UPDATE conversation SET title=$3,source=$4 WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .bind(title)
            .bind(source.as_str())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    /// Reads metadata and a deterministic tree under one owner scope.
    pub async fn conversation_detail(
        &self,
        owner: OwnerScope,
        id: Uuid,
    ) -> Result<ConversationDetail, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query("SELECT title,source FROM conversation WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(DbError::NotFound)?;
        let source: Source =
            serde_json::from_value(serde_json::Value::String(row.try_get("source")?))
                .map_err(|_| DbError::Conflict)?;
        let conversation = Conversation {
            id,
            owner_id: owner.id(),
            title: row.try_get("title")?,
            source,
        };
        let rows = sqlx::query("SELECT id,parent_message_id,role,content,created_order FROM message WHERE owner_id=$1 AND conversation_id=$2 ORDER BY created_order,id").bind(owner.id()).bind(id).fetch_all(&mut *tx).await?;
        let messages = rows
            .into_iter()
            .map(|row| {
                let role: Role =
                    serde_json::from_value(serde_json::Value::String(row.try_get("role")?))
                        .map_err(|_| DbError::Conflict)?;
                Ok(Message {
                    id: row.try_get("id")?,
                    owner_id: owner.id(),
                    conversation_id: id,
                    parent_message_id: row.try_get("parent_message_id")?,
                    role,
                    content: row.try_get("content")?,
                    created_order: row.try_get("created_order")?,
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        let rows = sqlx::query("SELECT id,session_id,head_message_id,message_count,(extract(epoch FROM occurred_at)*1000)::bigint AS occurred_at,(extract(epoch FROM created_at)*1000)::bigint AS created_at,(extract(epoch FROM updated_at)*1000)::bigint AS updated_at FROM conversation_path WHERE owner_id=$1 AND conversation_id=$2 ORDER BY updated_at DESC,id DESC")
            .bind(owner.id()).bind(id).fetch_all(&mut *tx).await?;
        let paths = rows
            .into_iter()
            .map(|row| {
                Ok(ConversationPath {
                    id: row.try_get("id")?,
                    session_id: SessionId::try_from(row.try_get::<String, _>("session_id")?)?,
                    head_message_id: row.try_get("head_message_id")?,
                    message_count: row.try_get("message_count")?,
                    occurred_at: row.try_get("occurred_at")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        tx.commit().await?;
        Ok(ConversationDetail {
            conversation,
            messages,
            paths,
        })
    }
    /// Exposes tree contents independently from external session metadata.
    pub async fn conversation(
        &self,
        owner: OwnerScope,
        id: Uuid,
    ) -> Result<(Conversation, Vec<Message>), DbError> {
        let detail = self.conversation_detail(owner, id).await?;
        Ok((detail.conversation, detail.messages))
    }
    /// Validates ancestors before exposing a path, even if administrative writes corrupted storage.
    pub async fn path(
        &self,
        owner: OwnerScope,
        conversation_id: Uuid,
        head: Uuid,
    ) -> Result<Vec<Message>, DbError> {
        let (conversation, messages) = self.conversation(owner, conversation_id).await?;
        Ok(read_path(&conversation, &messages, head)?)
    }
}
