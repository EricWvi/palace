use super::request;
use axum::{Router, http::StatusCode};
use http_body_util::BodyExt;
use palace_notes::{CACHE_FILE, NoteLibrary, Ripgrep};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::{fs, sync::Arc};
use tower::ServiceExt;

/// A notes folder with one note in a subfolder, a note the cache does not list, and a file
/// outside the folder that no request may read.
pub(super) fn library() -> (tempfile::TempDir, Arc<NoteLibrary>) {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path().join("notes");
    fs::create_dir_all(root.join("旅行")).unwrap();
    fs::write(folder.path().join("secret.md"), "secret").unwrap();
    fs::write(
        root.join("旅行").join("周末.md"),
        "---\ntitle: 周末\ndate: 2026-10-05\n---\n\n# 计划\n",
    )
    .unwrap();
    fs::write(root.join("unlisted.md"), "---\ntitle: 未入\n---\n计划\n").unwrap();
    fs::write(
        root.join(CACHE_FILE),
        r#"{"version":1,"notes":[{"id":"旅行/周末","title":"周末","date":"2026-10-05"}]}"#,
    )
    .unwrap();
    let library = NoteLibrary::open(root, chrono_tz::Asia::Shanghai, Ripgrep::default());
    (folder, Arc::new(library))
}

/// Percent-encodes a note id as the single path segment the API expects.
fn segment(id: &str) -> String {
    url::form_urlencoded::byte_serialize(id.as_bytes()).collect()
}

async fn get(app: &Router, cookie: &str, url: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            url,
            cookie,
            "https://palace.test",
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    let status = response.status();
    let body =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (status, body)
}

/// Every signed-in owner reads the same notes; anonymous requests and ids outside the index are
/// refused.
/// Core test cases:
/// - `specs/test-cases/web/stars/notes.md#notes-must-be-read-only-and-readable-by-every-signed-in-user`
/// - `specs/test-cases/web/stars/notes.md#note-page-must-read-only-files-listed-in-the-index`
pub(super) async fn exercise_notes(app: &Router, cookie: &str, foreign: &str) {
    // 2026-10-05 00:00 in Shanghai.
    let occurred_at = 1_791_129_600_000_i64;
    let item = json!({"id":"旅行/周末","title":"周末","occurred_at":occurred_at});
    let detail =
        json!({"id":"旅行/周末","title":"周末","occurred_at":occurred_at,"content":"# 计划\n"});
    let note = format!("/api/notes/{}", segment("旅行/周末"));
    for credential in [cookie, foreign] {
        assert_eq!(
            get(app, credential, "/api/notes").await,
            (
                StatusCode::OK,
                json!({"items":[item],"next_cursor":null,"total":1})
            )
        );
        // The unlisted file contains the term too, but only indexed notes are results.
        assert_eq!(
            get(
                app,
                credential,
                &format!("/api/notes?q={}", segment("计划"))
            )
            .await,
            (
                StatusCode::OK,
                json!({"items":[item],"next_cursor":null,"total":1})
            )
        );
        assert_eq!(
            get(app, credential, &note).await,
            (StatusCode::OK, detail.clone())
        );
    }
    assert_eq!(
        get(app, cookie, "/api/notes?cursor=nonsense").await.0,
        StatusCode::BAD_REQUEST
    );
    for id in ["../secret", "unlisted", "旅行"] {
        assert_eq!(
            get(app, cookie, &format!("/api/notes/{}", segment(id))).await,
            (
                StatusCode::NOT_FOUND,
                json!({"error":"not_found","login":""})
            ),
            "{id}"
        );
    }
    for url in ["/api/notes", note.as_str()] {
        assert_eq!(get(app, "", url).await.0, StatusCode::UNAUTHORIZED);
    }
}
