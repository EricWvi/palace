use super::articles::{call, import};
use axum::{Router, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const PAGE: &str = "https://blog.example.com/2026/web-page";

/// Annotating a web page, binding it, listing candidates near it and migrating its annotations
/// through the real Router, with every cross-owner request answered as if nothing were there.
/// Core test cases:
/// - `specs/test-cases/extension/web-annotations.md#web-pages-and-web-annotations-must-stay-within-the-owner-scope`
/// - `specs/test-cases/extension/web-annotations.md#a-bound-web-page-must-reject-new-and-changed-web-annotations`
pub(super) async fn exercise_web_pages(app: &Router, cookie: &str, foreign: &str) {
    let mark = |quote: &str, style: &str| json!({"url":PAGE,"title":"原网页","quote":quote,"prefix":"前","suffix":"后","position":0.5,"style":style});
    let (status, first) = call(
        app,
        "POST",
        "/api/web-pages/annotations",
        cookie,
        mark("值得", "yellow"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, second) = call(
        app,
        "POST",
        "/api/web-pages/annotations",
        cookie,
        mark("对不上", "rose"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = first["id"].as_str().unwrap().to_owned();
    let page_id = first["page_id"].as_str().unwrap().to_owned();
    let created_at = first["created_at"].as_i64().unwrap();
    assert_eq!(
        first,
        json!({"id":id,"page_id":page_id,"quote":"值得","prefix":"前","suffix":"后","position":0.5,"style":"yellow","note":null,"created_at":created_at,"updated_at":created_at})
    );

    let lookup = format!("/api/web-pages/lookup?url={}", urlencode(PAGE));
    let (status, detail) = call(app, "GET", &lookup, cookie, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let updated_at = detail["page"]["updated_at"].as_i64().unwrap();
    assert_eq!(
        detail,
        json!({"page":{"id":page_id,"url":PAGE,"title":"原网页","article_id":null,"updated_at":updated_at},"annotations":[first.clone(),second.clone()]})
    );
    assert_eq!(
        call(app, "GET", "/api/web-pages", cookie, Value::Null).await,
        (
            StatusCode::OK,
            json!({"items":[{"id":page_id,"url":PAGE,"title":"原网页","article_id":null,"updated_at":updated_at,"annotation_count":2}]})
        )
    );

    // Invalid parts are named; a fragment is the client's to strip.
    for (body, path) in [
        (
            json!({"url":format!("{PAGE}#top"),"title":"","quote":"q","prefix":"","suffix":"","position":0,"style":"blue"}),
            "url",
        ),
        (
            json!({"url":PAGE,"title":"","quote":" ","prefix":"","suffix":"","position":0,"style":"blue"}),
            "quote",
        ),
        (
            json!({"url":PAGE,"title":"","quote":"q","prefix":"","suffix":"","position":2,"style":"blue"}),
            "position",
        ),
        (
            json!({"url":PAGE,"title":"","quote":"q","prefix":"","suffix":"","position":0}),
            "style",
        ),
    ] {
        let (status, error) = call(app, "POST", "/api/web-pages/annotations", cookie, body).await;
        assert_eq!(
            (status, error["path"].clone()),
            (StatusCode::BAD_REQUEST, json!(path))
        );
    }
    let (status, error) = call(
        app,
        "GET",
        "/api/web-pages/lookup?url=relative",
        cookie,
        Value::Null,
    )
    .await;
    assert_eq!(
        (status, error["path"].clone()),
        (StatusCode::BAD_REQUEST, json!("url"))
    );

    let (status, edited) = call(
        app,
        "PUT",
        &format!("/api/web-annotations/{id}"),
        cookie,
        json!({"title":"新标题","style":"green","note":"想法"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let edited_at = edited["updated_at"].as_i64().unwrap();
    assert_eq!(
        edited,
        json!({"id":id,"page_id":page_id,"quote":"值得","prefix":"前","suffix":"后","position":0.5,"style":"green","note":"想法","created_at":created_at,"updated_at":edited_at})
    );

    // Candidates near the page: the same host and section first.
    let fields = |title: &str, url: &str| json!({"title":title,"content":format!("{title}：这句话值得记住。"),"url":url,"date":"2026-10-05"});
    let (_, article) = import(
        app,
        cookie,
        &fields(
            "网页文章",
            "https://blog.example.com/2026/web-page?from=feed",
        ),
        &[],
    )
    .await;
    let (_, elsewhere) = import(
        app,
        cookie,
        &fields("别处文章", "https://other.example.org/x"),
        &[],
    )
    .await;
    let article_id = article["id"].as_str().unwrap().to_owned();
    let near = format!("/api/articles?near={}", urlencode(PAGE));
    let (status, candidates) = call(app, "GET", &near, cookie, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let ranked: Vec<&Value> = candidates["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| &item["title"])
        .collect();
    assert_eq!(ranked.first(), Some(&&json!("网页文章")));
    assert!(ranked.contains(&&json!("别处文章")));
    assert_eq!(
        candidates["items"][0]["url"],
        json!("https://blog.example.com/2026/web-page?from=feed")
    );
    let (status, error) = call(
        app,
        "GET",
        &format!("{near}&cursor=1_{}", elsewhere["id"].as_str().unwrap()),
        cookie,
        Value::Null,
    )
    .await;
    assert_eq!(
        (status, error["path"].clone()),
        (StatusCode::BAD_REQUEST, json!("cursor"))
    );

    // Another owner sees nothing and changes nothing.
    let not_found = json!({"error":"not_found","login":""});
    let on_article = format!("/api/articles/{article_id}/web-annotations");
    for (method, uri, body) in [
        ("GET", lookup.clone(), Value::Null),
        (
            "PUT",
            format!("/api/web-annotations/{id}"),
            json!({"title":"","style":"rose"}),
        ),
        ("DELETE", format!("/api/web-annotations/{id}"), Value::Null),
        (
            "PUT",
            "/api/web-pages/binding".to_owned(),
            json!({"url":PAGE,"title":"","article_id":article_id}),
        ),
        ("GET", on_article.clone(), Value::Null),
        ("POST", format!("{on_article}/migrate"), json!({"items":[]})),
    ] {
        assert_eq!(
            call(app, method, &uri, foreign, body).await,
            (StatusCode::NOT_FOUND, not_found.clone()),
            "{method} {uri}"
        );
    }
    assert_eq!(
        call(app, "GET", "/api/web-pages", foreign, Value::Null).await,
        (StatusCode::OK, json!({"items":[]}))
    );

    // Binding stops the page taking annotations of its own.
    let (status, bound) = call(
        app,
        "PUT",
        "/api/web-pages/binding",
        cookie,
        json!({"url":PAGE,"title":"","article_id":article_id}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let bound_at = bound["updated_at"].as_i64().unwrap();
    assert_eq!(
        bound,
        json!({"id":page_id,"url":PAGE,"title":PAGE,"article_id":article_id,"updated_at":bound_at})
    );
    let conflict = json!({"error":"web_page_bound","login":""});
    assert_eq!(
        call(
            app,
            "POST",
            "/api/web-pages/annotations",
            cookie,
            mark("再标", "blue")
        )
        .await,
        (StatusCode::CONFLICT, conflict.clone())
    );
    assert_eq!(
        call(
            app,
            "PUT",
            &format!("/api/web-annotations/{id}"),
            cookie,
            json!({"title":"","style":"blue"})
        )
        .await,
        (StatusCode::CONFLICT, conflict)
    );

    // The article page migrates what it found; a bad anchor refuses the whole request.
    assert_eq!(
        call(app, "GET", &on_article, cookie, Value::Null).await,
        (
            StatusCode::OK,
            json!({"items":[edited.clone(),second.clone()]})
        )
    );
    let migrate = format!("{on_article}/migrate");
    let (status, error) = call(
        app,
        "POST",
        &migrate,
        cookie,
        json!({"items":[{"web_annotation_id":id,"start":0,"end":3,"quote":"值得"}]}),
    )
    .await;
    assert_eq!(
        (status, error["path"].clone()),
        (StatusCode::BAD_REQUEST, json!("quote"))
    );
    let (status, migrated) = call(
        app,
        "POST",
        &migrate,
        cookie,
        json!({"items":[{"web_annotation_id":id,"start":9,"end":11,"quote":"值得"}]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let new_id = migrated["items"][0]["id"].as_str().unwrap().to_owned();
    let migrated_at = migrated["items"][0]["updated_at"].as_i64().unwrap();
    let annotation = json!({"id":new_id,"message_id":null,"start":9,"end":11,"quote":"值得","style":"green","note":"想法","created_at":created_at,"updated_at":migrated_at});
    assert_eq!(migrated, json!({"items":[annotation.clone()]}));
    assert_eq!(
        call(
            app,
            "GET",
            &format!("/api/articles/{article_id}/annotations"),
            cookie,
            Value::Null
        )
        .await,
        (StatusCode::OK, json!({"items":[annotation]}))
    );
    // Migrating it again finds nothing to move.
    assert_eq!(
        call(
            app,
            "POST",
            &migrate,
            cookie,
            json!({"items":[{"web_annotation_id":id,"start":9,"end":11,"quote":"值得"}]}),
        )
        .await,
        (StatusCode::OK, json!({"items":[]}))
    );

    // The unmatched one is discarded from the article page; the bound page stays.
    let second_id = second["id"].as_str().unwrap();
    assert_eq!(
        call(
            app,
            "DELETE",
            &format!("/api/web-annotations/{second_id}"),
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
    let (status, detail) = call(app, "GET", &lookup, cookie, Value::Null).await;
    assert_eq!(
        (
            status,
            &detail["page"]["article_id"],
            &detail["annotations"]
        ),
        (StatusCode::OK, &json!(article_id), &json!([]))
    );
}

/// Percent-encodes a query value.
fn urlencode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
