//! Phase-one compatibility fixture, not a declaration of production HTTP operations.

use palace_domain::{Record, ServerVersion};
use serde::Serialize;
use serde_json::json;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct CursorEnvelope {
    // The domain newtype deliberately serializes through String, not its i64 storage type.
    #[schema(value_type = String, pattern = "^(0|[1-9][0-9]*)$")]
    cursor: ServerVersion,
    // Serde emits null rather than omitting this response field.
    #[schema(required = true)]
    parent_message_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "status", content = "record", rename_all = "snake_case")]
enum Outcome {
    Accepted(CursorEnvelope),
    Retained(CursorEnvelope),
}

#[derive(Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct FileImport {
    title: String,
    occurred_at: String,
    source: String,
    session_id: String,
    #[schema(format = Binary)]
    history: String,
    idempotency_key: String,
}

/// Describes a fixture operation so code generation exercises paths and multipart media types.
#[utoipa::path(
    post,
    path = "/probe/import",
    request_body(content = FileImport, content_type = "multipart/form-data"),
    responses((status = 200, description = "Probe outcome", body = Outcome))
)]
fn fixture_operation() {}

#[derive(OpenApi)]
#[openapi(paths(fixture_operation))]
struct ProbeApi;

/// Emits schemas and real Serde values without opening a database or contacting an identity provider.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    fixture_operation();
    let missing_body = serde_json::from_value::<Record>(json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "updatedAt": 0,
        "isDeleted": false
    }));
    let null_body: Record = serde_json::from_value(json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "updatedAt": 0,
        "isDeleted": false,
        "body": null
    }))?;
    let accepted = Outcome::Accepted(CursorEnvelope {
        cursor: ServerVersion::new(/*value*/ 9_007_199_254_740_993)?,
        parent_message_id: None,
    });
    let retained = Outcome::Retained(CursorEnvelope {
        cursor: ServerVersion::new(i64::MAX)?,
        parent_message_id: Some(Uuid::nil()),
    });
    let output = json!({
        "openapi": ProbeApi::openapi(),
        "samples": {"accepted": accepted, "retained": retained},
        "missing_body_rejected": missing_body.is_err(),
        "null_body": null_body,
        "invalid_cursors_rejected": (["", "00", "+1", "-1", "1.0", "9223372036854775808"]
            .map(|value| ServerVersion::try_from(value.to_owned()).is_err())),
        "multibyte_title_rejected": palace_domain::validate_title(&"中".repeat(342)).is_err(),
        "multipart": FileImport {
            title: "示例".into(),
            occurred_at: "0".into(),
            source: "chatgpt".into(),
            session_id: "session-1".into(),
            history: r#"[{"role":"user","content":"hello"}]"#.into(),
            idempotency_key: "probe-1".into(),
        },
    });
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
