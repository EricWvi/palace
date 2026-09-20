use super::request;
use axum::{Router, http::StatusCode};
use http_body_util::BodyExt;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Exercises decoder failures and stateful serialization against authenticated production routes.
pub(super) async fn exercise_contract_boundaries(
    app: &Router,
    cookie: &str,
    foreign: &str,
    pool: &sqlx::PgPool,
    owner: &palace_db::Owner,
) {
    let valid = json!({
        "title":"contract", "source":"chatgpt", "session_id":"contract",
        "occurred_at":1000, "history":r#"[{"role":"user","content":"hello"}]"#,
        "idempotency_key":"contract",
    });
    let mut cases: Vec<(&str, String, &str, String, StatusCode)> = Vec::new();
    for field in [
        "title",
        "source",
        "session_id",
        "occurred_at",
        "history",
        "idempotency_key",
    ] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        let mut null = valid.clone();
        null[field] = Value::Null;
        for input in [missing, null] {
            cases.push((
                "POST",
                "/api/import".into(),
                "application/json",
                input.to_string(),
                StatusCode::BAD_REQUEST,
            ));
        }
    }
    for (field, value) in [
        ("source", json!("unknown")),
        ("occurred_at", json!("1000")),
        ("owner_id", json!(uuid::Uuid::nil())),
    ] {
        let mut input = valid.clone();
        input[field] = value;
        cases.push((
            "POST",
            "/api/import".into(),
            "application/json",
            input.to_string(),
            StatusCode::BAD_REQUEST,
        ));
    }
    let record = json!({"id":uuid::Uuid::now_v7(),"updatedAt":1000,"isDeleted":false,"body":null});
    let mut missing = record.clone();
    missing.as_object_mut().unwrap().remove("body");
    let mut unknown = record.clone();
    unknown["ownerId"] = json!(uuid::Uuid::nil());
    let mut wrong = record.clone();
    wrong["isDeleted"] = json!("false");
    for input in [missing, unknown, wrong] {
        cases.push((
            "POST",
            "/api/sync".into(),
            "application/json",
            json!([input]).to_string(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ));
    }
    for cursor in ["01", "-1", "9223372036854775808", "null"] {
        cases.push((
            "GET",
            format!("/api/sync?cursor={cursor}&limit=10"),
            "application/json",
            String::new(),
            StatusCode::BAD_REQUEST,
        ));
    }
    for query in [
        "cursor=0",
        "limit=10",
        "cursor=0&limit=0",
        "cursor=0&limit=1001",
    ] {
        cases.push((
            "GET",
            format!("/api/sync?{query}"),
            "application/json",
            String::new(),
            StatusCode::BAD_REQUEST,
        ));
    }
    cases.extend([
        (
            "POST",
            "/api/sync".into(),
            "application/json",
            "[".into(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "POST",
            "/api/sync".into(),
            "text/plain",
            "[]".into(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            "POST",
            "/api/sync".into(),
            "application/json",
            " ".repeat(80_000),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            "POST",
            "/api/sync".into(),
            "application/json",
            json!(vec![record.clone(); 101]).to_string(),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            "POST",
            "/api/import/file".into(),
            "multipart/form-data",
            String::new(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "GET",
            "/api/conversations/not-a-uuid".into(),
            "application/json",
            String::new(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "GET",
            "/auth/callback".into(),
            "application/json",
            String::new(),
            StatusCode::BAD_REQUEST,
        ),
        (
            "GET",
            "/auth/callback?code=invalid&state=invalid".into(),
            "application/json",
            String::new(),
            StatusCode::UNAUTHORIZED,
        ),
        (
            "GET",
            "/api/me".into(),
            "application/json",
            String::new(),
            StatusCode::OK,
        ),
        (
            "GET",
            "/auth/login".into(),
            "application/json",
            String::new(),
            StatusCode::SEE_OTHER,
        ),
    ]);
    for (method, path, media, body, expected) in cases {
        let response = app
            .clone()
            .oneshot(request(
                method,
                &path,
                cookie,
                "https://palace.test",
                media,
                body,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{method} {path}");
    }

    // A real PostgreSQL sequence above 2^53 proves the wire cursor never passes through a JS number.
    sqlx::query("SELECT setval('business_server_version', 9007199254740993, false)")
        .execute(pool)
        .await
        .unwrap();
    for expected_status in ["accepted", "retained"] {
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/sync",
                cookie,
                "https://palace.test",
                "application/json",
                json!([record]).to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(
            body,
            json!([{
                "status": expected_status,
                "record": {"ownerId":owner.id, "serverVersion":"9007199254740993", "record":record}
            }])
        );
    }
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/sync?cursor=9007199254740992&limit=10",
            cookie,
            "https://palace.test",
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        page,
        json!({
            "cursor":"9007199254740993",
            "records":[{"ownerId":owner.id, "serverVersion":"9007199254740993", "record":record}]
        })
    );
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/auth/logout-all",
            foreign,
            "https://palace.test",
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
