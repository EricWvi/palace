use super::articles::call;
use axum::{Router, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const HOUR: i64 = 60 * 60 * 1000;
// 2025-10-28 23:30 UTC.
const BED: i64 = 1_761_694_200_000;

/// Starting, ending, recording, correcting, listing and deleting nights through the real Router,
/// with overlaps refused, bad fields named and every cross-owner request answered as not found.
/// Core test case:
/// - `specs/test-cases/web/sleep/sleep.md#sleep-must-stay-within-the-owner-scope`
pub(super) async fn exercise_sleep(app: &Router, cookie: &str, foreign: &str) {
    assert_eq!(
        call(app, "GET", "/api/sleep/running", cookie, Value::Null).await,
        (StatusCode::OK, json!({"started_at":null}))
    );
    let (status, started) = call(app, "PUT", "/api/sleep/running", cookie, Value::Null).await;
    let started_at = started["started_at"].as_i64().unwrap();
    assert_eq!(
        (
            status,
            call(app, "GET", "/api/sleep/running", cookie, Value::Null).await
        ),
        (StatusCode::OK, (StatusCode::OK, started.clone()))
    );
    // Another owner cannot end it, and a start nobody saw is not ended either.
    for (who, seen) in [(foreign, started_at), (cookie, started_at - 1)] {
        assert_eq!(
            call(
                app,
                "POST",
                "/api/sleep/running/end",
                who,
                json!({"started_at":seen})
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }

    let (status, recorded) = call(
        app,
        "POST",
        "/api/sleep/nights",
        cookie,
        json!({"started_at":BED,"ended_at":BED + 7 * HOUR}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = recorded["id"].as_str().unwrap().to_owned();
    assert_eq!(
        recorded,
        json!({"id":id,"started_at":BED,"ended_at":BED + 7 * HOUR})
    );
    let timeline = format!(
        "/api/timeline?start={}&end={}",
        BED + 6 * HOUR,
        BED + 8 * HOUR
    );
    assert_eq!(
        call(app, "GET", &timeline, cookie, Value::Null).await.1,
        json!([{"kind":"sleep","id":id,"occurred_at":BED + 7 * HOUR,"started_at":BED}])
    );
    assert_eq!(
        call(
            app,
            "POST",
            "/api/sleep/nights",
            cookie,
            json!({"started_at":BED + HOUR,"ended_at":BED + 2 * HOUR}),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let night = format!("/api/sleep/nights/{id}");
    let (status, moved) = call(
        app,
        "PUT",
        &night,
        cookie,
        json!({"started_at":BED - HOUR,"ended_at":BED + 6 * HOUR}),
    )
    .await;
    assert_eq!(
        (status, moved),
        (
            StatusCode::OK,
            json!({"id":id,"started_at":BED - HOUR,"ended_at":BED + 6 * HOUR})
        )
    );
    let range = format!("/api/sleep/nights?start={}&end={}", BED, BED + 24 * HOUR);
    assert_eq!(
        call(app, "GET", &range, cookie, Value::Null).await,
        (
            StatusCode::OK,
            json!([{"id":id,"started_at":BED - HOUR,"ended_at":BED + 6 * HOUR}])
        )
    );

    // Invalid parts are named; nothing is written.
    for (method, uri, body, path) in [
        (
            "POST",
            "/api/sleep/nights".to_owned(),
            json!({"started_at":BED,"ended_at":BED}),
            "ended_at",
        ),
        (
            "POST",
            "/api/sleep/nights".to_owned(),
            json!({"started_at":BED,"ended_at":BED + 25 * HOUR}),
            "ended_at",
        ),
        (
            "PUT",
            night.clone(),
            json!({"started_at":-1,"ended_at":BED}),
            "started_at",
        ),
        (
            "GET",
            format!(
                "/api/sleep/nights?start={BED}&end={}",
                BED + 401 * 24 * HOUR
            ),
            Value::Null,
            "end",
        ),
    ] {
        let (status, error) = call(app, method, &uri, cookie, body).await;
        assert_eq!(
            (status, &error["path"]),
            (StatusCode::BAD_REQUEST, &json!(path)),
            "{uri}"
        );
    }

    // Another owner meets nothing at all.
    for (method, body) in [
        ("PUT", json!({"started_at":BED,"ended_at":BED + HOUR})),
        ("DELETE", Value::Null),
    ] {
        assert_eq!(
            call(app, method, &night, foreign, body).await.0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        (
            call(app, "GET", &range, foreign, Value::Null).await.1,
            call(app, "GET", "/api/sleep/running", foreign, Value::Null)
                .await
                .1
        ),
        (json!([]), json!({"started_at":null}))
    );

    assert_eq!(
        call(app, "DELETE", &night, cookie, Value::Null).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert_eq!(
        call(app, "GET", &range, cookie, Value::Null).await.1,
        json!([])
    );
}
