// Integration-test helpers should fail immediately when setup fails.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::Bytes;
use object_store::{
    Attribute, ObjectStore,
    aws::{AmazonS3, AmazonS3Builder},
    path::Path,
};
use palace_db::{Database, DbError, Media, OwnerScope};
use palace_domain::{MediaLimits, MediaUpload};
use palace_media::{
    Addressing, MediaError, MediaStore, S3Bucket, S3Config, StoreError, object_key,
};
use palace_testkit::{MINIO_ACCESS_KEY, MINIO_BUCKET, MINIO_REGION, MINIO_SECRET_KEY, Minio};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use uuid::Uuid;

/// Real PostgreSQL and MinIO plus two owners; the containers live as long as this value.
struct Services {
    _postgres: ContainerAsync<GenericImage>,
    _minio: Minio,
    db: Database,
    pool: PgPool,
    /// The production store under test.
    bucket: S3Bucket,
    /// An independent client that inspects what actually reached the bucket.
    raw: AmazonS3,
    a: OwnerScope,
    b: OwnerScope,
}

async fn services() -> Services {
    let postgres = palace_testkit::reaped(
        GenericImage::new("postgres", "17-alpine")
            .with_exposed_port(5432.tcp())
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            )),
    )
    .unwrap()
    .with_env_var("POSTGRES_PASSWORD", "palace-test")
    .start()
    .await
    .unwrap();
    let host = postgres.get_host().await.unwrap();
    let port = postgres
        .get_host_port_ipv4(/*internal_port*/ 5432)
        .await
        .unwrap();
    let url = format!("postgres://postgres:palace-test@{host}:{port}/postgres");
    let db = Database::connect(&url).await.unwrap();
    let pool = PgPool::connect(&url).await.unwrap();
    let minio = palace_testkit::minio().await.unwrap();
    let config = S3Config {
        endpoint: url::Url::parse(&minio.endpoint).unwrap(),
        region: MINIO_REGION.into(),
        bucket: MINIO_BUCKET.into(),
        access_key_id: MINIO_ACCESS_KEY.into(),
        secret_access_key: MINIO_SECRET_KEY.into(),
        addressing: Addressing::Path,
        limits: MediaLimits::default(),
    };
    let raw = AmazonS3Builder::new()
        .with_endpoint(&minio.endpoint)
        .with_allow_http(true)
        .with_region(MINIO_REGION)
        .with_bucket_name(MINIO_BUCKET)
        .with_access_key_id(MINIO_ACCESS_KEY)
        .with_secret_access_key(MINIO_SECRET_KEY)
        .build()
        .unwrap();
    let mut owners = Vec::new();
    for subject in ["a", "b"] {
        let owner = db
            .resolve_identity(
                "https://idp.test",
                subject,
                &format!("{subject}@example.com"),
            )
            .await
            .unwrap();
        owners.push(owner.scope());
    }
    Services {
        _postgres: postgres,
        _minio: minio,
        db,
        pool,
        bucket: config.connect().unwrap(),
        raw,
        a: owners[0],
        b: owners[1],
    }
}

/// Wraps the real bucket and fails one operation on demand, at the exact step under test.
struct Faulty<'a> {
    inner: &'a S3Bucket,
    fail_put: AtomicBool,
    fail_delete: AtomicBool,
}
impl<'a> Faulty<'a> {
    fn new(inner: &'a S3Bucket) -> Self {
        Self {
            inner,
            fail_put: AtomicBool::new(false),
            fail_delete: AtomicBool::new(false),
        }
    }
}
impl MediaStore for Faulty<'_> {
    async fn put(&self, key: &str, content: Bytes, content_type: &str) -> Result<(), StoreError> {
        if self.fail_put.load(Ordering::SeqCst) {
            return Err(StoreError::new("injected put failure"));
        }
        self.inner.put(key, content, content_type).await
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(StoreError::new("injected delete failure"));
        }
        self.inner.delete(key).await
    }
}

/// Makes the next writes of one kind to `media` fail inside real PostgreSQL.
async fn fail_media(pool: &PgPool, operation: &str) {
    sqlx::query("CREATE OR REPLACE FUNCTION fail_media() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected media failure'; END $$")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(&format!(
        "CREATE TRIGGER fail_media_{operation} BEFORE {operation} ON media FOR EACH ROW EXECUTE FUNCTION fail_media()"
    ))
    .execute(pool)
    .await
    .unwrap();
}
async fn heal_media(pool: &PgPool, operation: &str) {
    sqlx::query(&format!("DROP TRIGGER fail_media_{operation} ON media"))
        .execute(pool)
        .await
        .unwrap();
}

/// The persisted row in full, including the columns no API returns.
#[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
struct Row {
    id: Uuid,
    owner_id: Uuid,
    object_key: String,
    content_type: String,
    byte_size: i64,
    sha256: Vec<u8>,
    file_name: Option<String>,
}
async fn rows(pool: &PgPool) -> Vec<Row> {
    sqlx::query_as("SELECT id,owner_id,object_key,content_type,byte_size,sha256,file_name FROM media ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}
/// Every key in the bucket, sorted.
async fn keys(raw: &AmazonS3) -> Vec<String> {
    let mut keys: Vec<String> = raw
        .list_with_delimiter(None)
        .await
        .unwrap()
        .common_prefixes
        .into_iter()
        .map(|prefix| prefix.to_string())
        .collect();
    let mut objects = Vec::new();
    for prefix in keys.drain(..) {
        for object in raw
            .list_with_delimiter(Some(&Path::from(prefix)))
            .await
            .unwrap()
            .objects
        {
            objects.push(object.location.to_string());
        }
    }
    objects.sort();
    objects
}
fn png(bytes: &[u8]) -> MediaUpload {
    MediaUpload::new(
        bytes.to_vec(),
        Some("image/png"),
        Some("截图.png"),
        MediaLimits::default(),
    )
    .unwrap()
}

/// A row is written only after its object, with metadata that matches the stored bytes; failed
/// writes on either side leave no row and no object.
/// Core test cases:
/// - `specs/test-cases/server/media/media-storage.md#a-media-row-must-exist-only-after-its-object-is-stored`
/// - `specs/test-cases/server/media/media-storage.md#media-identity-must-be-a-server-generated-stable-uuid-v7`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn upload_registers_rows_only_for_stored_objects() {
    let s = services().await;
    let upload = png(b"abc");
    let (_, metadata) = upload.clone().into_parts();
    let media = palace_media::upload(&s.db, &s.bucket, s.a, upload)
        .await
        .unwrap();
    assert_eq!(media.id.get_version_num(), 7);
    assert_eq!(
        media,
        Media {
            id: media.id,
            content_type: "image/png".into(),
            byte_size: 3,
            file_name: Some("截图.png".into()),
            created_at: media.created_at,
        }
    );
    let key = object_key(s.a.id(), media.id);
    assert_eq!(
        rows(&s.pool).await,
        vec![Row {
            id: media.id,
            owner_id: s.a.id(),
            object_key: key.clone(),
            content_type: "image/png".into(),
            byte_size: 3,
            sha256: metadata.sha256().to_vec(),
            file_name: Some("截图.png".into()),
        }]
    );
    let stored = s.raw.get(&Path::from(key.as_str())).await.unwrap();
    assert_eq!(
        stored.attributes.get(&Attribute::ContentType),
        Some(&"image/png".into())
    );
    assert_eq!(stored.bytes().await.unwrap(), Bytes::from_static(b"abc"));

    let faulty = Faulty::new(&s.bucket);
    faulty.fail_put.store(true, Ordering::SeqCst);
    assert!(matches!(
        palace_media::upload(&s.db, &faulty, s.a, png(b"put fails")).await,
        Err(MediaError::Store(_))
    ));
    fail_media(&s.pool, "insert").await;
    assert!(matches!(
        palace_media::upload(&s.db, &s.bucket, s.a, png(b"insert fails")).await,
        Err(MediaError::Database(DbError::Storage(_)))
    ));
    heal_media(&s.pool, "insert").await;
    assert_eq!(rows(&s.pool).await.len(), 1);
    assert_eq!(keys(&s.raw).await, vec![key]);
}

/// Success means the object is gone; a failure at either step keeps the row so a retry converges.
/// Core test case:
/// - `specs/test-cases/server/media/media-storage.md#deleting-media-must-remove-the-object-before-reporting-success`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn delete_removes_the_object_first_and_retries_converge() {
    let s = services().await;
    let media = palace_media::upload(&s.db, &s.bucket, s.a, png(b"abc"))
        .await
        .unwrap();
    let key = object_key(s.a.id(), media.id);

    let faulty = Faulty::new(&s.bucket);
    faulty.fail_delete.store(true, Ordering::SeqCst);
    assert!(matches!(
        palace_media::delete(&s.db, &faulty, s.a, media.id).await,
        Err(MediaError::Store(_))
    ));
    assert_eq!(rows(&s.pool).await.len(), 1);
    assert_eq!(keys(&s.raw).await, vec![key.clone()]);

    fail_media(&s.pool, "delete").await;
    assert!(matches!(
        palace_media::delete(&s.db, &s.bucket, s.a, media.id).await,
        Err(MediaError::Database(DbError::Storage(_)))
    ));
    // The object is already gone while the row survives, so the retry deletes an absent key.
    assert_eq!(rows(&s.pool).await.len(), 1);
    assert_eq!(keys(&s.raw).await, Vec::<String>::new());
    heal_media(&s.pool, "delete").await;
    palace_media::delete(&s.db, &s.bucket, s.a, media.id)
        .await
        .unwrap();
    assert_eq!(rows(&s.pool).await, Vec::new());
    assert!(matches!(
        palace_media::delete(&s.db, &s.bucket, s.a, media.id).await,
        Err(MediaError::Database(DbError::NotFound))
    ));
}

/// Another owner's id behaves like an absent one, and ownership cannot be reassigned.
/// Core test case:
/// - `specs/test-cases/server/media/media-storage.md#media-access-must-stay-within-the-authenticated-owner-scope`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn media_stays_within_its_owner() {
    let s = services().await;
    let media = palace_media::upload(&s.db, &s.bucket, s.a, png(b"abc"))
        .await
        .unwrap();
    assert!(matches!(
        palace_media::delete(&s.db, &s.bucket, s.b, media.id).await,
        Err(MediaError::Database(DbError::NotFound))
    ));
    assert!(matches!(
        palace_media::delete(&s.db, &s.bucket, s.b, Uuid::now_v7()).await,
        Err(MediaError::Database(DbError::NotFound))
    ));
    assert_eq!(keys(&s.raw).await, vec![object_key(s.a.id(), media.id)]);
    assert!(
        sqlx::query("UPDATE media SET owner_id=$1 WHERE id=$2")
            .bind(s.b.id())
            .bind(media.id)
            .execute(&s.pool)
            .await
            .is_err()
    );
    assert_eq!(rows(&s.pool).await[0].owner_id, s.a.id());
}
