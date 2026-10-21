use super::{
    media::{form, send},
    request,
};
use axum::{Router, http::StatusCode};
use http_body_util::BodyExt;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const ORIGIN: &str = "https://palace.test";

/// Sends a JSON or empty request and returns the status and the parsed body (null when empty).
pub(super) async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: &str,
    body: Value,
) -> (StatusCode, Value) {
    let text = if body.is_null() {
        String::new()
    } else {
        body.to_string()
    };
    let response = app
        .clone()
        .oneshot(request(
            method,
            uri,
            cookie,
            ORIGIN,
            "application/json",
            text,
        ))
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

/// Posts an import whose `article` part carries `fields`, followed by `files`.
pub(super) async fn import(
    app: &Router,
    cookie: &str,
    fields: &Value,
    files: &[(&str, &str, &[u8])],
) -> (StatusCode, Value) {
    let article = fields.to_string();
    let mut parts = vec![(
        "article",
        None,
        Some("application/json"),
        article.as_bytes(),
    )];
    for (name, content_type, bytes) in files {
        parts.push(("file", Some(*name), Some(*content_type), *bytes));
    }
    let (status, body) = send(app, "POST", "/api/articles", cookie, ORIGIN, form(&parts)).await;
    (status, serde_json::from_str(&body).unwrap())
}

/// Imports, lists, edits and deletes articles through the real Router without media storage:
/// an article that embeds no file needs none, and one that does is refused whole.
/// Core test cases:
/// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
/// - `specs/test-cases/web/stars/articles.md#importing-the-same-url-must-converge-on-one-article`
/// - `specs/test-cases/web/stars/articles.md#articles-must-stay-within-the-owner`
/// - `specs/test-cases/web/stars/articles.md#article-list-must-order-by-published-date-while-the-timeline-uses-import-time`
/// - `specs/test-cases/web/stars/articles.md#editing-an-article-must-replace-only-its-metadata`
/// - `specs/test-cases/server/media/media-reading.md#reading-media-must-redirect-only-within-the-owner-scope`
pub(super) async fn exercise_without_media(
    app: &Router,
    cookie: &str,
    foreign: &str,
    pool: &PgPool,
) {
    let fields = json!({"title":"标题","content":"# 正文\n","url":"https://www.example.com/a","author":" ","date":"2026-10-05"});
    let (status, created) = import(app, cookie, &fields, &[]).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_owned();
    let occurred_at = created["occurred_at"].as_i64().unwrap();
    assert_eq!(
        created,
        json!({"id":id,"title":"标题","content":"# 正文\n","url":"https://www.example.com/a","author":null,"site":"example.com","published_at":1_791_129_600_000_i64,"date":"2026-10-05","occurred_at":occurred_at,"media":[]})
    );
    // A retry is answered with the stored article, whatever else it carries.
    let mut retry = fields.clone();
    retry["title"] = json!("另一个标题");
    assert_eq!(
        import(app, cookie, &retry, &[]).await,
        (StatusCode::OK, created.clone())
    );

    // Files cannot be kept without storage, and nothing of the article is saved either.
    let embeds = json!({"title":"t","content":"![](media:a.png)","url":"https://example.com/b","date":"2026-10-05"});
    assert_eq!(
        import(app, cookie, &embeds, &[("a.png", "image/png", b"png")]).await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"media_storage_unavailable","login":""})
        )
    );
    for (parts, path) in [
        (form(&[("file", Some("a.png"), None, b"png")]), "article"),
        (form(&[("article", None, None, b"{}")]), "article"),
        (
            form(&[(
                "article",
                None,
                None,
                br#"{"title":"t","content":"c","url":"https://example.com/c","date":"10/05"}"#,
            )]),
            "date",
        ),
    ] {
        let (status, body) = send(app, "POST", "/api/articles", cookie, ORIGIN, parts).await;
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            (status, &body["path"]),
            (StatusCode::BAD_REQUEST, &json!(path))
        );
    }
    let articles: i64 = sqlx::query_scalar("SELECT count(*) FROM article")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(articles, 1);

    // The list and the article belong to their owner only.
    let item = json!({"id":id,"title":"标题","author":null,"url":"https://www.example.com/a","site":"example.com","published_at":1_791_129_600_000_i64,"date":"2026-10-05"});
    assert_eq!(
        call(app, "GET", "/api/articles", cookie, Value::Null).await,
        (
            StatusCode::OK,
            json!({"items":[item],"next_cursor":null,"total":1})
        )
    );
    assert_eq!(
        call(
            app,
            "GET",
            "/api/articles?q=%E6%AD%A3%E6%96%87",
            cookie,
            Value::Null
        )
        .await
        .1["items"][0]["id"],
        json!(id)
    );
    assert_eq!(
        call(app, "GET", "/api/articles", foreign, Value::Null).await,
        (
            StatusCode::OK,
            json!({"items":[],"next_cursor":null,"total":0})
        )
    );
    let path = format!("/api/articles/{id}");
    let not_found = (
        StatusCode::NOT_FOUND,
        json!({"error":"not_found","login":""}),
    );
    assert_eq!(
        call(app, "GET", &path, foreign, Value::Null).await,
        not_found
    );
    assert_eq!(
        call(app, "DELETE", &path, foreign, Value::Null).await,
        not_found
    );
    assert_eq!(
        call(app, "GET", &path, cookie, Value::Null).await,
        (StatusCode::OK, created.clone())
    );

    // The import day holds the article on the timeline.
    let timeline = format!("/api/timeline?start={occurred_at}&end={}", occurred_at + 1);
    assert_eq!(
        call(app, "GET", &timeline, cookie, Value::Null).await,
        (
            StatusCode::OK,
            json!([{"kind":"article","id":id,"occurred_at":occurred_at,"title":"标题","author":null,"site":"example.com"}])
        )
    );

    // Edits replace the metadata, never the body or the import time; a taken URL is a conflict.
    let (_, other) = import(
        app,
        cookie,
        &json!({"title":"另一篇","content":"c","url":"https://example.com/other","date":"2026-10-04"}),
        &[],
    )
    .await;
    let edit = json!({"title":"新题","url":"https://example.com/new","author":"某人","date":"2026-10-05T08:30:00+08:00"});
    let (status, edited) = call(app, "PUT", &path, cookie, edit).await;
    assert_eq!(status, StatusCode::OK);
    let mut expected = created.clone();
    for (field, value) in [
        ("title", json!("新题")),
        ("url", json!("https://example.com/new")),
        ("author", json!("某人")),
        ("site", json!("example.com")),
        ("published_at", json!(1_791_160_200_000_i64)),
        ("date", json!("2026-10-05T08:30:00+08:00")),
    ] {
        expected[field] = value;
    }
    assert_eq!(edited, expected);
    assert_eq!(
        call(
            app,
            "PUT",
            &path,
            cookie,
            json!({"title":"t","url":"https://example.com/other","date":"2026-10-05"}),
        )
        .await,
        (
            StatusCode::CONFLICT,
            json!({"error":"identity_or_request_conflict","login":""})
        )
    );
    assert_eq!(
        call(
            app,
            "PUT",
            &path,
            foreign,
            json!({"title":"t","url":"https://example.com/x","date":"2026-10-05"})
        )
        .await,
        not_found
    );
    assert_eq!(
        call(app, "GET", &path, cookie, Value::Null).await,
        (StatusCode::OK, edited)
    );

    // Without storage there is nothing to read; deleting needs no storage when nothing is embedded.
    assert_eq!(
        call(
            app,
            "GET",
            &format!("/api/media/{}", uuid::Uuid::now_v7()),
            cookie,
            Value::Null
        )
        .await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"media_storage_unavailable","login":""})
        )
    );
    for article in [
        path.clone(),
        format!("/api/articles/{}", other["id"].as_str().unwrap()),
    ] {
        assert_eq!(
            call(app, "DELETE", &article, cookie, Value::Null).await,
            (StatusCode::NO_CONTENT, Value::Null)
        );
    }
    assert_eq!(
        call(app, "GET", &path, cookie, Value::Null).await,
        not_found
    );
}

/// Imports an article with its files, reads them back through redirects to MinIO, refuses to
/// delete them alone, and deletes them with the article.
/// Core test cases:
/// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
/// - `specs/test-cases/web/stars/articles.md#deleting-an-article-must-remove-its-media-objects-before-reporting-success`
/// - `specs/test-cases/server/media/media-reading.md#reading-media-must-redirect-only-within-the-owner-scope`
/// - `specs/test-cases/server/media/media-reading.md#referenced-media-must-not-be-deletable-on-its-own`
pub(super) async fn exercise_with_media(app: &Router, cookie: &str, foreign: &str, pool: &PgPool) {
    // Earlier steps of the media test leave their own rows; only this exercise's are counted.
    let counts = || async {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT (SELECT count(*) FROM media),(SELECT count(*) FROM article)",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    };
    let (media_before, articles_before) = counts().await;
    let fields = json!({"title":"带图","content":"![封面](media:cover.png)\n\n![演示](media:demo.mp4)\n","url":"https://example.com/media","date":"2026-10-05"});
    // A file the body never embeds fails the whole import.
    let (status, body) = import(
        app,
        cookie,
        &fields,
        &[
            ("cover.png", "image/png", b"png"),
            ("demo.mp4", "video/mp4", b"mp4"),
            ("extra.png", "image/png", b"png"),
        ],
    )
    .await;
    assert_eq!(
        (status, &body["path"]),
        (StatusCode::BAD_REQUEST, &json!("file"))
    );
    let (status, article) = import(
        app,
        cookie,
        &fields,
        &[
            ("cover.png", "image/png", b"png"),
            ("demo.mp4", "video/mp4", b"mp4"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let cover = article["media"][0]["id"].as_str().unwrap().to_owned();
    let demo = article["media"][1]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        (&article["content"], &article["media"]),
        (
            &json!(format!(
                "![封面](/api/media/{cover})\n\n![演示](/api/media/{demo})\n"
            )),
            &json!([{"id":cover,"content_type":"image/png"},{"id":demo,"content_type":"video/mp4"}])
        )
    );
    assert_eq!(counts().await, (media_before + 2, articles_before + 1));
    // A retry with other files gets the stored article and stores nothing.
    assert_eq!(
        import(
            app,
            cookie,
            &fields,
            &[("cover.png", "image/png", b"other")]
        )
        .await,
        (StatusCode::OK, article.clone())
    );
    assert_eq!(counts().await, (media_before + 2, articles_before + 1));

    // The owner is redirected to bytes MinIO serves; anyone else sees nothing.
    let media = format!("/api/media/{cover}");
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &media,
            cookie,
            ORIGIN,
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    let cache = response.headers()["cache-control"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cache.starts_with("private, max-age="), "{cache}");
    let location = response.headers()["location"].to_str().unwrap().to_owned();
    let fetched = reqwest::get(&location).await.unwrap();
    assert_eq!(fetched.headers()["content-type"], "image/png");
    assert_eq!(fetched.bytes().await.unwrap().as_ref(), b"png");
    let not_found = (
        StatusCode::NOT_FOUND,
        json!({"error":"not_found","login":""}),
    );
    assert_eq!(
        call(app, "GET", &media, foreign, Value::Null).await,
        not_found
    );
    assert_eq!(
        call(
            app,
            "GET",
            &format!("/api/media/{}", uuid::Uuid::now_v7()),
            cookie,
            Value::Null
        )
        .await,
        not_found
    );
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/media/not-a-uuid",
            cookie,
            ORIGIN,
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // A type no page embeds downloads under its name instead of opening.
    let (status, page) = send(
        app,
        "POST",
        "/api/media",
        cookie,
        ORIGIN,
        form(&[("file", Some("page.html"), Some("text/html"), b"<p>")]),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let page = format!(
        "/api/media/{}",
        serde_json::from_str::<Value>(&page).unwrap()["id"]
            .as_str()
            .unwrap()
    );
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &page,
            cookie,
            ORIGIN,
            "application/json",
            String::new(),
        ))
        .await
        .unwrap();
    let fetched = reqwest::get(response.headers()["location"].to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(
        fetched.headers()["content-disposition"],
        "attachment; filename*=UTF-8''page.html"
    );
    assert_eq!(
        call(app, "DELETE", &page, cookie, Value::Null).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );

    // Embedded media goes only with its article.
    assert_eq!(
        call(app, "DELETE", &media, cookie, Value::Null).await,
        (
            StatusCode::CONFLICT,
            json!({"error":"media_in_use","login":""})
        )
    );
    assert_eq!(
        reqwest::get(&location).await.unwrap().status(),
        reqwest::StatusCode::OK
    );
    let path = format!("/api/articles/{}", article["id"].as_str().unwrap());
    assert_eq!(
        call(app, "DELETE", &path, cookie, Value::Null).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert_eq!(counts().await, (media_before, articles_before));
    assert_eq!(
        call(app, "GET", &media, cookie, Value::Null).await,
        not_found
    );
    assert_eq!(
        reqwest::get(&location).await.unwrap().status(),
        reqwest::StatusCode::NOT_FOUND
    );
}
