use super::api_contract;
use crate::http::{dto, error::ApiError, routes};
use axum::{http::StatusCode, response::IntoResponse};
use http_body_util::BodyExt;
use palace_domain::{InputError, InputErrorKind, ServerVersion};
use pretty_assertions::assert_eq;
use serde_json::json;
use uuid::Uuid;

/// Checks route coverage and the nullable/string semantics that default derives can misrepresent.
#[test]
fn contract_covers_operations_and_special_wire_types() {
    let document = serde_json::to_value(api_contract()).unwrap();
    let mut operations = document["paths"]
        .as_object()
        .unwrap()
        .iter()
        .flat_map(|(path, item)| {
            item.as_object()
                .unwrap()
                .keys()
                .map(move |method| format!("{method} {path}"))
        })
        .collect::<Vec<_>>();
    operations.sort();
    macro_rules! inventory {
        ($( $method:ident $path:ident => $module:ident::$handler:ident, )*) => {
            vec![$(format!("{} {}", stringify!($method), routes::$path)),*]
        };
    }
    let mut expected = routes::auth_routes!(inventory);
    expected.extend(routes::business_routes!(inventory));
    expected.sort();
    assert_eq!(operations, expected);
    let schemas = &document["components"]["schemas"];
    assert_eq!(schemas["Cursor"]["type"], "string");
    assert_eq!(schemas["Record"]["properties"]["body"].get("type"), None);
    assert!(
        schemas["Record"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("body"))
    );
    assert!(
        schemas["Message"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("parent_message_id"))
    );
    assert_eq!(
        schemas["Message"]["properties"]["parent_message_id"]["type"],
        json!(["string", "null"])
    );
    assert_eq!(
        schemas["FileImport"]["properties"]["history"]["format"],
        "binary"
    );
    assert_eq!(schemas["TextImport"]["additionalProperties"], false);
    assert_eq!(document, serde_json::to_value(api_contract()).unwrap());
}

/// Full-object comparisons protect existing fields while removing direct DB/domain serialization.
/// Core test case:
/// - `specs/test-cases/server/conversation/reading-page.md#the-message-toc-must-list-every-message-of-the-current-path-by-its-opening`
#[test]
fn dto_conversions_preserve_existing_payloads() {
    let id = Uuid::nil();
    let owner = palace_db::Owner {
        id,
        email: "owner@example.com".into(),
        identity_id: id,
    };
    assert_eq!(
        serde_json::to_value(dto::Owner::from(owner.clone())).unwrap(),
        serde_json::to_value(owner).unwrap()
    );
    let conversation = palace_domain::Conversation {
        id,
        owner_id: id,
        source: palace_domain::Source::Gemini,
    };
    assert_eq!(
        serde_json::to_value(dto::Conversation::from(conversation.clone())).unwrap(),
        serde_json::to_value(conversation).unwrap()
    );
    let message = palace_domain::Message {
        id,
        owner_id: id,
        conversation_id: id,
        parent_message_id: None,
        role: palace_domain::Role::Assistant,
        content: "  内容\r\n".into(),
        created_order: 1,
    };
    // The wire adds the table-of-contents line beside the persisted fields.
    let mut expected = serde_json::to_value(message.clone()).unwrap();
    expected["toc_line"] = json!("内容");
    assert_eq!(
        serde_json::to_value(dto::Message::from(message)).unwrap(),
        expected
    );
    // The wire flattens the persisted kind into a `kind` tag beside the card fields.
    let moment = palace_db::Moment {
        id,
        occurred_at: 123,
        detail: palace_db::MomentDetail::Conversation(palace_db::ConversationCard {
            conversation_id: id,
            title: "title".into(),
            source: palace_domain::Source::Grok,
            message_count: 2,
            excerpt: vec![palace_db::ExcerptLine {
                role: palace_domain::Role::User,
                text: "你好".into(),
            }],
        }),
    };
    assert_eq!(
        serde_json::to_value(dto::Moment::from(moment)).unwrap(),
        json!({"kind":"conversation","id":id,"occurred_at":123,"conversation_id":id,"title":"title","source":"grok","message_count":2,"excerpt":[{"role":"user","text":"你好"}]})
    );
    let imported = palace_db::ImportResult {
        import_id: id,
        conversation_id: id,
        path_id: id,
        head_message_id: id,
        created: 2,
        reused: 1,
    };
    assert_eq!(
        serde_json::to_value(dto::ImportResult::from(imported.clone())).unwrap(),
        serde_json::to_value(imported).unwrap()
    );
    for body in [
        json!(null),
        json!([]),
        json!({"free":true}),
        json!(42),
        json!("text"),
        json!(false),
    ] {
        let record = palace_domain::Record {
            id,
            updated_at: i64::MAX,
            is_deleted: true,
            body,
        };
        let wire = dto::Record::from(record.clone());
        assert_eq!(
            serde_json::to_value(&wire).unwrap(),
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(palace_domain::Record::from(wire), record);
        let published = palace_domain::PublishedRecord {
            owner_id: id,
            server_version: ServerVersion::new(i64::MAX).unwrap(),
            record,
        };
        for result in [
            palace_domain::UploadResult::Accepted(published.clone()),
            palace_domain::UploadResult::Retained(published.clone()),
        ] {
            assert_eq!(
                serde_json::to_value(dto::UploadResult::from(result.clone())).unwrap(),
                serde_json::to_value(result).unwrap()
            );
        }
        let page = palace_domain::SyncPage {
            records: vec![published],
            cursor: ServerVersion::new(i64::MAX).unwrap(),
        };
        assert_eq!(
            serde_json::to_value(dto::SyncPage::from(page.clone())).unwrap(),
            serde_json::to_value(page).unwrap()
        );
    }
}

/// Verifies strict envelopes and lossless cursors continue to reject invalid input at runtime.
#[test]
fn request_dtos_preserve_parser_boundaries() {
    let valid = json!({"title":"title", "source":"chatgpt", "occurred_at":0, "session_id":"s1", "history":"[]", "idempotency_key":"key"});
    assert!(serde_json::from_value::<dto::TextImport>(valid.clone()).is_ok());
    for invalid in [
        json!(null),
        json!({}),
        {
            let mut value = valid.clone();
            value["owner_id"] = json!(Uuid::nil());
            value
        },
        {
            let mut value = valid;
            value["source"] = json!("unknown");
            value
        },
    ] {
        assert!(serde_json::from_value::<dto::TextImport>(invalid).is_err());
    }
    for invalid in [
        json!(42),
        json!("01"),
        json!("9223372036854775808"),
        json!(null),
    ] {
        assert!(serde_json::from_value::<dto::Cursor>(invalid).is_err());
    }
    let cursor: dto::Cursor = serde_json::from_value(json!("9007199254740993")).unwrap();
    assert_eq!(
        serde_json::to_value(cursor).unwrap(),
        json!("9007199254740993")
    );
    let record = json!({"id":Uuid::nil(), "updatedAt":0, "isDeleted":false, "body":null});
    assert!(serde_json::from_value::<dto::Record>(record.clone()).is_ok());
    let mut missing = record;
    missing.as_object_mut().unwrap().remove("body");
    assert!(serde_json::from_value::<dto::Record>(missing).is_err());
}

/// Exercises the real error responder so typed payloads cannot silently change status or JSON shape.
#[tokio::test]
async fn error_dtos_preserve_status_and_body() {
    for (error, status, expected) in [
        (
            ApiError::Unauthorized,
            StatusCode::UNAUTHORIZED,
            json!({"error":"authentication_required", "login":"/auth/login"}),
        ),
        (
            ApiError::Forbidden,
            StatusCode::FORBIDDEN,
            json!({"error":"origin_rejected", "login":""}),
        ),
        (
            ApiError::Conflict,
            StatusCode::CONFLICT,
            json!({"error":"identity_or_request_conflict", "login":""}),
        ),
        (
            ApiError::DuplicateSession,
            StatusCode::CONFLICT,
            json!({"error":"session_already_exists", "login":""}),
        ),
        (
            ApiError::NotFound,
            StatusCode::NOT_FOUND,
            json!({"error":"not_found", "login":""}),
        ),
        (
            ApiError::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"identity_unavailable", "login":""}),
        ),
        (
            ApiError::Internal,
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error":"persistence_failed", "login":""}),
        ),
        (
            ApiError::Input(InputError::new(InputErrorKind::Field, "title", "invalid")),
            StatusCode::BAD_REQUEST,
            json!({"kind":"field", "path":"title", "message":"invalid"}),
        ),
        (
            ApiError::Input(InputError::new(
                InputErrorKind::Limit,
                "history",
                "too large",
            )),
            StatusCode::PAYLOAD_TOO_LARGE,
            json!({"kind":"limit", "path":"history", "message":"too large"}),
        ),
        (
            ApiError::Input(InputError::new(
                InputErrorKind::Syntax,
                "history",
                "invalid",
            )),
            StatusCode::BAD_REQUEST,
            json!({"kind":"syntax", "path":"history", "message":"invalid"}),
        ),
    ] {
        let response = error.into_response();
        let actual_status = response.status();
        let content_type = response.headers()["content-type"]
            .to_str()
            .unwrap()
            .to_owned();
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(
            (actual_status, content_type, body),
            (status, "application/json".to_owned(), expected)
        );
    }
}
