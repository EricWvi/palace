use super::articles::{call, import};
use axum::{Router, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const PAGE: &str = "https://blog.example.com/2026/overall-note";

/// Each path of a conversation keeps its own note.
async fn exercise_path(app: &Router, cookie: &str, foreign: &str) {
    let history = r#"[{"role":"user","content":"U"},{"role":"assistant","content":"A"}]"#;
    let root = json!({"title":"总批","source":"chatgpt","session_id":"overall-note","history":history,"occurred_at":1000,"idempotency_key":"overall-note"});
    let (status, imported) = call(app, "POST", "/api/import", cookie, root).await;
    assert_eq!(status, StatusCode::OK);
    let (conversation, path) = (
        imported["conversation_id"].as_str().unwrap(),
        imported["path_id"].as_str().unwrap(),
    );
    let note = format!("/api/conversations/{conversation}/paths/{path}/overall-note");
    assert_eq!(
        call(app, "GET", &note, cookie, Value::Null).await,
        (StatusCode::OK, json!({"text":null,"version":0}))
    );
    let (status, saved) = call(
        app,
        "PUT",
        &note,
        cookie,
        json!({"text":"这一支","version":0}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(app, "GET", &note, cookie, Value::Null).await,
        (StatusCode::OK, saved)
    );
    assert_eq!(
        call(app, "GET", &note, foreign, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
}

/// A note written on a web page moves into the article it is bound to, and a stale save or
/// another owner touches nothing, through the real Router.
/// Core test cases:
/// - `specs/test-cases/web/stars/overall-notes.md#an-overall-note-must-belong-to-exactly-one-document-and-go-with-it`
/// - `specs/test-cases/web/stars/overall-notes.md#binding-a-web-page-must-carry-its-overall-note-into-the-article-whole`
pub(super) async fn exercise_overall_notes(app: &Router, cookie: &str, foreign: &str) {
    exercise_path(app, cookie, foreign).await;
    let fields = json!({"title":"总批","content":"正文\n","url":PAGE,"date":"2026-10-05"});
    let (status, article) = import(app, cookie, &fields, &[]).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = article["id"].as_str().unwrap();
    let note = format!("/api/articles/{id}/overall-note");

    assert_eq!(
        call(app, "GET", &note, cookie, Value::Null).await,
        (StatusCode::OK, json!({"text":null,"version":0}))
    );
    let not_found = (
        StatusCode::NOT_FOUND,
        json!({"error":"not_found","login":""}),
    );
    assert_eq!(
        call(app, "GET", &note, foreign, Value::Null).await,
        not_found
    );

    let save = json!({"url":PAGE,"title":"标签页","text":"网页上的想法","version":0});
    let (status, saved) = call(app, "PUT", "/api/web-pages/overall-note", cookie, save).await;
    assert_eq!(status, StatusCode::OK);
    let version = saved["version"].as_i64().unwrap();
    assert_eq!(saved, json!({"text":"网页上的想法","version":version}));
    let stale = json!({"url":PAGE,"title":"标签页","text":"过期","version":0});
    assert_eq!(
        call(app, "PUT", "/api/web-pages/overall-note", cookie, stale).await,
        (
            StatusCode::CONFLICT,
            json!({"error":"overall_note_changed","login":""})
        )
    );

    let binding = json!({"url":PAGE,"title":"标签页","article_id":id});
    let (status, _) = call(app, "PUT", "/api/web-pages/binding", cookie, binding).await;
    assert_eq!(status, StatusCode::OK);
    let (status, moved) = call(app, "GET", &note, cookie, Value::Null).await;
    assert_eq!(
        (status, &moved["text"]),
        (StatusCode::OK, &json!("网页上的想法"))
    );
    let version = moved["version"].as_i64().unwrap();
    // Once bound the page reads its article's note and has none of its own to write.
    let late = json!({"url":PAGE,"title":"标签页","text":"迟到","version":0});
    assert_eq!(
        call(app, "PUT", "/api/web-pages/overall-note", cookie, late).await,
        (
            StatusCode::CONFLICT,
            json!({"error":"web_page_bound","login":""})
        )
    );

    let (status, saved) = call(
        app,
        "PUT",
        &note,
        cookie,
        json!({"text":"改过","version":version}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let version = saved["version"].as_i64().unwrap();
    assert_eq!(
        call(
            app,
            "PUT",
            &note,
            cookie,
            json!({"text":" ","version":version})
        )
        .await,
        (StatusCode::OK, json!({"text":null,"version":0}))
    );
}
