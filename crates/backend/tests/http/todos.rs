use super::articles::call;
use axum::{Router, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Lists, tasks, the body, done, planning, finishing and restoring through the real Router, with
/// every cross-owner request answered as if nothing were there and bad fields named.
pub(super) async fn exercise_todos(app: &Router, cookie: &str, foreign: &str) {
    let (status, work) = call(
        app,
        "POST",
        "/api/todo-lists",
        cookie,
        json!({"name":" 工作 ","position":"a0"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let list = work["id"].as_str().unwrap().to_owned();
    assert_eq!(work, json!({"id":list,"name":"工作","position":"a0"}));
    // A second list on the same key is refused.
    assert_eq!(
        call(
            app,
            "POST",
            "/api/todo-lists",
            cookie,
            json!({"name":"撞键","position":"a0"}),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );

    let (status, created) = call(
        app,
        "POST",
        "/api/todos",
        cookie,
        json!({"list_id":list,"title":"读书","position":"a0"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(
        created,
        json!({"id":id,"list_id":list,"title":"读书","position":"a0","link":null,"has_body":false,"plan":{"kind":"open"},"done_days":0})
    );
    let (status, inbox) = call(
        app,
        "POST",
        "/api/todos",
        cookie,
        json!({"list_id":null,"title":"先收下","position":"a0"}),
    )
    .await;
    assert_eq!(
        (status, &inbox["list_id"]),
        (StatusCode::CREATED, &Value::Null)
    );
    let inbox = inbox["id"].as_str().unwrap().to_owned();

    // Plan it on a day through an edit, with a link; then done counts the day.
    let edit = json!({"list_id":list,"title":"读书","link":"https://example.com/book","position":"a0","plan":{"kind":"day","day":"2026-10-23"}});
    let (status, planned) = call(app, "PUT", &format!("/api/todos/{id}"), cookie, edit).await;
    assert_eq!(
        (status, &planned["plan"], &planned["link"]),
        (
            StatusCode::OK,
            &json!({"kind":"day","day":"2026-10-23","done":false}),
            &json!("https://example.com/book")
        )
    );
    let done = format!("/api/todos/{id}/done");
    let (status, ticked) = call(
        app,
        "PUT",
        &done,
        cookie,
        json!({"day":"2026-10-23","done":true}),
    )
    .await;
    assert_eq!(
        (status, &ticked["plan"]["done"], &ticked["done_days"]),
        (StatusCode::OK, &json!(true), &json!(1))
    );
    assert_eq!(
        call(
            app,
            "PUT",
            &done,
            cookie,
            json!({"day":"2026-10-22","done":true})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );

    let body = format!("/api/todos/{id}/body");
    assert_eq!(
        call(app, "PUT", &body, cookie, json!({"body":"第一章"}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(app, "GET", &body, cookie, Value::Null).await,
        (StatusCode::OK, json!({"body":"第一章"}))
    );
    let (_, board) = call(app, "GET", "/api/todos", cookie, Value::Null).await;
    assert_eq!(board["lists"], json!([work]));
    assert_eq!(
        board["todos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|todo| (todo["title"].clone(), todo["has_body"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("读书"), json!(true)),
            (json!("先收下"), json!(false))
        ]
    );

    assert_eq!(
        call(
            app,
            "POST",
            "/api/todos/plan",
            cookie,
            json!({"day":"2026-10-24","ids":[inbox]})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            app,
            "POST",
            &format!("/api/todos/{inbox}/complete"),
            cookie,
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            app,
            "POST",
            &format!("/api/todos/{id}/complete"),
            cookie,
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let completed = format!("/api/todo-lists/{list}/completed");
    let (status, finished) = call(app, "GET", &completed, cookie, Value::Null).await;
    let at = finished["items"][0]["completed_at"].as_i64().unwrap();
    assert_eq!(
        (status, finished),
        (
            StatusCode::OK,
            json!({"items":[{"id":id,"title":"读书","link":"https://example.com/book","has_body":true,"done_days":1,"completed_at":at}]})
        )
    );
    let (status, restored) = call(
        app,
        "POST",
        &format!("/api/todos/{id}/restore"),
        cookie,
        json!({"position":"Zz"}),
    )
    .await;
    assert_eq!(
        (status, &restored["plan"], &restored["position"]),
        (StatusCode::OK, &json!({"kind":"open"}), &json!("Zz"))
    );

    // Invalid parts are named; nothing is written.
    for (method, uri, body, path) in [
        (
            "POST",
            "/api/todo-lists".to_owned(),
            json!({"name":"  ","position":"a1"}),
            "name",
        ),
        (
            "PUT",
            format!("/api/todo-lists/{list}"),
            json!({"name":"工作","position":"a10"}),
            "position",
        ),
        (
            "POST",
            "/api/todos".to_owned(),
            json!({"list_id":list,"title":"x","position":"a00"}),
            "position",
        ),
        (
            "POST",
            "/api/todos".to_owned(),
            json!({"list_id":list,"title":" ","position":"a5"}),
            "title",
        ),
        (
            "PUT",
            format!("/api/todos/{inbox}"),
            json!({"list_id":null,"title":"x","link":null,"position":"a0","plan":{"kind":"shelved"}}),
            "plan",
        ),
        (
            "PUT",
            format!("/api/todos/{id}"),
            json!({"list_id":list,"title":"x","link":"ftp://x","position":"Zz","plan":{"kind":"open"}}),
            "link",
        ),
        (
            "PUT",
            format!("/api/todos/{id}"),
            json!({"list_id":list,"title":"x","link":null,"position":"Zz","plan":{"kind":"day","day":"10/23"}}),
            "plan.day",
        ),
        ("PUT", body.clone(), json!({"body":" \n"}), "body"),
        (
            "PUT",
            done.clone(),
            json!({"day":"2026-13-01","done":true}),
            "day",
        ),
    ] {
        let (status, error) = call(app, method, &uri, cookie, body).await;
        assert_eq!(
            (status, &error["path"]),
            (StatusCode::BAD_REQUEST, &json!(path)),
            "{uri}"
        );
    }
    let ids: Vec<String> = (0..1001).map(|_| id.clone()).collect();
    assert_eq!(
        call(
            app,
            "POST",
            "/api/todos/plan",
            cookie,
            json!({"day":"2026-10-23","ids":ids})
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );

    // Another owner meets nothing at all.
    let missing = [
        ("GET", body.clone(), Value::Null),
        (
            "PUT",
            format!("/api/todos/{id}"),
            json!({"list_id":list,"title":"x","link":null,"position":"a1","plan":{"kind":"open"}}),
        ),
        (
            "PUT",
            done.clone(),
            json!({"day":"2026-10-23","done":false}),
        ),
        ("POST", format!("/api/todos/{id}/complete"), Value::Null),
        ("DELETE", format!("/api/todos/{id}"), Value::Null),
        (
            "POST",
            "/api/todos".to_owned(),
            json!({"list_id":list,"title":"x","position":"a1"}),
        ),
        (
            "PUT",
            format!("/api/todo-lists/{list}"),
            json!({"name":"x","position":"a0"}),
        ),
        ("GET", completed.clone(), Value::Null),
        ("DELETE", completed.clone(), Value::Null),
        ("DELETE", format!("/api/todo-lists/{list}"), Value::Null),
    ];
    for (method, uri, body) in missing {
        assert_eq!(
            call(app, method, &uri, foreign, body).await.0,
            StatusCode::NOT_FOUND,
            "{method} {uri}"
        );
    }
    assert_eq!(
        call(app, "GET", "/api/todos", foreign, Value::Null).await,
        (StatusCode::OK, json!({"lists":[],"todos":[]}))
    );

    let (status, renamed) = call(
        app,
        "PUT",
        &format!("/api/todo-lists/{list}"),
        cookie,
        json!({"name":"职业","position":"Zz"}),
    )
    .await;
    assert_eq!(
        (status, renamed),
        (
            StatusCode::OK,
            json!({"id":list,"name":"职业","position":"Zz"})
        )
    );
    assert_eq!(
        call(app, "DELETE", &completed, cookie, Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            app,
            "DELETE",
            &format!("/api/todos/{inbox}"),
            cookie,
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            app,
            "DELETE",
            &format!("/api/todo-lists/{list}"),
            cookie,
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(app, "GET", "/api/todos", cookie, Value::Null).await,
        (StatusCode::OK, json!({"lists":[],"todos":[]}))
    );
}
