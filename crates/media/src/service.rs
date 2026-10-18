use crate::{MediaStore, StoreError, object_key};
use bytes::Bytes;
use palace_db::{Database, DbError, Media, OwnerScope};
use palace_domain::MediaUpload;
use palace_logging::palace_warn;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Stores the object first and registers it second, so every media row has its object. A row
/// that cannot be written leaves an object nobody references; it is removed on a best-effort
/// basis and otherwise left for a later reconciliation by owner prefix.
pub async fn upload<S: MediaStore>(
    database: &Database,
    store: &S,
    owner: OwnerScope,
    upload: MediaUpload,
) -> Result<Media, MediaError> {
    let id = Uuid::now_v7();
    let key = object_key(owner.id(), id);
    let (content, metadata) = upload.into_parts();
    if let Err(error) = store
        .put(&key, Bytes::from(content), metadata.content_type())
        .await
    {
        palace_warn!(message = "Media object write failed", key = %key, error = %error);
        return Err(error.into());
    }
    match database.insert_media(owner, id, &key, &metadata).await {
        Ok(media) => Ok(media),
        Err(error) => {
            if let Err(cleanup) = store.delete(&key).await {
                palace_warn!(message = "Orphaned media object after a failed insert", key = %key, error = %cleanup);
            }
            Err(error.into())
        }
    }
}

/// Removes the object before the row. If either step fails the row survives, so a retry runs
/// both steps again and success is only reported once the object is really gone.
pub async fn delete<S: MediaStore>(
    database: &Database,
    store: &S,
    owner: OwnerScope,
    id: Uuid,
) -> Result<(), MediaError> {
    let key = database.media_object_key(owner, id).await?;
    if let Err(error) = store.delete(&key).await {
        palace_warn!(message = "Media object delete failed; the row is kept for a retry", key = %key, error = %error);
        return Err(error.into());
    }
    database.delete_media(owner, id).await?;
    Ok(())
}
