use crate::{Database, DbError, OwnerScope};
use palace_domain::{InputError, InputErrorKind};
use uuid::Uuid;

impl Database {
    /// Retains shared ancestors; the final path must be removed through conversation deletion.
    pub async fn delete_path(
        &self,
        owner: OwnerScope,
        conversation_id: Uuid,
        path_id: Uuid,
    ) -> Result<(), DbError> {
        let mut tx = self.begin_write().await?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversation_path WHERE owner_id=$1 AND conversation_id=$2 AND id=$3)")
            .bind(owner.id()).bind(conversation_id).bind(path_id).fetch_one(&mut *tx).await?;
        if !exists {
            return Err(DbError::NotFound);
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM conversation_path WHERE owner_id=$1 AND conversation_id=$2",
        )
        .bind(owner.id())
        .bind(conversation_id)
        .fetch_one(&mut *tx)
        .await?;
        if count == 1 {
            return Err(InputError::new(
                InputErrorKind::Field,
                "path",
                "最后一个分支请通过删除对话移除",
            )
            .into());
        }
        sqlx::query("DELETE FROM conversation_import WHERE owner_id=$1 AND conversation_id=$2 AND path_id=$3")
            .bind(owner.id()).bind(conversation_id).bind(path_id).execute(&mut *tx).await?;
        sqlx::query(
            "DELETE FROM conversation_path WHERE owner_id=$1 AND conversation_id=$2 AND id=$3",
        )
        .bind(owner.id())
        .bind(conversation_id)
        .bind(path_id)
        .execute(&mut *tx)
        .await?;
        // Delete complete unused suffixes together so no retained parent reference is broken.
        sqlx::query("WITH RECURSIVE retained(id,parent_message_id) AS (SELECT m.id,m.parent_message_id FROM message m JOIN conversation_path p ON p.head_message_id=m.id WHERE p.owner_id=$1 AND p.conversation_id=$2 UNION SELECT m.id,m.parent_message_id FROM message m JOIN retained r ON m.id=r.parent_message_id) DELETE FROM message WHERE owner_id=$1 AND conversation_id=$2 AND id NOT IN (SELECT id FROM retained)")
            .bind(owner.id()).bind(conversation_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Removes a whole card, its paths, receipts and messages in one owner-scoped transaction.
    pub async fn delete_conversation(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let mut tx = self.begin_write().await?;
        sqlx::query("DELETE FROM conversation_import WHERE owner_id=$1 AND conversation_id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM conversation_path WHERE owner_id=$1 AND conversation_id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM message WHERE owner_id=$1 AND conversation_id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let removed = sqlx::query("DELETE FROM conversation WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if removed.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        tx.commit().await?;
        Ok(())
    }
}
