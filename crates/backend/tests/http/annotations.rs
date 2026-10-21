use super::articles::{call, import};
use axum::{Router, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Imports a one-message conversation and returns its conversation and message ids.
async fn import_conversation(app: &Router, cookie: &str, session: &str) -> (String, String) {
    let input = json!({"occurred_at":1_700_000_000_000_i64,"title":"t","source":"chatgpt","session_id":session,"history":r#"[{"role":"user","content":"一句话"}]"#,"idempotency_key":session});
    let (status, imported) = call(app, "POST", "/api/import", cookie, input).await;
    assert_eq!(status, StatusCode::OK);
    let conversation = imported["conversation_id"].as_str().unwrap().to_owned();
    let (_, detail) = call(
        app,
        "GET",
        &format!("/api/conversations/{conversation}"),
        cookie,
        Value::Null,
    )
    .await;
    let message = detail["messages"][0]["id"].as_str().unwrap().to_owned();
    (conversation, message)
}

/// Creating, listing, editing and deleting annotations through the real Router, with every
/// cross-owner request answered as if nothing were there.
pub(super) async fn exercise_annotations(app: &Router, cookie: &str, foreign: &str) {
    let fields = json!({"title":"标注","content":"有一句话值得记住。","url":"https://example.com/annotated","date":"2026-10-05"});
    let (status, article) = import(app, cookie, &fields, &[]).await;
    assert_eq!(status, StatusCode::CREATED);
    let article = article["id"].as_str().unwrap().to_owned();
    let on_article = format!("/api/articles/{article}/annotations");
    let (status, created) = call(
        app,
        "POST",
        &on_article,
        cookie,
        json!({"start":1,"end":4,"quote":"一句话","style":"yellow"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_owned();
    let created_at = created["created_at"].as_i64().unwrap();
    assert_eq!(
        created,
        json!({"id":id,"message_id":null,"start":1,"end":4,"quote":"一句话","style":"yellow","note":null,"created_at":created_at,"updated_at":created_at})
    );
    assert_eq!(
        call(app, "GET", &on_article, cookie, Value::Null).await,
        (StatusCode::OK, json!({"items":[created.clone()]}))
    );

    let (conversation, message) = import_conversation(app, cookie, "annotated").await;
    let on_conversation = format!("/api/conversations/{conversation}/annotations");
    let (status, noted) = call(
        app,
        "POST",
        &on_conversation,
        cookie,
        json!({"message_id":message,"start":0,"end":2,"quote":"一句","note":"记一下"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        (&noted["message_id"], &noted["style"], &noted["note"]),
        (&json!(message), &Value::Null, &json!("记一下"))
    );
    assert_eq!(
        call(app, "GET", &on_conversation, cookie, Value::Null).await,
        (StatusCode::OK, json!({"items":[noted.clone()]}))
    );

    // Invalid parts are named; nothing is written.
    for (body, path) in [
        (json!({"start":1,"end":4,"quote":"一句话"}), "style"),
        (
            json!({"start":1,"end":4,"quote":"一句","style":"blue"}),
            "quote",
        ),
        (
            json!({"start":4,"end":4,"quote":"一句话","style":"blue"}),
            "end",
        ),
        (
            json!({"start":1,"end":4,"quote":"一句话","note":"  "}),
            "note",
        ),
    ] {
        let (status, error) = call(app, "POST", &on_article, cookie, body).await;
        assert_eq!(
            (status, error["path"].clone()),
            (StatusCode::BAD_REQUEST, json!(path))
        );
    }
    let (status, _) = call(
        app,
        "POST",
        &on_article,
        cookie,
        json!({"start":1,"end":4,"quote":"一句话","style":"red"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Another owner sees nothing and changes nothing, even naming its own conversation.
    let not_found = json!({"error":"not_found","login":""});
    let (theirs, _) = import_conversation(app, foreign, "theirs").await;
    for (method, uri, body) in [
        ("GET", on_article.clone(), Value::Null),
        (
            "POST",
            on_article.clone(),
            json!({"start":1,"end":4,"quote":"一句话","style":"blue"}),
        ),
        ("GET", on_conversation.clone(), Value::Null),
        (
            "POST",
            format!("/api/conversations/{theirs}/annotations"),
            json!({"message_id":message,"start":0,"end":2,"quote":"一句","style":"blue"}),
        ),
        (
            "PUT",
            format!("/api/annotations/{id}"),
            json!({"style":"rose"}),
        ),
        ("DELETE", format!("/api/annotations/{id}"), Value::Null),
    ] {
        assert_eq!(
            call(app, method, &uri, foreign, body).await,
            (StatusCode::NOT_FOUND, not_found.clone()),
            "{method} {uri}"
        );
    }

    // An edit replaces both parts and keeps the anchor; clearing both is refused.
    let (status, edited) = call(
        app,
        "PUT",
        &format!("/api/annotations/{id}"),
        cookie,
        json!({"note":"值得"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let updated_at = edited["updated_at"].as_i64().unwrap();
    assert_eq!(
        edited,
        json!({"id":id,"message_id":null,"start":1,"end":4,"quote":"一句话","style":null,"note":"值得","created_at":created_at,"updated_at":updated_at})
    );
    let (status, error) = call(
        app,
        "PUT",
        &format!("/api/annotations/{id}"),
        cookie,
        json!({"style":null,"note":null}),
    )
    .await;
    assert_eq!(
        (status, error["path"].clone()),
        (StatusCode::BAD_REQUEST, json!("style"))
    );
    assert_eq!(
        call(
            app,
            "DELETE",
            &format!("/api/annotations/{id}"),
            cookie,
            Value::Null
        )
        .await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert_eq!(
        call(app, "GET", &on_article, cookie, Value::Null).await,
        (StatusCode::OK, json!({"items":[]}))
    );
}
