use super::{Provider, contract, now};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use object_store::{ObjectStore, aws::AmazonS3Builder};
use palace_backend::{Server, router};
use palace_db::{CredentialKey, Database, IdentityTokens};
use palace_domain::{ImportLimits, MediaLimits, SourceLinks};
use palace_media::{Addressing, MediaStorage, S3Config};
use palace_testkit::{MINIO_ACCESS_KEY, MINIO_BUCKET, MINIO_REGION, MINIO_SECRET_KEY};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use testcontainers::{
    GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tower::ServiceExt;

const ORIGIN: &str = "https://palace.test";

/// One multipart part: field name, optional filename, optional Content-Type and content.
type Part<'a> = (&'a str, Option<&'a str>, Option<&'a str>, &'a [u8]);

/// Encodes parts exactly as a browser form would, so the real extractor parses them.
fn form(parts: &[Part]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, file_name, content_type, content) in parts {
        body.extend_from_slice(b"--boundary\r\n");
        let disposition = match file_name {
            Some(file_name) => {
                format!(
                    "Content-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\n"
                )
            }
            None => format!("Content-Disposition: form-data; name=\"{name}\"\r\n"),
        };
        body.extend_from_slice(disposition.as_bytes());
        if let Some(content_type) = content_type {
            body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"--boundary--\r\n");
    body
}

/// Sends a request through the whole router and returns status and body text.
async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: &str,
    origin: &str,
    body: Vec<u8>,
) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("cookie", cookie)
        .header("origin", origin)
        .header("content-type", "multipart/form-data; boundary=boundary")
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// Without configured storage both operations answer 503 and nothing else changes.
/// Core test case:
/// - `specs/test-cases/server/media/media-storage.md#object-storage-configuration-must-be-all-or-nothing`
pub(super) async fn exercise_unconfigured(app: &Router, cookie: &str) {
    let unavailable = json!({"error":"media_storage_unavailable","login":""});
    let (status, body) = send(
        app,
        "POST",
        "/api/media",
        cookie,
        ORIGIN,
        form(&[("file", Some("a.png"), Some("image/png"), b"abc")]),
    )
    .await;
    assert_eq!(
        (status, serde_json::from_str::<Value>(&body).unwrap()),
        (StatusCode::SERVICE_UNAVAILABLE, unavailable.clone())
    );
    let (status, body) = send(
        app,
        "DELETE",
        &format!("/api/media/{}", uuid::Uuid::now_v7()),
        cookie,
        ORIGIN,
        Vec::new(),
    )
    .await;
    assert_eq!(
        (status, serde_json::from_str::<Value>(&body).unwrap()),
        (StatusCode::SERVICE_UNAVAILABLE, unavailable)
    );
}

/// Uploads and deletes through real HTTP, PostgreSQL and MinIO: the response carries only the
/// media identity, limits are enforced by the media route, and deletes stay owner-scoped.
/// Core test cases:
/// - `specs/test-cases/server/media/media-storage.md#media-identity-must-be-a-server-generated-stable-uuid-v7`
/// - `specs/test-cases/server/media/media-storage.md#deleting-media-must-remove-the-object-before-reporting-success`
/// - `specs/test-cases/server/media/media-storage.md#media-access-must-stay-within-the-authenticated-owner-scope`
/// - `specs/test-cases/server/media/media-storage.md#upload-limits-must-be-enforced-by-the-media-route-itself`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn media_http_uploads_and_deletes_within_the_owner() {
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
    let config = S3Config {
        endpoint: url::Url::parse(&minio.endpoint).unwrap(),
        region: MINIO_REGION.into(),
        bucket: MINIO_BUCKET.into(),
        access_key_id: MINIO_ACCESS_KEY.into(),
        secret_access_key: MINIO_SECRET_KEY.into(),
        addressing: Addressing::Path,
        limits: MediaLimits { max_bytes: 8 },
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
    let key = CredentialKey::new([4; 32]);
    let mut cookies = Vec::new();
    for subject in ["a", "b"] {
        let session = db
            .create_session(
                IdentityTokens {
                    issuer: "https://idp.test".into(),
                    subject: subject.into(),
                    email: format!("{subject}@example.com"),
                    refresh: "refresh".into(),
                },
                &key,
                now(),
                /*previous*/ None,
            )
            .await
            .unwrap();
        cookies.push(format!("__Host-palace-session={}", session.secret));
    }
    let (cookie, foreign) = (&cookies[0], &cookies[1]);
    let capture = contract::Capture::default();
    let app = capture.attach(router(Server {
        database: db,
        provider: Provider,
        credential_key: key,
        origin: ORIGIN.into(),
        links: SourceLinks::standard().unwrap(),
        limits: ImportLimits::default(),
        notes: std::sync::Arc::new(palace_notes::NoteLibrary::absent()),
        media: MediaStorage::connect(&palace_media::MediaConfig::S3(config)).unwrap(),
        jottings: palace_jottings::JottingService::Unconfigured,
        now,
    }));
    let objects = || async {
        let mut keys: Vec<String> = Vec::new();
        for prefix in raw.list_with_delimiter(None).await.unwrap().common_prefixes {
            for object in raw
                .list_with_delimiter(Some(&prefix))
                .await
                .unwrap()
                .objects
            {
                keys.push(object.location.to_string());
            }
        }
        keys.len()
    };

    // The response is exactly the identity and metadata; owner fields in the form are refused.
    let (status, body) = send(
        &app,
        "POST",
        "/api/media",
        cookie,
        ORIGIN,
        form(&[("file", Some("截图.png"), Some("Image/PNG"), b"abc")]),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let body: Value = serde_json::from_str(&body).unwrap();
    let id: uuid::Uuid = serde_json::from_value(body["id"].clone()).unwrap();
    assert_eq!(id.get_version_num(), 7);
    assert_eq!(
        body,
        json!({"id":id,"content_type":"image/png","byte_size":3,"file_name":"截图.png","created_at":body["created_at"]})
    );
    assert!(body["created_at"].is_i64());
    let (status, body) = send(
        &app,
        "POST",
        "/api/media",
        cookie,
        ORIGIN,
        form(&[
            ("owner_id", None, None, b"other"),
            ("file", Some("a.png"), None, b"abc"),
        ]),
    )
    .await;
    assert_eq!(
        (status, serde_json::from_str::<Value>(&body).unwrap()),
        (
            StatusCode::BAD_REQUEST,
            json!({"kind":"field","path":"owner_id","message":"unknown or repeated field"})
        )
    );

    // The limit is inclusive and belongs to the media route; rejected bodies store nothing.
    let (status, _) = send(
        &app,
        "POST",
        "/api/media",
        cookie,
        ORIGIN,
        form(&[("file", None, None, b"12345678")]),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    for (parts, status, expected) in [
        (
            form(&[("file", None, None, b"123456789")]),
            StatusCode::PAYLOAD_TOO_LARGE,
            json!({"kind":"limit","path":"file","message":"file too large"}),
        ),
        (
            form(&[("file", None, None, b"")]),
            StatusCode::BAD_REQUEST,
            json!({"kind":"field","path":"file","message":"empty file"}),
        ),
        (
            form(&[]),
            StatusCode::BAD_REQUEST,
            json!({"kind":"field","path":"file","message":"missing field"}),
        ),
    ] {
        let (actual, body) = send(&app, "POST", "/api/media", cookie, ORIGIN, parts).await;
        assert_eq!(
            (actual, serde_json::from_str::<Value>(&body).unwrap()),
            (status, expected)
        );
    }
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM media")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((rows, objects().await), (2, 2));
    let (status, _) = send(
        &app,
        "POST",
        "/api/media",
        cookie,
        "https://evil.test",
        form(&[("file", None, None, b"abc")]),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Another owner sees the id as absent; the owner's delete removes the object and the row.
    let item = format!("/api/media/{id}");
    let not_found = json!({"error":"not_found","login":""});
    let (status, body) = send(&app, "DELETE", &item, foreign, ORIGIN, Vec::new()).await;
    assert_eq!(
        (status, serde_json::from_str::<Value>(&body).unwrap()),
        (StatusCode::NOT_FOUND, not_found.clone())
    );
    assert_eq!(objects().await, 2);
    assert_eq!(
        send(&app, "DELETE", &item, cookie, ORIGIN, Vec::new()).await,
        (StatusCode::NO_CONTENT, String::new())
    );
    assert_eq!(objects().await, 1);
    let (status, body) = send(&app, "DELETE", &item, cookie, ORIGIN, Vec::new()).await;
    assert_eq!(
        (status, serde_json::from_str::<Value>(&body).unwrap()),
        (StatusCode::NOT_FOUND, not_found)
    );
    let (status, _) = send(
        &app,
        "DELETE",
        "/api/media/not-a-uuid",
        cookie,
        ORIGIN,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    capture.verify(contract::Coverage::ObservedOperations);
}
