//! Owner-scoped media rows. Media is outside the sync domain, so these writes never take the
//! publication lock, and object storage is never called while a transaction is open.
use crate::{Database, DbError, OwnerScope};
use palace_domain::MediaMetadata;
use sqlx::Row;
use uuid::Uuid;

/// A stored media file as it may leave the server: the object key and digest stay internal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Media {
    pub id: Uuid,
    pub content_type: String,
    pub byte_size: i64,
    pub file_name: Option<String>,
    /// Unix epoch milliseconds assigned by the database clock.
    pub created_at: i64,
}

impl Database {
    /// Registers an object that storage has already accepted, so a row always has its object.
    pub async fn insert_media(
        &self,
        owner: OwnerScope,
        id: Uuid,
        object_key: &str,
        metadata: &MediaMetadata,
    ) -> Result<Media, DbError> {
        let row = sqlx::query("INSERT INTO media(id,owner_id,object_key,content_type,byte_size,sha256,file_name) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING (extract(epoch FROM created_at)*1000)::bigint AS created_at")
            .bind(id)
            .bind(owner.id())
            .bind(object_key)
            .bind(metadata.content_type())
            .bind(metadata.byte_size())
            .bind(metadata.sha256().as_slice())
            .bind(metadata.file_name())
            .fetch_one(&self.pool)
            .await?;
        Ok(Media {
            id,
            content_type: metadata.content_type().into(),
            byte_size: metadata.byte_size(),
            file_name: metadata.file_name().map(str::to_owned),
            created_at: row.try_get("created_at")?,
        })
    }

    /// Locates the object behind a media id; ids of other owners are indistinguishable from absent ones.
    pub async fn media_object_key(&self, owner: OwnerScope, id: Uuid) -> Result<String, DbError> {
        sqlx::query_scalar("SELECT object_key FROM media WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DbError::NotFound)
    }

    /// Forgets a media row after its object is gone. A row a concurrent delete already removed
    /// reaches the same end state, so it is not reported as missing.
    pub async fn delete_media(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        sqlx::query("DELETE FROM media WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
