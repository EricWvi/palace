use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// A stored media file. Its id is the only reference to keep; storage locations never leave the server.
#[derive(Serialize, ToSchema)]
pub(crate) struct Media {
    /// Server-generated UUID v7; stable for the life of the file.
    pub id: Uuid,
    /// Lowercase `type/subtype` declared by the uploading part, or application/octet-stream.
    pub content_type: String,
    pub byte_size: i64,
    /// Name the client gave the file, for display only; null when none was given.
    #[schema(required = true)]
    pub file_name: Option<String>,
    /// Unix epoch milliseconds.
    pub created_at: i64,
}
impl From<palace_db::Media> for Media {
    /// Lists every wire field so a new persisted field cannot leak by accident.
    fn from(value: palace_db::Media) -> Self {
        Self {
            id: value.id,
            content_type: value.content_type,
            byte_size: value.byte_size,
            file_name: value.file_name,
            created_at: value.created_at,
        }
    }
}

/// Exactly one part named `file`; any other or repeated part is rejected.
#[derive(ToSchema)]
pub(crate) struct MediaUploadForm {
    /// 1 byte to the deployment limit (default 50 MiB). The part's Content-Type and filename are recorded.
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}
