use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use palace_db::{Database, Owner};
use palace_domain::{Record, ServerVersion};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Reads the complete response while checking that local requests never create browser sessions.
async fn json_response(app: &Router, request: Request<Body>) -> Value {
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

/// Exercises credential-free writes, stable identity, rejected foreign origins and owner-scoped reads.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn fixed_user_http_reuses_owner_without_login_or_cookies() {
    let directory = tempfile::tempdir().unwrap();
    let postgres =
        super::postgres::Postgres::start(directory.path(), super::postgres::PortBinding::Random)
            .await
            .unwrap();
    let database = Database::connect(&postgres.url).await.unwrap();
    let origin = "http://127.0.0.1:8080";
    let app = palace_backend::fixed_user_router(database.clone(), origin.into())
        .await
        .unwrap();
    let me = json_response(
        &app,
        Request::builder()
            .uri("/api/me")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let owner: Owner = serde_json::from_value(me.clone()).unwrap();
    assert_eq!(owner.email, "local-test@palace.test");

    let other = database
        .resolve_identity("test:foreign", "other", "other@palace.test")
        .await
        .unwrap();
    let foreign = Record {
        id: uuid::Uuid::now_v7(),
        updated_at: 1,
        is_deleted: false,
        body: json!({"text":"foreign"}),
    };
    database
        .upload_records(other.scope(), &[foreign])
        .await
        .unwrap();
    let own = Record {
        id: uuid::Uuid::now_v7(),
        updated_at: 1,
        is_deleted: false,
        body: json!({"text":"local"}),
    };
    let body = serde_json::to_string(&[own]).unwrap();
    for rejected in [None, Some("http://foreign.test")] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/sync")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = rejected {
            request = request.header(header::ORIGIN, origin);
        }
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::from(body.clone())).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/sync")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, origin)
            .body(Body::from(body))
            .unwrap(),
    )
    .await;

    let rebuilt = palace_backend::fixed_user_router(database.clone(), origin.into())
        .await
        .unwrap();
    assert_eq!(
        json_response(
            &rebuilt,
            Request::builder()
                .uri("/api/me")
                .header(
                    header::COOKIE,
                    "__Host-palace-session=invalid; __Host-palace-session=other"
                )
                .header("authorization", "Bearer ignored")
                .body(Body::empty())
                .unwrap()
        )
        .await,
        me
    );
    let page = database
        .pull_records(owner.scope(), ServerVersion::default(), /*limit*/ 100)
        .await
        .unwrap();
    assert_eq!(page.records.len(), 1);
    assert_eq!(
        json_response(
            &rebuilt,
            Request::builder()
                .uri("/api/sync?cursor=0&limit=100")
                .body(Body::empty())
                .unwrap()
        )
        .await,
        serde_json::to_value(page).unwrap()
    );
    for path in ["/auth/login", "/auth/callback", "/auth/logout"] {
        assert_eq!(
            rebuilt
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
}
