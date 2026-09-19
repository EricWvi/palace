use crate::{Database, DbError, OwnerScope};
use palace_domain::{Conversation, ImportRequest, Message, Role, SessionId, Source, read_path};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ImportResult {
    pub import_id: Uuid,
    pub conversation_id: Uuid,
    pub head_message_id: Uuid,
    pub created: usize,
    pub reused: usize,
}
impl Database {
    /// Serializes source matching and commits the complete import or none of its business state.
    pub async fn import_path(
        &self,
        owner: OwnerScope,
        request: &ImportRequest,
    ) -> Result<ImportResult, DbError> {
        let mut tx = self.begin_write().await?;
        let previous = sqlx::query("SELECT input_digest,result FROM conversation_import WHERE owner_id=$1 AND idempotency_key=$2").bind(owner.id()).bind(request.idempotency_key()).fetch_optional(&mut *tx).await?;
        if let Some(row) = previous {
            if row.try_get::<Vec<u8>, _>("input_digest")? != request.digest() {
                return Err(DbError::Conflict);
            }
            return Ok(row
                .try_get::<sqlx::types::Json<ImportResult>, _>("result")?
                .0);
        }
        let id: Uuid = sqlx::query_scalar("INSERT INTO conversation(id,owner_id,title,source,session_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT(owner_id,source,session_id) DO UPDATE SET session_id=EXCLUDED.session_id RETURNING id")
            .bind(Uuid::new_v4()).bind(owner.id()).bind(request.title()).bind(request.source().as_str()).bind(request.session_id().as_str()).fetch_one(&mut *tx).await?;
        let mut parent: Option<Uuid> = None;
        let mut created = 0;
        for message in request.messages() {
            let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM message WHERE owner_id=$1 AND conversation_id=$2 AND parent_message_id IS NOT DISTINCT FROM $3 AND role=$4 AND content=$5 ORDER BY created_order,id LIMIT 1")
                .bind(owner.id()).bind(id).bind(parent).bind(message.role.as_str()).bind(&message.content).fetch_optional(&mut *tx).await?;
            parent = Some(if let Some(existing) = existing {
                existing
            } else {
                let message_id = Uuid::new_v4();
                sqlx::query("INSERT INTO message(id,owner_id,conversation_id,parent_message_id,role,content) VALUES($1,$2,$3,$4,$5,$6)")
                    .bind(message_id).bind(owner.id()).bind(id).bind(parent).bind(message.role.as_str()).bind(&message.content).execute(&mut *tx).await?;
                created += 1;
                message_id
            });
        }
        let result = ImportResult {
            import_id: Uuid::new_v4(),
            conversation_id: id,
            head_message_id: parent.ok_or(DbError::Conflict)?,
            created,
            reused: request.messages().len() - created,
        };
        sqlx::query("INSERT INTO conversation_import(id,owner_id,conversation_id,head_message_id,input_digest,message_count,idempotency_key,result,occurred_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,to_timestamp($9::double precision / 1000.0))")
            .bind(result.import_id).bind(owner.id()).bind(id).bind(result.head_message_id).bind(request.digest()).bind(request.messages().len() as i64).bind(request.idempotency_key()).bind(sqlx::types::Json(&result)).bind(request.occurred_at()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result)
    }
    /// Changes display metadata in place; source identity, message tree and import heads remain stable.
    pub async fn rename_conversation(
        &self,
        owner: OwnerScope,
        id: Uuid,
        title: &str,
    ) -> Result<(), DbError> {
        palace_domain::validate_title(title)?;
        let changed = sqlx::query("UPDATE conversation SET title=$3 WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .bind(title)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() != 1 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }
    /// Reads metadata and a deterministic tree under one owner scope.
    pub async fn conversation(
        &self,
        owner: OwnerScope,
        id: Uuid,
    ) -> Result<(Conversation, Vec<Message>), DbError> {
        let row = sqlx::query(
            "SELECT title,source,session_id FROM conversation WHERE owner_id=$1 AND id=$2",
        )
        .bind(owner.id())
        .bind(id)
        .fetch_optional(&self.pool)
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
            session_id: SessionId::try_from(row.try_get::<String, _>("session_id")?)?,
        };
        let rows = sqlx::query("SELECT id,parent_message_id,role,content,created_order FROM message WHERE owner_id=$1 AND conversation_id=$2 ORDER BY created_order,id").bind(owner.id()).bind(id).fetch_all(&self.pool).await?;
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
        Ok((conversation, messages))
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
