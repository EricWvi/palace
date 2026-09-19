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

#[path = "http/paths.rs"]
mod paths;

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
/// Core test cases:
/// - `specs/test-cases/server/import/linear-path-import.md#text-and-file-inputs-must-have-identical-parsing-semantics`
/// - `specs/test-cases/server/owner/owner-isolation.md#owner-scope-must-come-only-from-the-authenticated-server-context`
/// - `specs/test-cases/server/owner/owner-isolation.md#ordinary-operations-must-not-transfer-or-cascade-delete-an-owner`
/// - `specs/test-cases/server/owner/owner-isolation.md#an-inactive-session-past-24-hours-must-revalidate-on-its-next-request`
/// - `specs/test-cases/server/owner/owner-isolation.md#a-browser-session-secret-must-remain-opaque-and-resistant-to-fixation`
/// - `specs/test-cases/server/owner/owner-isolation.md#local-session-revocation-must-take-effect-before-external-logout-succeeds`
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
    let pool = sqlx::PgPool::connect(&format!("postgres://postgres:test@{host}:{port}/postgres"))
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
    let source_hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hits = source_hits.clone();
    let source_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source_url = format!("http://{}/source/", source_listener.local_addr().unwrap());
    let trap = axum::Router::new().fallback(move || {
        let hits = hits.clone();
        async move {
            hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            StatusCode::SERVICE_UNAVAILABLE
        }
    });
    let source_server = tokio::spawn(async move {
        axum::serve(source_listener, trap).await.unwrap();
    });
    let mut links = SourceLinks::standard().unwrap();
    links.chatgpt = url::Url::parse(&source_url).unwrap();
    let app = router(Server {
        database: db,
        provider: Provider,
        credential_key: key,
        origin: "https://palace.test".into(),
        links,
        limits: ImportLimits {
            bytes: 1024,
            ..Default::default()
        },
        now,
    });
    let history = r#"[{"role":"user","content":"<script>alert(1)</script>\r\n","extra":123}]"#;
    let input = serde_json::json!({"occurred_at":1700000000000_i64,"title":"t","source":"chatgpt","session_id":"s","history":history,"idempotency_key":"key"});
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
        ("occurred_at", "1700000000000"),
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
    for (credential, expected) in [
        (
            &cookie,
            serde_json::json!([{"id":result["conversation_id"],"title":"t","source":"chatgpt","session_ids":["s"],"path_id":result["path_id"],"path_count":1,"occurred_at":1700000000000_i64,"head_message_id":result["head_message_id"],"message_count":1}]),
        ),
        (&foreign, serde_json::json!([])),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                "GET",
                "/api/conversations",
                credential,
                "https://palace.test",
                "application/json",
                String::new(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body, expected);
    }
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
        tree,
        serde_json::json!({"conversation":{"id":result["conversation_id"],"owner_id":sessions[0].owner.id,"title":"t","source":"chatgpt"},"messages":[{"id":result["head_message_id"],"owner_id":sessions[0].owner.id,"conversation_id":result["conversation_id"],"parent_message_id":null,"role":"user","content":"<script>alert(1)</script>\r\n","created_order":1}],"paths":[{"id":result["path_id"],"session_id":"s","head_message_id":result["head_message_id"],"message_count":1,"occurred_at":1700000000000_i64,"created_at":tree["paths"][0]["created_at"],"updated_at":tree["paths"][0]["updated_at"],"original_link":format!("{source_url}s")}]})
    );
    assert_eq!(source_hits.load(std::sync::atomic::Ordering::SeqCst), 0);
    source_server.abort();
    for (history, expected_status) in [
        (
            r#"[{"role":"user","content":1}]"#.to_owned(),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!(r#"[{{"role":"user","content":"{}"}}]"#, "x".repeat(1024)),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ] {
        let mut invalid = input.clone();
        invalid["history"] = serde_json::json!(history);
        invalid["session_id"] = serde_json::json!("invalid");
        invalid["idempotency_key"] = serde_json::json!("invalid");
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/import",
                &cookie,
                "https://palace.test",
                "application/json",
                invalid.to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
        let expected: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let mut file = String::new();
        for (name, value) in [
            ("title", "t"),
            ("source", "chatgpt"),
            ("session_id", "invalid"),
            ("history", history.as_str()),
            ("idempotency_key", "invalid"),
            ("occurred_at", "1700000000000"),
        ] {
            file.push_str(&format!(
                "--boundary\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            ));
        }
        file.push_str("--boundary--\r\n");
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/import/file",
                &cookie,
                "https://palace.test",
                "multipart/form-data; boundary=boundary",
                file,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
        let actual: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        // Transport-level size rejection may have a different detail, but category/location remain identical.
        assert_eq!(
            (&actual["kind"], &actual["path"]),
            (&expected["kind"], &expected["path"])
        );
        if expected_status == StatusCode::BAD_REQUEST {
            assert_eq!(actual, expected);
        }
    }
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM conversation),(SELECT count(*) FROM message),(SELECT count(*) FROM conversation_import)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 1, 1));
    paths::exercise_path_lifecycle(&app, &cookie, &foreign).await;
    let anonymous = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/me",
            "",
            "https://palace.test",
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let error: serde_json::Value =
        serde_json::from_slice(&anonymous.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        error,
        serde_json::json!({"error":"authentication_required","login":"/auth/login"})
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

/// Verifies authenticated uploads and incremental pulls through the real HTTP/PostgreSQL boundary.
/// Core test case:
/// - `specs/test-cases/server/owner/owner-isolation.md#owner-sessions-and-oidc-credentials-must-never-enter-business-synchronization`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn http_sync_round_propagates_records_and_tombstones() {
    use palace_domain::Record;
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
    headers.insert("origin", origin.parse().unwrap());
    headers.insert(
        "cookie",
        format!("__Host-palace-session={}", session.secret)
            .parse()
            .unwrap(),
    );
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    let record = Record {
        id: uuid::Uuid::now_v7(),
        updated_at: 1000,
        is_deleted: false,
        body: serde_json::json!({"text":"body"}),
    };
    let response = client
        .post(format!("{origin}/api/sync"))
        .json(&[&record])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!([{"status":"accepted","record":{"ownerId":session.owner.id,"serverVersion":"1","record":record}}])
    );
    let response = client
        .get(format!("{origin}/api/sync?cursor=0&limit=100"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        page,
        serde_json::json!({"records":[{"ownerId":session.owner.id,"serverVersion":"1","record":record}],"cursor":"1"})
    );
    let anonymous = reqwest::Client::new()
        .get(format!("{origin}/api/me"))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let tombstone = Record {
        updated_at: 1001,
        is_deleted: true,
        body: serde_json::Value::Null,
        ..record
    };
    let response = client
        .post(format!("{origin}/api/sync"))
        .json(&[&tombstone])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!([{"status":"accepted","record":{"ownerId":session.owner.id,"serverVersion":"2","record":tombstone}}])
    );
    let response = client
        .get(format!("{origin}/api/sync?cursor=1&limit=100"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"records":[{"ownerId":session.owner.id,"serverVersion":"2","record":tombstone}],"cursor":"2"})
    );
    server.abort();
}
