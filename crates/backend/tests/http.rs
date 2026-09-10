use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use palace_backend::{LoginProvider, LoginRedirect, Server, router};
use palace_db::{CredentialKey, Database, IdentityProvider, IdentityTokens, ProviderError};
use palace_domain::{ImportLimits, SourceLinks};
use pretty_assertions::assert_eq;
use testcontainers::{
    GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tower::ServiceExt;

struct Provider;
impl IdentityProvider for Provider {
    /// Fails if this test's fresh sessions unexpectedly request remote revalidation.
    async fn refresh(&self, _: &str) -> Result<IdentityTokens, ProviderError> {
        Err(ProviderError::Rejected)
    }
    /// Keeps logout's external failure path observable without live credentials.
    async fn revoke(&self, _: &str) -> Result<(), ProviderError> {
        Err(ProviderError::Unavailable)
    }
}
impl LoginProvider for Provider {
    /// Supplies a local fake authorization target for redirect contract checks.
    fn authorize(&self) -> Result<LoginRedirect, ProviderError> {
        Ok(LoginRedirect {
            url: url::Url::parse("https://idp.test/authorize").unwrap(),
            state: "random-state".into(),
            proof: "proof".into(),
        })
    }
    /// Is not used by these authenticated business request tests.
    async fn callback(&self, _: String, _: &str) -> Result<IdentityTokens, ProviderError> {
        Err(ProviderError::Rejected)
    }
}
/// Uses the same frozen epoch for session creation and every HTTP request.
fn now() -> i64 {
    100
}
/// Builds requests that exercise the actual router and body extractors.
fn request(
    method: &str,
    path: &str,
    cookie: &str,
    origin: &str,
    content_type: &str,
    body: String,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie)
        .header("origin", origin)
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap()
}
/// Checks owner spoofing, JSON/file parity, inert content, rolling cookies and CSRF against real PostgreSQL.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn authenticated_http_imports_preserve_scope_and_file_parity() {
    assert!(
        std::process::Command::new("docker")
            .args(["image", "inspect", "postgres:17-alpine"])
            .output()
            .unwrap()
            .status
            .success(),
        "prepare postgres:17-alpine; downloads are forbidden"
    );
    let container = GenericImage::new("postgres", "17-alpine")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_PASSWORD", "test")
        .start()
        .await
        .unwrap();
    let host = container.get_host().await.unwrap();
    let port = container
        .get_host_port_ipv4(/*internal_port*/ 5432)
        .await
        .unwrap();
    let db = Database::connect(&format!("postgres://postgres:test@{host}:{port}/postgres"))
        .await
        .unwrap();
    let key = CredentialKey::new([4; 32]);
    let mut sessions = Vec::new();
    for subject in ["a", "b"] {
        sessions.push(
            db.create_session(
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
            .unwrap(),
        );
    }
    let cookie = format!("__Host-palace-session={}", sessions[0].secret);
    let foreign = format!("__Host-palace-session={}", sessions[1].secret);
    let app = router(Server {
        database: db,
        provider: Provider,
        credential_key: key,
        origin: "https://palace.test".into(),
        links: SourceLinks::standard().unwrap(),
        limits: ImportLimits::default(),
        now,
    });
    let history = r#"[{"role":"user","content":"<script>alert(1)</script>\r\n","extra":123}]"#;
    let input = serde_json::json!({"title":"t","source":"chatgpt","session_id":"s","history":history,"idempotency_key":"key"});
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/import",
            &cookie,
            "https://palace.test",
            "application/json",
            input.to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Secure; HttpOnly; SameSite=Lax")
    );
    let result: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let fields = [
        ("title", "t"),
        ("source", "chatgpt"),
        ("session_id", "s"),
        ("history", history),
        ("idempotency_key", "key"),
    ];
    let mut body = String::new();
    for (name, value) in fields {
        body.push_str(&format!(
            "--boundary\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    body.push_str("--boundary--\r\n");
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/import/file",
            &cookie,
            "https://palace.test",
            "multipart/form-data; boundary=boundary",
            body,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let file_result: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(file_result, result);
    let path = format!(
        "/api/conversations/{}",
        result["conversation_id"].as_str().unwrap()
    );
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &path,
            &foreign,
            "https://palace.test",
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &path,
            &cookie,
            "https://palace.test",
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    let tree: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        tree["messages"][0]["content"],
        "<script>alert(1)</script>\r\n"
    );
    let mut spoof = input.clone();
    spoof["owner_id"] = serde_json::json!(sessions[1].owner.id);
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/import",
            &cookie,
            "https://palace.test",
            "application/json",
            spoof.to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    for origin in ["https://evil.test", "null", ""] {
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/auth/logout",
                &cookie,
                origin,
                "application/json",
                String::new(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/auth/logout",
            &cookie,
            "https://palace.test",
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .oneshot(request(
            "GET",
            "/api/me",
            &cookie,
            "https://palace.test",
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Runs producer upload and consumer pull through the real HTTP/PG boundary and persists both local replicas.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn http_sync_round_propagates_records_and_tombstones() {
    use palace_domain::Record;
    use palace_sync::{HttpTransport, Replica, SyncClient};
    assert!(
        std::process::Command::new("docker")
            .args(["image", "inspect", "postgres:17-alpine"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let container = GenericImage::new("postgres", "17-alpine")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_PASSWORD", "test")
        .start()
        .await
        .unwrap();
    let host = container.get_host().await.unwrap();
    let port = container
        .get_host_port_ipv4(/*internal_port*/ 5432)
        .await
        .unwrap();
    let db = Database::connect(&format!("postgres://postgres:test@{host}:{port}/postgres"))
        .await
        .unwrap();
    let key = CredentialKey::new([4; 32]);
    let session = db
        .create_session(
            IdentityTokens {
                issuer: "https://idp.test".into(),
                subject: "a".into(),
                email: "a@example.com".into(),
                refresh: "refresh".into(),
            },
            &key,
            now(),
            /*previous*/ None,
        )
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = router(Server {
        database: db,
        provider: Provider,
        credential_key: key,
        origin: origin.clone(),
        links: SourceLinks::standard().unwrap(),
        limits: ImportLimits::default(),
        now,
    });
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "cookie",
        format!("__Host-palace-session={}", session.secret)
            .parse()
            .unwrap(),
    );
    let transport = HttpTransport::new(
        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap(),
        &origin,
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let producer = SyncClient::new(
        Replica::open(&directory.path().join("producer.sqlite"), session.owner.id).unwrap(),
    );
    let consumer = SyncClient::new(
        Replica::open(&directory.path().join("consumer.sqlite"), session.owner.id).unwrap(),
    );
    let record = Record {
        id: uuid::Uuid::new_v4(),
        updated_at: 1000,
        is_deleted: false,
        body: serde_json::json!({"text":"body"}),
    };
    producer.edit(record.clone()).unwrap();
    producer.synchronize(&transport).await.unwrap();
    consumer.synchronize(&transport).await.unwrap();
    assert_eq!(
        consumer.record(record.id).unwrap().unwrap().mutation.record,
        record
    );
    producer
        .edit(Record {
            updated_at: 1001,
            is_deleted: true,
            body: serde_json::Value::Null,
            ..record.clone()
        })
        .unwrap();
    producer.synchronize(&transport).await.unwrap();
    consumer.synchronize(&transport).await.unwrap();
    assert_eq!(
        (
            producer.record(record.id).unwrap(),
            consumer.record(record.id).unwrap()
        ),
        (None, None)
    );
    server.abort();
}
