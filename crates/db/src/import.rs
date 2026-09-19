use crate::{Database, DbError, OwnerScope};
use palace_domain::{ImportRequest, ImportTarget, InputError, InputErrorKind, Role};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ImportResult {
    pub import_id: Uuid,
    pub conversation_id: Uuid,
    pub path_id: Uuid,
    pub head_message_id: Uuid,
    pub created: usize,
    pub reused: usize,
}
impl Database {
    /// Serializes tree mutations so prefix checks, session uniqueness and receipts commit together.
    pub async fn import_path(
        &self,
        owner: OwnerScope,
        request: &ImportRequest,
    ) -> Result<ImportResult, DbError> {
        let mut tx = self.begin_write().await?;
        let previous = sqlx::query("SELECT input_digest,result FROM conversation_import WHERE owner_id=$1 AND idempotency_key=$2")
            .bind(owner.id()).bind(request.idempotency_key()).fetch_optional(&mut *tx).await?;
        if let Some(row) = previous {
            if row.try_get::<Vec<u8>, _>("input_digest")? != request.digest() {
                return Err(DbError::Conflict);
            }
            return Ok(row
                .try_get::<sqlx::types::Json<ImportResult>, _>("result")?
                .0);
        }
        let (conversation_id, path_id, source, session_id, old_head) = match request.target() {
            ImportTarget::Conversation {
                title,
                source,
                session_id,
            } => {
                let id = Uuid::now_v7();
                sqlx::query(
                    "INSERT INTO conversation(id,owner_id,title,source) VALUES($1,$2,$3,$4)",
                )
                .bind(id)
                .bind(owner.id())
                .bind(title)
                .bind(source.as_str())
                .execute(&mut *tx)
                .await?;
                (
                    id,
                    Uuid::now_v7(),
                    source.as_str().to_owned(),
                    session_id.as_str().to_owned(),
                    None,
                )
            }
            ImportTarget::Branch {
                conversation_id,
                session_id,
            } => {
                let source: String = sqlx::query_scalar(
                    "SELECT source FROM conversation WHERE owner_id=$1 AND id=$2",
                )
                .bind(owner.id())
                .bind(conversation_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(DbError::NotFound)?;
                (
                    *conversation_id,
                    Uuid::now_v7(),
                    source,
                    session_id.as_str().to_owned(),
                    None,
                )
            }
            ImportTarget::Update {
                conversation_id,
                path_id,
            } => {
                let row = sqlx::query("SELECT source,session_id,head_message_id FROM conversation_path WHERE owner_id=$1 AND conversation_id=$2 AND id=$3")
                    .bind(owner.id()).bind(conversation_id).bind(path_id).fetch_optional(&mut *tx).await?.ok_or(DbError::NotFound)?;
                (
                    *conversation_id,
                    *path_id,
                    row.try_get("source")?,
                    row.try_get("session_id")?,
                    Some(row.try_get::<Uuid, _>("head_message_id")?),
                )
            }
        };
        if old_head.is_none() {
            let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversation_path WHERE owner_id=$1 AND source=$2 AND session_id=$3)")
                .bind(owner.id()).bind(&source).bind(&session_id).fetch_one(&mut *tx).await?;
            if taken {
                return Err(DbError::DuplicateSession);
            }
        }
        let mut parent = None;
        let mut created = 0;
        let mut shared_assistant = false;
        let mut reached_old_head = old_head.is_none();
        for message in request.messages() {
            let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM message WHERE owner_id=$1 AND conversation_id=$2 AND parent_message_id IS NOT DISTINCT FROM $3 AND role=$4 AND content=$5 ORDER BY created_order,id LIMIT 1")
                .bind(owner.id()).bind(conversation_id).bind(parent).bind(message.role.as_str()).bind(&message.content).fetch_optional(&mut *tx).await?;
            parent = Some(if let Some(id) = existing {
                shared_assistant |= message.role == Role::Assistant;
                if Some(id) == old_head {
                    reached_old_head = true;
                }
                id
            } else {
                if !reached_old_head {
                    return Err(InputError::new(
                        InputErrorKind::Field,
                        "history",
                        "只能追加消息，不能修改已有分支的历史节点",
                    )
                    .into());
                }
                let id = Uuid::now_v7();
                sqlx::query("INSERT INTO message(id,owner_id,conversation_id,parent_message_id,role,content) VALUES($1,$2,$3,$4,$5,$6)")
                    .bind(id).bind(owner.id()).bind(conversation_id).bind(parent).bind(message.role.as_str()).bind(&message.content).execute(&mut *tx).await?;
                created += 1;
                id
            });
        }
        if !reached_old_head {
            return Err(InputError::new(
                InputErrorKind::Field,
                "history",
                "不能截短已有分支，请上传包含全部历史节点的 JSON",
            )
            .into());
        }
        if matches!(request.target(), ImportTarget::Branch { .. }) && !shared_assistant {
            return Err(InputError::new(
                InputErrorKind::Field,
                "history",
                "新分支必须共享从根到 assistant 消息的完整前缀；无共同前缀请导入新会话",
            )
            .into());
        }
        let result = ImportResult {
            import_id: Uuid::now_v7(),
            conversation_id,
            path_id,
            head_message_id: parent.ok_or(DbError::Conflict)?,
            created,
            reused: request.messages().len() - created,
        };
        if old_head.is_some() {
            sqlx::query("UPDATE conversation_path SET head_message_id=$4,message_count=$5,occurred_at=to_timestamp($6::double precision/1000.0),updated_at=GREATEST(date_trunc('milliseconds',clock_timestamp()),updated_at+interval '1 millisecond') WHERE owner_id=$1 AND conversation_id=$2 AND id=$3")
                .bind(owner.id()).bind(conversation_id).bind(path_id).bind(result.head_message_id).bind(request.messages().len() as i64).bind(request.occurred_at()).execute(&mut *tx).await?;
        } else {
            sqlx::query("INSERT INTO conversation_path(id,owner_id,conversation_id,source,session_id,head_message_id,message_count,occurred_at) VALUES($1,$2,$3,$4,$5,$6,$7,to_timestamp($8::double precision/1000.0))")
                .bind(path_id).bind(owner.id()).bind(conversation_id).bind(source).bind(session_id).bind(result.head_message_id).bind(request.messages().len() as i64).bind(request.occurred_at()).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO conversation_import(id,owner_id,conversation_id,path_id,head_message_id,input_digest,message_count,idempotency_key,result,occurred_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,to_timestamp($10::double precision/1000.0))")
            .bind(result.import_id).bind(owner.id()).bind(conversation_id).bind(path_id).bind(result.head_message_id).bind(request.digest()).bind(request.messages().len() as i64).bind(request.idempotency_key()).bind(sqlx::types::Json(&result)).bind(request.occurred_at()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result)
    }
}
