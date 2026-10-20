//! `pal` against a real Palace router backed by real PostgreSQL and MinIO: what it prints must be
//! what Palace stored, and deletes must follow Palace's owner scope.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "support/article.rs"]
mod article;
#[path = "support/article_palace.rs"]
mod article_palace;
#[allow(dead_code)]
#[path = "support/pal.rs"]
mod support;

use palace_backend::{LoginProvider, LoginRedirect, Server, router};
use palace_db::{CredentialKey, Database, IdentityProvider, IdentityTokens, ProviderError};
use palace_domain::{AccessTokenNote, ImportLimits, MediaLimits, SourceLinks};
use palace_media::{Addressing, MediaConfig, MediaStorage, S3Config};
use palace_testkit::{MINIO_ACCESS_KEY, MINIO_BUCKET, MINIO_REGION, MINIO_SECRET_KEY};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Output, TOKEN, pal, serve};
use testcontainers::{
    GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};

/// Bearer requests never reach the identity provider; any call is a test failure surfaced as 5xx.
struct Provider;
impl IdentityProvider for Provider {
    /// Rejects, because fresh sessions never need revalidation in this test.
    async fn refresh(&self, _: &str) -> Result<IdentityTokens, ProviderError> {
        Err(ProviderError::Rejected)
    }
    /// Unused: nobody logs out.
    async fn revoke(&self, _: &str) -> Result<(), ProviderError> {
        Err(ProviderError::Unavailable)
    }
}
impl LoginProvider for Provider {
    /// Unused: nobody logs in.
    fn authorize(&self) -> Result<LoginRedirect, ProviderError> {
        Err(ProviderError::Unavailable)
    }
    /// Unused: nobody logs in.
    async fn callback(&self, _: String, _: &str) -> Result<IdentityTokens, ProviderError> {
        Err(ProviderError::Rejected)
    }
}

/// Keeps session creation and requests on one frozen epoch.
fn now() -> i64 {
    100
}

/// Builds a Palace whose media storage is `media`, as production would.
fn palace(database: Database, media: MediaStorage) -> axum::Router {
    router(Server {
        database,
        provider: Provider,
        credential_key: CredentialKey::new([4; 32]),
        origin: "https://palace.test".into(),
        links: SourceLinks::standard().unwrap(),
        limits: ImportLimits::default(),
        notes: std::sync::Arc::new(palace_notes::NoteLibrary::absent()),
        media,
        jottings: palace_jottings::JottingService::Unconfigured,
        timezone: chrono_tz::Asia::Shanghai,
        now,
    })
}

/// Core test cases:
/// - `specs/test-cases/cli/media-commands.md#the-access-token-must-reach-only-the-configured-palace-host`
/// - `specs/test-cases/cli/media-commands.md#upload-must-print-exactly-the-stored-media-and-nothing-on-failure`
/// - `specs/test-cases/cli/media-commands.md#delete-must-succeed-only-when-palace-confirms-removal`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn pal_uploads_and_deletes_media_through_a_real_palace() {
    use tracing::instrument::WithSubscriber;
    palace_logging::with_trace_logging(|| {
        async {
            palace_logging::initialize_test_clock();
            let postgres = palace_testkit::reaped(
                GenericImage::new("postgres", "17-alpine")
                    .with_exposed_port(5432.tcp())
                    .with_wait_for(WaitFor::message_on_stderr(
                        "database system is ready to accept connections",
                    )),
            )
            .unwrap()
            .with_env_var("POSTGRES_PASSWORD", "test")
            .start()
            .await
            .unwrap();
            let host = postgres.get_host().await.unwrap();
            let port = postgres
                .get_host_port_ipv4(/*internal_port*/ 5432)
                .await
                .unwrap();
            let url = format!("postgres://postgres:test@{host}:{port}/postgres");
            let db = Database::connect(&url).await.unwrap();
            let pool = sqlx::PgPool::connect(&url).await.unwrap();
            let minio = palace_testkit::minio().await.unwrap();
            let storage = MediaStorage::connect(&MediaConfig::S3(Box::new(S3Config {
                endpoint: url::Url::parse(&minio.endpoint).unwrap(),
                public_endpoint: None,
                region: MINIO_REGION.into(),
                bucket: MINIO_BUCKET.into(),
                access_key_id: MINIO_ACCESS_KEY.into(),
                secret_access_key: MINIO_SECRET_KEY.into(),
                addressing: Addressing::Path,
                limits: MediaLimits {
                    max_bytes: 8,
                    ..MediaLimits::default()
                },
            })))
            .unwrap();
            let mut tokens = Vec::new();
            for subject in ["a", "b"] {
                let session = db
                    .create_session(
                        IdentityTokens {
                            issuer: "https://idp.test".into(),
                            subject: subject.into(),
                            email: format!("{subject}@example.com"),
                            refresh: "refresh".into(),
                        },
                        &CredentialKey::new([4; 32]),
                        now(),
                        /*previous*/ None,
                    )
                    .await
                    .unwrap();
                let issued = db
                    .create_access_token(&session.owner, &AccessTokenNote::parse("cli").unwrap())
                    .await
                    .unwrap();
                tokens.push(issued.secret.expose().to_owned());
            }
            let configured = serve(palace(db.clone(), storage)).await;
            let unconfigured = serve(palace(db, MediaStorage::Unconfigured)).await;
            let owner = [
                ("PAL_URL", configured.as_str()),
                ("PAL_TOKEN", tokens[0].as_str()),
            ];
            let foreign = [
                ("PAL_URL", configured.as_str()),
                ("PAL_TOKEN", tokens[1].as_str()),
            ];
            let folder = tempfile::tempdir().unwrap();
            let path = |name: &str, content: &[u8]| {
                let path = folder.path().join(name);
                std::fs::write(&path, content).unwrap();
                path.to_str().unwrap().to_owned()
            };
            let failed = |stderr: &str| Output {
                code: Some(1),
                stdout: String::new(),
                stderr: format!("pal: {stderr}\n"),
            };
            let stored = |id: Value| {
                let pool = pool.clone();
                async move {
                    sqlx::query_as::<_, (String, i64, Option<String>, i64)>(
                        "SELECT content_type,byte_size,file_name,(extract(epoch FROM created_at)*1000)::bigint FROM media WHERE id=$1",
                    )
                    .bind(uuid::Uuid::parse_str(id.as_str().unwrap()).unwrap())
                    .fetch_optional(&pool)
                    .await
                    .unwrap()
                    .map(|(content_type, byte_size, file_name, created_at)| {
                        json!({"id":id,"content_type":content_type,"byte_size":byte_size,"file_name":file_name,"created_at":created_at})
                    })
                }
            };

            // stdout is one line of JSON, equal to the row Palace stored; the type comes from the
            // extension, and an unknown extension is declared as octet-stream.
            let mut ids = Vec::new();
            for (name, content, content_type) in [
                ("截图.png", &b"abc"[..], "image/png"),
                ("notes.zzz", &b"12345678"[..], "application/octet-stream"),
            ] {
                let output = pal(&["s3", "upload", &path(name, content)], &owner).await;
                assert_eq!(
                    (output.code, output.stderr.as_str()),
                    (Some(0), ""),
                    "{name}"
                );
                assert_eq!(output.stdout.lines().count(), 1, "{}", output.stdout);
                let media: Value = serde_json::from_str(&output.stdout).unwrap();
                assert_eq!(Some(media.clone()), stored(media["id"].clone()).await);
                assert_eq!(
                    (&media["file_name"], &media["content_type"]),
                    (&json!(name), &json!(content_type))
                );
                ids.push(media["id"].as_str().unwrap().to_owned());
            }
            assert_eq!(
                pal(&["s3", "upload", &path("big.png", b"123456789")], &owner).await,
                failed("Palace answered 413 Payload Too Large: limit error at file: file too large")
            );

            article_palace::exercise(&pool, &owner, &foreign).await;

            // Palace rejects a well-formed token it never issued, and the token stays out of stderr.
            let unknown = [("PAL_URL", configured.as_str()), ("PAL_TOKEN", TOKEN)];
            assert_eq!(
                pal(&["s3", "delete", &ids[0]], &unknown).await,
                failed(
                    "Palace answered 401 Unauthorized: authentication_required (check that PAL_TOKEN is a token that has not been deleted)"
                )
            );

            // Another owner's token cannot delete; the owner's can, once.
            let not_found =
                failed("Palace answered 404 Not Found: not_found (no such media for this token's owner)");
            assert_eq!(pal(&["s3", "delete", &ids[0]], &foreign).await, not_found);
            assert!(stored(json!(ids[0])).await.is_some());
            let deleted = Output {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            };
            assert_eq!(pal(&["s3", "delete", &ids[0]], &owner).await, deleted);
            assert_eq!(stored(json!(ids[0])).await, None);
            assert_eq!(pal(&["s3", "delete", &ids[0]], &owner).await, not_found);

            let without_storage = [
                ("PAL_URL", unconfigured.as_str()),
                ("PAL_TOKEN", tokens[0].as_str()),
            ];
            assert_eq!(
                pal(&["s3", "delete", &ids[1]], &without_storage).await,
                failed("Palace answered 503 Service Unavailable: media_storage_unavailable")
            );
        }.with_current_subscriber()
    }).await;
}
