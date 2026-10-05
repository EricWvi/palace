use super::request;
use axum::{Router, http::StatusCode};
use http_body_util::BodyExt;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Executes real authenticated router calls without mocking database business rules.
async fn call(
    app: &Router,
    cookie: &str,
    method: &str,
    url: &str,
    input: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(
            method,
            url,
            cookie,
            "https://palace.test",
            "application/json",
            input.to_string(),
        ))
        .await
        .unwrap();
    let status = response.status();
    let body =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (status, body)
}
/// Covers branch titles, path metadata edits, unique source links, append-only updates, owner
/// isolation and deletion.
/// Core test cases:
/// - `specs/test-cases/server/import/linear-path-import.md#every-import-must-carry-a-valid-path-title`
/// - `specs/test-cases/server/conversation/message-tree.md#each-path-must-own-its-title-independently`
/// - `specs/test-cases/server/conversation/message-tree.md#source-sessions-must-uniquely-identify-owned-paths`
/// - `specs/test-cases/server/conversation/message-tree.md#metadata-correction-must-atomically-preserve-conversation-tree-identities`
/// - `specs/test-cases/server/import/linear-path-import.md#path-updates-must-retain-the-entire-historical-prefix`
/// - `specs/test-cases/server/import/linear-path-import.md#import-receipts-and-tree-mutations-must-commit-atomically`
pub(super) async fn exercise_path_lifecycle(app: &Router, cookie: &str, foreign: &str) {
    let history = r#"[{"role":"user","content":"U1"},{"role":"assistant","content":"A1"}]"#;
    let root = json!({"title":"branches","source":"chatgpt","session_id":"http-root","history":history,"occurred_at":1000,"idempotency_key":"http-root"});
    let (status, first) = call(app, cookie, "POST", "/api/import", root.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let id = first["conversation_id"].as_str().unwrap();
    let url = format!("/api/conversations/{id}");
    let branch_url = format!("{url}/paths");
    let branch = json!({"title":"branch title","session_id":"http-branch","history":history,"occurred_at":2000,"idempotency_key":"http-branch"});
    let mut untitled = branch.clone();
    untitled.as_object_mut().unwrap().remove("title");
    let mut blank = branch.clone();
    blank["title"] = json!(" ");
    for invalid in [untitled, blank] {
        assert_eq!(
            call(app, cookie, "POST", &branch_url, invalid).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    for (field, value) in [("source", json!("grok")), ("owner_id", json!(id))] {
        let mut invalid = branch.clone();
        invalid[field] = value;
        assert_eq!(
            call(app, cookie, "POST", &branch_url, invalid).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        call(app, foreign, "POST", &branch_url, branch.clone())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, second) = call(app, cookie, "POST", &branch_url, branch.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(app, cookie, "POST", &branch_url, branch).await,
        (StatusCode::OK, second.clone())
    );
    let mut duplicate = root;
    duplicate["idempotency_key"] = json!("different-key");
    assert_eq!(
        call(app, cookie, "POST", "/api/import", duplicate).await.0,
        StatusCode::CONFLICT
    );
    let first_path = first["path_id"].as_str().unwrap();
    let metadata_url = format!("{branch_url}/{first_path}/metadata");
    assert_eq!(
        call(
            app,
            foreign,
            "PUT",
            &metadata_url,
            json!({"title":"foreign","source":"grok"}),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for invalid in [
        json!({"title":"invalid","source":"unknown"}),
        json!({"title":" ","source":"grok"}),
    ] {
        assert_eq!(
            call(app, cookie, "PUT", &metadata_url, invalid).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let legacy = app
        .clone()
        .oneshot(request(
            "PUT",
            &format!("{url}/title"),
            cookie,
            "https://palace.test",
            "application/json",
            json!({"title":"legacy"}).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(legacy.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        call(
            app,
            cookie,
            "PUT",
            &metadata_url,
            json!({"title":"corrected","source":"gemini"}),
        )
        .await,
        (
            StatusCode::OK,
            json!({"conversation_id":id,"path_id":first_path,"title":"corrected","source":"gemini"})
        )
    );
    let (_, corrected) = call(app, cookie, "GET", &url, json!({})).await;
    assert_eq!(corrected["conversation"]["source"], "gemini");
    // Only the addressed path is renamed; the branch keeps the title it was imported with.
    let mut titles: Vec<_> = corrected["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path["title"].as_str().unwrap())
        .collect();
    titles.sort();
    assert_eq!(titles, vec!["branch title", "corrected"]);
    assert!(corrected["paths"].as_array().unwrap().iter().all(|path| {
        path["original_link"]
            .as_str()
            .unwrap()
            .starts_with("https://gemini.google.com/app/")
    }));
    let path_url = format!("{branch_url}/{}", second["path_id"].as_str().unwrap());
    let update = json!({"title":"continued","history":format!("{},{{\"role\":\"user\",\"content\":\"U2\"}}]",&history[..history.len()-1]),"occurred_at":3000,"idempotency_key":"http-update"});
    assert_eq!(
        call(app, cookie, "PUT", &path_url, update).await.0,
        StatusCode::OK
    );
    let short = json!({"title":"continued","history":history,"occurred_at":4000,"idempotency_key":"http-truncate"});
    assert_eq!(
        call(app, cookie, "PUT", &path_url, short).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(app, foreign, "DELETE", &path_url, json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, detail) = call(app, cookie, "GET", &url, json!({})).await;
    let sessions: Vec<_> = detail["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["session_id"].as_str().unwrap())
        .collect();
    assert_eq!(sessions, vec!["http-branch", "http-root"]);
    assert!(
        detail["paths"][0]["original_link"]
            .as_str()
            .unwrap()
            .ends_with("/http-branch")
    );
    assert_eq!(
        call(app, cookie, "GET", &path_url, json!({}))
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        call(app, cookie, "DELETE", &path_url, json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(app, cookie, "DELETE", &url, json!({})).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(app, cookie, "GET", &url, json!({})).await.0,
        StatusCode::NOT_FOUND
    );
}
