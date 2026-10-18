use bytes::Bytes;
use object_store::{Attribute, Attributes, PutOptions, PutPayload, path::Path};
use std::future::Future;
use uuid::Uuid;

/// Writes and removes media objects.
///
/// The media service depends on this narrow seam rather than on a full object store so tests can
/// wrap a real backend and fail one operation at the exact step under test. Implementations must
/// report deleting an absent key as success: a retried media delete repeats the object removal
/// after a partial failure, and S3 `DeleteObject` already behaves this way.
pub trait MediaStore: Send + Sync {
    /// Stores the complete content under `key`, recording `content_type` on the object.
    fn put(
        &self,
        key: &str,
        content: Bytes,
        content_type: &str,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
    /// Removes the object under `key`; an absent object is not an error.
    fn delete(&self, key: &str) -> impl Future<Output = Result<(), StoreError>> + Send;
}

/// An object storage request that did not complete; callers must not assume either outcome.
#[derive(Debug, thiserror::Error)]
#[error("object storage request failed")]
pub struct StoreError(#[source] Box<dyn std::error::Error + Send + Sync>);
impl StoreError {
    pub fn new(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(source.into())
    }
}

/// One bucket of any `object_store` backend: S3 in production, in memory where no S3 is needed.
pub struct Bucket<T>(T);
impl<T> Bucket<T> {
    pub fn new(store: T) -> Self {
        Self(store)
    }
}
impl<T: object_store::ObjectStore> MediaStore for Bucket<T> {
    /// Writes the object in a single request; media files are small enough not to need multipart.
    async fn put(&self, key: &str, content: Bytes, content_type: &str) -> Result<(), StoreError> {
        let options = PutOptions {
            attributes: Attributes::from_iter([(Attribute::ContentType, content_type.to_owned())]),
            ..Default::default()
        };
        self.0
            .put_opts(
                &Path::parse(key).map_err(StoreError::new)?,
                PutPayload::from(content),
                options,
            )
            .await
            .map(drop)
            .map_err(StoreError::new)
    }
    /// Relies on the backend's idempotent delete, which S3 and the in-memory store both provide.
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.0
            .delete(&Path::parse(key).map_err(StoreError::new)?)
            .await
            .map_err(StoreError::new)
    }
}

/// Places a new object under its owner so one owner's objects can be listed and reconciled by
/// prefix. Neither part carries a file name, so keys need no escaping and never collide.
pub fn object_key(owner_id: Uuid, media_id: Uuid) -> String {
    Path::from_iter([owner_id.to_string(), media_id.to_string()]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::{ObjectStore, memory::InMemory};
    use pretty_assertions::assert_eq;
    use std::sync::Arc;

    /// Keys are the owner and media ids joined as two segments, with no file name or date.
    /// Core test case:
    /// - `specs/test-cases/server/media/media-storage.md#media-identity-must-be-a-server-generated-stable-uuid-v7`
    #[test]
    fn object_keys_are_owner_prefixed_media_ids() {
        let owner = Uuid::from_u128(1);
        let media = Uuid::from_u128(2);
        assert_eq!(
            object_key(owner, media),
            "00000000-0000-0000-0000-000000000001/00000000-0000-0000-0000-000000000002"
        );
    }

    /// Content and type reach the backend, and deleting twice leaves no object and no error.
    #[tokio::test]
    async fn bucket_writes_typed_objects_and_deletes_idempotently() {
        let backend: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let bucket = Bucket::new(backend.clone());
        bucket
            .put("o/m", Bytes::from_static(b"png"), "image/png")
            .await
            .unwrap();
        let stored = backend.get(&Path::from("o/m")).await.unwrap();
        assert_eq!(
            stored.attributes.get(&Attribute::ContentType),
            Some(&"image/png".into())
        );
        assert_eq!(stored.bytes().await.unwrap(), Bytes::from_static(b"png"));
        bucket.delete("o/m").await.unwrap();
        bucket.delete("o/m").await.unwrap();
        assert!(backend.head(&Path::from("o/m")).await.is_err());
    }
}
