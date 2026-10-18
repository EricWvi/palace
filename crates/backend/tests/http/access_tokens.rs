use super::{Provider, contract, now};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use palace_backend::{Server, router};
use palace_db::{CredentialKey, Database, IdentityTokens};
use palace_domain::{ImportLimits, SourceLinks};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use testcontainers::{
    GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tower::ServiceExt;

const ORIGIN: &str = "https://palace.test";

/// What a test request carries besides method, path and body.
#[derive(Default)]
struct Credentials<'a> {
    cookie: Option<&'a str>,
    authorization: &'a [&'a str],
    origin: Option<&'a str>,
}

/// The parts of a response these tests compare.
#[derive(Debug, PartialEq)]
struct Answer {
    status: StatusCode,
    set_cookie: bool,
    body: Value,
}

/// Sends a JSON request through the whole router.
async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    credentials: Credentials<'_>,
    body: Value,
) -> Answer {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(cookie) = credentials.cookie {
        request = request.header("cookie", cookie);
    }
    for value in credentials.authorization {
        request = request.header("authorization", *value);
    }
    if let Some(origin) = credentials.origin {
        request = request.header("origin", origin);
    }
    let body = if body.is_null() {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let set_cookie = response.headers().contains_key("set-cookie");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Answer {
        status,
        set_cookie,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

/// A session request, with the configured Origin on unsafe methods.
fn session(cookie: &str) -> Credentials<'_> {
    Credentials {
        cookie: Some(cookie),
        origin: Some(ORIGIN),
        ..Default::default()
    }
}

/// Lists, creates and deletes a token through a session so the main HTTP test covers every
/// management operation in its contract check.
pub(super) async fn exercise_management(app: &Router, cookie: &str) {
    let created = send(
        app,
        "POST",
        "/api/access-tokens",
        session(cookie),
        json!({"note":"cli"}),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let listed = send(
        app,
        "GET",
        "/api/access-tokens",
        session(cookie),
        Value::Null,
    )
    .await;
    assert_eq!(listed.body, json!([created.body["access_token"]]));
    let id = created.body["access_token"]["id"].as_str().unwrap();
    let deleted = send(
        app,
        "DELETE",
        &format!("/api/access-tokens/{id}"),
        session(cookie),
        Value::Null,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
}

/// Tokens authenticate business requests on their own, never manage tokens, and stop working
/// once deleted or disabled; sessions keep their Origin check.
/// Core test cases:
/// - `specs/test-cases/server/owner/access-tokens.md#the-full-access-token-must-leave-the-server-only-in-its-creation-response`
/// - `specs/test-cases/server/owner/access-tokens.md#an-access-token-must-stop-working-as-soon-as-it-is-deleted-or-its-owner-is-disabled`
/// - `specs/test-cases/server/owner/access-tokens.md#a-request-carrying-authorization-must-authenticate-only-by-its-access-token`
/// - `specs/test-cases/server/owner/access-tokens.md#an-access-token-must-never-manage-access-tokens`
/// - `specs/test-cases/server/owner/access-tokens.md#access-tokens-must-stay-within-their-owner`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn access_tokens_authenticate_business_requests_only() {
    let container = palace_testkit::reaped(
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
    let host = container.get_host().await.unwrap();
    let port = container
        .get_host_port_ipv4(/*internal_port*/ 5432)
        .await
        .unwrap();
    let url = format!("postgres://postgres:test@{host}:{port}/postgres");
    let db = Database::connect(&url).await.unwrap();
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
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
    let capture = contract::Capture::default();
    let app = capture.attach(router(Server {
        database: db,
        provider: Provider,
        credential_key: key,
        origin: ORIGIN.into(),
        links: SourceLinks::standard().unwrap(),
        limits: ImportLimits::default(),
        notes: std::sync::Arc::new(palace_notes::NoteLibrary::absent()),
        media: palace_media::MediaStorage::Unconfigured,
        now,
    }));
    let unauthorized = Answer {
        status: StatusCode::UNAUTHORIZED,
        set_cookie: false,
        body: json!({"error":"authentication_required","login":"/auth/login"}),
    };
    let session_required = Answer {
        status: StatusCode::FORBIDDEN,
        set_cookie: false,
        body: json!({"error":"session_required","login":""}),
    };

    // Creation needs a session with its Origin and a valid note.
    let no_origin = Credentials {
        cookie: Some(&cookie),
        ..Default::default()
    };
    assert_eq!(
        send(
            &app,
            "POST",
            "/api/access-tokens",
            no_origin,
            json!({"note":"cli"})
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    for (note, status) in [
        (" ".to_owned(), StatusCode::BAD_REQUEST),
        ("x".repeat(101), StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        assert_eq!(
            send(
                &app,
                "POST",
                "/api/access-tokens",
                session(&cookie),
                json!({"note":note})
            )
            .await
            .status,
            status
        );
    }
    let created = send(
        &app,
        "POST",
        "/api/access-tokens",
        session(&cookie),
        json!({"note":" cli "}),
    )
    .await;
    let token = created.body["token"].as_str().unwrap().to_owned();
    let random = &token["palace_pat_".len()..];
    let masked = format!(
        "palace_pat_{}{}{}",
        &random[..4],
        "*".repeat(35),
        &random[39..]
    );
    assert_eq!(
        created,
        Answer {
            status: StatusCode::CREATED,
            set_cookie: true,
            body: json!({
                "access_token": {
                    "id": created.body["access_token"]["id"],
                    "note": "cli",
                    "masked": masked,
                    "created_at": created.body["access_token"]["created_at"],
                    "last_used_at": null,
                },
                "token": token,
            }),
        }
    );
    // The list shows the masked form only.
    let listed = send(
        &app,
        "GET",
        "/api/access-tokens",
        session(&cookie),
        Value::Null,
    )
    .await;
    assert_eq!(listed.body, json!([created.body["access_token"]]));
    assert!(!listed.body.to_string().contains(&random[4..39]));

    let header = format!("Bearer {token}");
    let bearer = [header.as_str()];
    let bearer_only = || Credentials {
        authorization: &bearer,
        ..Default::default()
    };
    // A valid token reaches business routes as its owner, without cookie or Origin, and a
    // cookie of another owner beside it is ignored.
    let me = json!({"id":sessions[0].owner.id,"email":"a@example.com","identity_id":sessions[0].owner.identity_id});
    for cookie in [None, Some(foreign.as_str())] {
        let credentials = Credentials {
            cookie,
            ..bearer_only()
        };
        assert_eq!(
            send(&app, "GET", "/api/me", credentials, Value::Null).await,
            Answer {
                status: StatusCode::OK,
                set_cookie: false,
                body: me.clone()
            }
        );
    }
    let history = r#"[{"role":"user","content":"hello"}]"#;
    let import = json!({"occurred_at":1700000000000_i64,"title":"t","source":"chatgpt","session_id":"s","history":history,"idempotency_key":"k"});
    let imported = send(&app, "POST", "/api/import", bearer_only(), import.clone()).await;
    assert_eq!(
        (imported.status, imported.set_cookie),
        (StatusCode::OK, false)
    );
    // The token sees only its owner's data.
    let other = send(&app, "POST", "/api/import", session(&foreign), import).await;
    assert_eq!(other.status, StatusCode::OK);
    let mine = send(
        &app,
        "GET",
        "/api/conversations",
        bearer_only(),
        Value::Null,
    )
    .await;
    assert_eq!(
        mine.body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].clone())
            .collect::<Vec<_>>(),
        vec![imported.body["path_id"].clone()]
    );

    // Any Authorization that is not a valid token fails, even beside a valid session cookie.
    let doubled = [bearer[0], bearer[0]];
    let malformed = format!("Bearer {}", &token[..token.len() - 1]);
    let basic = format!("Basic {token}");
    for authorization in [
        &doubled[..],
        &[malformed.as_str()],
        &[basic.as_str()],
        &[token.as_str()],
    ] {
        let credentials = Credentials {
            cookie: Some(&cookie),
            authorization,
            origin: Some(ORIGIN),
        };
        assert_eq!(
            send(&app, "GET", "/api/me", credentials, Value::Null).await,
            unauthorized
        );
    }
    let unknown = format!("Bearer palace_pat_{}", "A".repeat(43));
    let credentials = Credentials {
        authorization: &[unknown.as_str()],
        ..Default::default()
    };
    assert_eq!(
        send(&app, "GET", "/api/me", credentials, Value::Null).await,
        unauthorized
    );

    // A token can neither list, create nor delete tokens, whatever cookie comes with it.
    let id = created.body["access_token"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for (method, uri, body) in [
        ("GET", "/api/access-tokens".to_owned(), Value::Null),
        (
            "POST",
            "/api/access-tokens".to_owned(),
            json!({"note":"backdoor"}),
        ),
        ("DELETE", format!("/api/access-tokens/{id}"), Value::Null),
    ] {
        let credentials = Credentials {
            cookie: Some(&cookie),
            origin: Some(ORIGIN),
            ..bearer_only()
        };
        assert_eq!(
            send(&app, method, &uri, credentials, body).await,
            session_required
        );
    }
    assert_eq!(
        send(
            &app,
            "GET",
            "/api/access-tokens",
            session(&cookie),
            Value::Null
        )
        .await
        .body
        .as_array()
        .unwrap()
        .len(),
        1
    );

    // Another owner cannot delete the token; its owner can, and the token stops at once.
    let foreign_delete = send(
        &app,
        "DELETE",
        &format!("/api/access-tokens/{id}"),
        session(&foreign),
        Value::Null,
    )
    .await;
    assert_eq!(foreign_delete.status, StatusCode::NOT_FOUND);
    assert_eq!(
        send(&app, "GET", "/api/me", bearer_only(), Value::Null)
            .await
            .status,
        StatusCode::OK
    );
    let deleted = send(
        &app,
        "DELETE",
        &format!("/api/access-tokens/{id}"),
        session(&cookie),
        Value::Null,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(
        send(&app, "GET", "/api/me", bearer_only(), Value::Null).await,
        unauthorized
    );

    // Disabling the owner stops its tokens with the same indistinguishable answer.
    let created = send(
        &app,
        "POST",
        "/api/access-tokens",
        session(&foreign),
        json!({"note":"b"}),
    )
    .await;
    let foreign_header = format!("Bearer {}", created.body["token"].as_str().unwrap());
    let foreign_bearer = [foreign_header.as_str()];
    let foreign_token = || Credentials {
        authorization: &foreign_bearer,
        ..Default::default()
    };
    assert_eq!(
        send(&app, "GET", "/api/me", foreign_token(), Value::Null)
            .await
            .status,
        StatusCode::OK
    );
    sqlx::query("UPDATE owner SET disabled=true WHERE id=$1")
        .bind(sessions[1].owner.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        send(&app, "GET", "/api/me", foreign_token(), Value::Null).await,
        unauthorized
    );
    capture.verify(contract::Coverage::ObservedOperations);
}
