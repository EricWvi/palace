use super::common::{Cursor, Source};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

/// Original history JSON is a string here, not an embedded message array.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextImport {
    /// Nonblank after trimming; preserve the original value; at most 1024 UTF-8 bytes.
    pub title: String,
    /// User-selected Unix epoch milliseconds, calendar years 0001 through 9999.
    #[schema(minimum = -62135596800000_i64, maximum = 253402300799999_i64)]
    pub occurred_at: i64,
    pub source: Source,
    /// 1..512 UTF-8 bytes; one source path segment, no whitespace, controls or /\\?#%:.
    pub session_id: String,
    /// Nonempty role/content array encoded as text. Default limits: 8 MiB, 10000 messages,
    /// 1 MiB per content, depth 32. Extra message fields are ignored; content is preserved.
    pub history: String,
    /// Nonblank after trimming; at most 128 UTF-8 bytes, scoped to the owner.
    pub idempotency_key: String,
}

/// A multipart transport view of TextImport; each metadata part is UTF-8 text (at most 1024 bytes).
#[derive(ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileImport {
    /// Nonblank; at most 1024 UTF-8 bytes.
    pub title: String,
    /// Text parsed as i64 epoch milliseconds in -62135596800000..=253402300799999.
    #[schema(value_type = String)]
    pub occurred_at: i64,
    pub source: Source,
    /// 1..512 UTF-8 bytes; a single literal source identity segment.
    pub session_id: String,
    /// Raw history bytes; default limit 8 MiB. File or text part accepted; filename/MIME ignored.
    #[schema(value_type = String, format = Binary)]
    pub history: Vec<u8>,
    /// Nonblank; at most 128 UTF-8 bytes.
    pub idempotency_key: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewPath {
    /// Title of the new path. Nonblank after trimming; at most 1024 UTF-8 bytes.
    pub title: String,
    /// 1..512 UTF-8 bytes; same source identity validation as initial import.
    pub session_id: String,
    /// Original JSON array text, with the same limits as TextImport.history.
    pub history: String,
    /// Unix epoch milliseconds selected by the user.
    #[schema(minimum = -62135596800000_i64, maximum = 253402300799999_i64)]
    pub occurred_at: i64,
    /// Nonblank; at most 128 UTF-8 bytes, scoped to the owner.
    pub idempotency_key: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdatePath {
    /// Replaces the path title. Nonblank after trimming; at most 1024 UTF-8 bytes.
    pub title: String,
    /// Full historical prefix plus appended messages as original JSON text; TextImport limits apply.
    pub history: String,
    /// Unix epoch milliseconds selected by the user.
    #[schema(minimum = -62135596800000_i64, maximum = 253402300799999_i64)]
    pub occurred_at: i64,
    /// Nonblank; at most 128 UTF-8 bytes, scoped to the owner.
    pub idempotency_key: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PathMetadata {
    /// Title of the addressed path. Nonblank after trimming; at most 1024 UTF-8 bytes; no normalization.
    pub title: String,
    /// Source of the whole conversation; it cascades to every path.
    pub source: Source,
}
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TimelineRange {
    /// Inclusive start, Unix epoch milliseconds computed in the caller's time zone.
    pub start: i64,
    /// Exclusive end, Unix epoch milliseconds; after start and at most 48 hours later.
    pub end: i64,
}
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct Pull {
    pub cursor: Cursor,
    #[param(minimum = 1, maximum = 1000)]
    pub limit: u32,
}
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct Callback {
    pub code: String,
    pub state: String,
}
