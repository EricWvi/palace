use super::request;
use axum::{Router, http::StatusCode};
use http_body_util::BodyExt;
use palace_db::Database;
use palace_jottings::{JottingService, Jottings};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::{fs, process::Command, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://palace.test";
// 2026-10-19 08:30 in Shanghai.
const CREATED: i64 = 1_792_369_800_000;

/// A Git repository with one committed jotting, indexed and served.
pub(super) async fn service(database: Database) -> (tempfile::TempDir, Uuid, JottingService) {
    let folder = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(folder.path())
            .args([
                "-c",
                "user.name=Agent",
                "-c",
                "user.email=agent@palace.test",
            ])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    };
    git(&["init", "--quiet", "--initial-branch=main"]);
    let id = Uuid::now_v7();
    fs::write(
        folder.path().join("agent.md"),
        format!("---\nid: {id}\ncreated: 2026-10-19T08:30:00+08:00\ntitle: 读书\n---\n\n正文\n"),
    )
    .unwrap();
    git(&["add", "--all"]);
    git(&["commit", "--quiet", "-m", "add"]);
    let jottings = Jottings::open(folder.path(), database, chrono_tz::Asia::Shanghai).unwrap();
    jottings.refresh().await.unwrap();
    (folder, id, JottingService::Configured(Arc::new(jottings)))
}

/// Sends one request and returns its status and body, parsed as JSON when it is JSON.
async fn send(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(
            method,
            path,
            cookie,
            ORIGIN,
            "application/json",
            body.to_string(),
        ))
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::String(text)),
    )
}

/// The feed and uploads through the real router: every signed-in user sees the same feed,
/// retries answer 200 with the jotting's version, and a used id answers 409.
/// Core test cases:
/// - `specs/test-cases/server/jotting/jotting-sync.md#retrying-an-upload-must-never-create-a-second-jotting`
/// - `specs/test-cases/server/jotting/jotting-sync.md#uploading-a-different-jotting-under-an-existing-id-must-be-rejected`
/// - `specs/test-cases/server/jotting/jotting-sync.md#pulling-changes-by-cursor-must-not-skip-any-committed-change`
pub(super) async fn exercise_jottings(app: &Router, cookie: &str, foreign: &str, agent: Uuid) {
    let first = json!({
        "changes": [{
            "id": agent, "version": "1", "deleted": false,
            "jotting": {"id": agent, "created_at": CREATED, "title": "读书", "tags": [], "body": "正文\n"},
        }],
        "next": "1",
        "has_more": false,
    });
    for user in [cookie, foreign] {
        assert_eq!(
            send(
                app,
                "GET",
                "/api/jottings/changes?since=0",
                user,
                Value::Null
            )
            .await,
            (StatusCode::OK, first.clone())
        );
    }

    let id = Uuid::now_v7();
    let upload = json!({"id": id, "created_at": CREATED, "tags": ["phone"], "body": "\n离线写的"});
    let accepted = json!({"id": id, "version": "2"});
    assert_eq!(
        send(app, "POST", "/api/jottings", cookie, upload.clone()).await,
        (StatusCode::CREATED, accepted.clone())
    );
    assert_eq!(
        send(app, "POST", "/api/jottings", foreign, upload).await,
        (StatusCode::OK, accepted)
    );
    assert_eq!(
        send(
            app,
            "POST",
            "/api/jottings",
            cookie,
            json!({"id": agent, "created_at": CREATED, "body": "x"})
        )
        .await,
        (
            StatusCode::CONFLICT,
            json!({"error": "jotting_id_conflict", "login": ""})
        )
    );
    assert_eq!(
        send(
            app,
            "POST",
            "/api/jottings",
            cookie,
            json!({"id": Uuid::now_v7(), "created_at": CREATED, "title": " ", "body": "x"})
        )
        .await,
        (
            StatusCode::BAD_REQUEST,
            json!({"kind": "field", "path": "title", "message": "expected a non-blank title or null"})
        )
    );
    let (status, _) = send(
        app,
        "POST",
        "/api/jottings",
        cookie,
        json!({"id": Uuid::now_v7(), "created_at": CREATED, "body": "x", "owner_id": agent}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = send(
        app,
        "GET",
        "/api/jottings/changes?since=01",
        cookie,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    assert_eq!(
        send(
            app,
            "GET",
            "/api/jottings/changes?since=1&limit=1",
            cookie,
            Value::Null
        )
        .await,
        (
            StatusCode::OK,
            json!({
                "changes": [{
                    "id": id, "version": "2", "deleted": false,
                    "jotting": {"id": id, "created_at": CREATED, "title": null, "tags": ["phone"], "body": "离线写的"},
                }],
                "next": "2",
                "has_more": false,
            })
        )
    );
    exercise_reading(app, cookie, agent, id).await;
}

/// The 回响 reads through the real router, after `exercise_jottings` left the agent's committed
/// jotting and one upload still in the inbox, both created at `CREATED`.
/// Core test cases:
/// - `specs/test-cases/web/echo/jotting-reading.md#jotting-list-must-page-every-live-jotting-exactly-once-newest-first`
/// - `specs/test-cases/web/echo/jotting-reading.md#pending-must-mean-the-jotting-file-sits-under-inbox`
/// - `specs/test-cases/web/echo/jotting-reading.md#overview-must-bucket-jottings-by-the-deployments-local-day`
async fn exercise_reading(app: &Router, cookie: &str, agent: Uuid, upload: Uuid) {
    let get = |path: String| async move { send(app, "GET", &path, cookie, Value::Null).await };
    let uploaded = json!({"id": upload, "created_at": CREATED, "title": null, "tags": ["phone"], "body": "离线写的", "pending": true});
    let committed = json!({"id": agent, "created_at": CREATED, "title": "读书", "tags": [], "body": "正文\n", "pending": false});
    // Equal times fall back to the id, and the upload's v7 id is the later one.
    let (status, first) = get("/api/jottings?limit=1".into()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (&first["jottings"], &first["total"]),
        (&json!([uploaded]), &json!(2))
    );
    let cursor = first["next_cursor"].as_str().unwrap();
    assert_eq!(
        get(format!("/api/jottings?limit=1&cursor={cursor}")).await,
        (
            StatusCode::OK,
            json!({"jottings": [committed], "total": 2, "next_cursor": null})
        )
    );
    assert_eq!(
        get("/api/jottings?tag=phone&q=%E7%A6%BB%E7%BA%BF".into()).await,
        (
            StatusCode::OK,
            json!({"jottings": [uploaded], "total": 1, "next_cursor": null})
        )
    );

    let today = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Shanghai)
        .date_naive()
        .to_string();
    assert_eq!(
        get(format!("/api/jottings/overview?from={today}")).await,
        (
            StatusCode::OK,
            json!({"total": 2, "days": 1, "tags": [{"name": "phone", "count": 1}], "daily": [{"date": today, "count": 0}]})
        )
    );
    let (status, recall) = get("/api/jottings/recall".into()).await;
    assert_eq!(status, StatusCode::OK);
    let mut ids: Vec<&str> = recall["jottings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|jotting| jotting["id"].as_str().unwrap())
        .collect();
    ids.sort_unstable();
    let mut expected = [agent.to_string(), upload.to_string()];
    expected.sort_unstable();
    assert_eq!(
        (recall["date"].as_str(), ids),
        (
            Some(today.as_str()),
            expected.iter().map(String::as_str).collect()
        )
    );

    for (path, field, message) in [
        (
            "/api/jottings?cursor=bad",
            "cursor",
            "expected a list cursor",
        ),
        ("/api/jottings?limit=201", "limit", "expected 1..200"),
        (
            "/api/jottings/overview?from=2026-1-1",
            "from",
            "expected YYYY-MM-DD",
        ),
        (
            "/api/jottings/overview?from=2020-01-01",
            "from",
            "expected a day within the last year",
        ),
    ] {
        assert_eq!(
            get(path.into()).await,
            (
                StatusCode::BAD_REQUEST,
                json!({"kind": "field", "path": field, "message": message})
            ),
            "{path}"
        );
    }
}

/// Without a folder the feed is empty at the client's cursor and uploads are unavailable.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#jotting-configuration-must-fail-fast-when-misconfigured`
pub(super) async fn exercise_unconfigured(app: &Router, cookie: &str) {
    assert_eq!(
        send(
            app,
            "GET",
            "/api/jottings/changes?since=5",
            cookie,
            Value::Null
        )
        .await,
        (
            StatusCode::OK,
            json!({"changes": [], "next": "5", "has_more": false})
        )
    );
    assert_eq!(
        send(
            app,
            "POST",
            "/api/jottings",
            cookie,
            json!({"id": Uuid::now_v7(), "created_at": CREATED, "body": "x"})
        )
        .await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error": "jottings_unavailable", "login": ""})
        )
    );
    for path in [
        "/api/jottings",
        "/api/jottings/overview?from=2026-10-19",
        "/api/jottings/recall",
    ] {
        assert_eq!(
            send(app, "GET", path, cookie, Value::Null).await,
            (
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error": "jottings_unavailable", "login": ""})
            ),
            "{path}"
        );
    }
}
