use super::articles::call;
use axum::{Router, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const MINUTE: i64 = 60 * 1000;
// 2026-10-31 07:05 UTC.
const RUN: i64 = 1_793_430_300_000;

/// Arranging habits, setting a day's count, retiring, and adding types and activities through
/// the real Router, with bad fields named and every cross-owner request answered as not found.
/// Core test case:
/// - `specs/test-cases/web/habits/habits.md#activity-types-must-be-unique-and-owned`
pub(super) async fn exercise_habits(app: &Router, cookie: &str, foreign: &str) {
    let (status, habit) = call(
        app,
        "POST",
        "/api/habits",
        cookie,
        json!({"name":" 锻炼 ","mark":"锻","times_per_day":2}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = habit["id"].as_str().unwrap().to_owned();
    let created_at = habit["created_at"].as_i64().unwrap();
    assert_eq!(
        habit,
        json!({"id":id,"name":"锻炼","mark":"锻","times_per_day":2,"created_at":created_at,"retired":false})
    );
    assert_eq!(
        call(
            app,
            "POST",
            "/api/habits",
            cookie,
            json!({"name":"读书","mark":"读书","times_per_day":1})
        )
        .await,
        (
            StatusCode::BAD_REQUEST,
            json!({"kind":"field","path":"mark","message":"expected exactly one character"})
        )
    );

    let check = format!("/api/habits/{id}/checks/2026-10-31");
    for _ in 0..2 {
        assert_eq!(
            call(app, "PUT", &check, cookie, json!({"count":1})).await,
            (
                StatusCode::OK,
                json!({"habit_id":id,"day":"2026-10-31","count":1})
            )
        );
    }
    assert_eq!(
        call(app, "PUT", &check, cookie, json!({"count":3})).await.0,
        StatusCode::BAD_REQUEST
    );
    let board = "/api/habits?from=2026-10-30&to=2026-10-31";
    assert_eq!(
        call(app, "GET", board, cookie, Value::Null).await,
        (
            StatusCode::OK,
            json!({"habits":[habit],"checks":[{"habit_id":id,"day":"2026-10-31","count":1}]})
        )
    );
    assert_eq!(
        call(
            app,
            "GET",
            "/api/habits?from=2026-10-31&to=2026-10-30",
            cookie,
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    // Another owner sees nothing and changes nothing.
    assert_eq!(
        call(app, "GET", board, foreign, Value::Null).await.1,
        json!({"habits":[],"checks":[]})
    );
    let habit_path = format!("/api/habits/{id}");
    let retire = format!("/api/habits/{id}/retire");
    for (method, uri, body) in [
        ("PUT", check.as_str(), json!({"count":0})),
        (
            "PUT",
            habit_path.as_str(),
            json!({"name":"偷","mark":"偷","times_per_day":1}),
        ),
        ("POST", retire.as_str(), Value::Null),
    ] {
        assert_eq!(
            call(app, method, uri, foreign, body).await.0,
            StatusCode::NOT_FOUND
        );
    }

    let (status, edited) = call(
        app,
        "PUT",
        &habit_path,
        cookie,
        json!({"name":"运动","mark":"运","times_per_day":1}),
    )
    .await;
    assert_eq!(
        (status, edited["times_per_day"].clone()),
        (StatusCode::OK, json!(1))
    );
    assert_eq!(
        call(app, "POST", &retire, cookie, Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(app, "PUT", &check, cookie, json!({"count":0})).await.0,
        StatusCode::NOT_FOUND
    );

    let (status, running) = call(
        app,
        "POST",
        "/api/activity-types",
        cookie,
        json!({"name":"跑步"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        call(
            app,
            "POST",
            "/api/activity-types",
            cookie,
            json!({"name":" 跑步"})
        )
        .await,
        (StatusCode::OK, running.clone())
    );
    assert_eq!(
        call(app, "GET", "/api/activity-types", cookie, Value::Null).await,
        (StatusCode::OK, json!([running]))
    );
    let type_id = running["id"].as_str().unwrap();
    let (status, run) = call(
        app,
        "POST",
        "/api/activities",
        cookie,
        json!({"type_id":type_id,"started_at":RUN,"minutes":35,"note":" 晨跑 "}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let run_id = run["id"].as_str().unwrap().to_owned();
    assert_eq!(
        run,
        json!({"id":run_id,"type_id":type_id,"type_name":"跑步","started_at":RUN,"minutes":35,"note":"晨跑"})
    );
    let timeline = format!("/api/timeline?start={}&end={}", RUN - MINUTE, RUN + MINUTE);
    assert_eq!(
        call(app, "GET", &timeline, cookie, Value::Null).await.1,
        json!([{"kind":"activity","id":run_id,"occurred_at":RUN,"type_id":type_id,"type_name":"跑步","minutes":35,"note":"晨跑"}])
    );
    let listed = format!(
        "/api/activities?start={}&end={}",
        RUN - MINUTE,
        RUN + MINUTE
    );
    assert_eq!(
        call(app, "GET", &listed, cookie, Value::Null).await,
        (StatusCode::OK, json!([run]))
    );
    assert_eq!(
        call(
            app,
            "POST",
            "/api/activities",
            cookie,
            json!({"type_id":type_id,"started_at":RUN,"minutes":0,"note":null}),
        )
        .await
        .1["path"],
        json!("minutes")
    );
    // Another owner can neither use the type nor touch the activity.
    let activity = format!("/api/activities/{run_id}");
    for (method, uri, body) in [
        (
            "POST",
            "/api/activities",
            json!({"type_id":type_id,"started_at":RUN,"minutes":30,"note":null}),
        ),
        (
            "PUT",
            activity.as_str(),
            json!({"type_id":type_id,"started_at":RUN,"minutes":30,"note":null}),
        ),
        ("DELETE", activity.as_str(), Value::Null),
    ] {
        assert_eq!(
            call(app, method, uri, foreign, body).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let (status, moved) = call(
        app,
        "PUT",
        &activity,
        cookie,
        json!({"type_id":type_id,"started_at":RUN + 10 * MINUTE,"minutes":40,"note":null}),
    )
    .await;
    assert_eq!(
        (status, moved),
        (
            StatusCode::OK,
            json!({"id":run_id,"type_id":type_id,"type_name":"跑步","started_at":RUN + 10 * MINUTE,"minutes":40,"note":null})
        )
    );
    assert_eq!(
        call(app, "DELETE", &activity, cookie, Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(app, "GET", &listed, cookie, Value::Null).await.1,
        json!([])
    );
}
