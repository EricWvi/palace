use super::common::Cursor;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Independent opaque records; this protocol cannot modify imported conversation trees.
#[derive(Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    pub id: Uuid,
    /// Client Unix epoch milliseconds; full i64 range, not guaranteed lossless in JS number.
    pub updated_at: i64,
    pub is_deleted: bool,
    /// Required but accepts any JSON value, including null; no domain object shape is imposed.
    #[schema(value_type = Value, required = true)]
    pub body: serde_json::Value,
}
impl From<Record> for palace_domain::Record {
    /// Keeps storage and LWW decisions in the domain/database boundary.
    fn from(value: Record) -> Self {
        Self {
            id: value.id,
            updated_at: value.updated_at,
            is_deleted: value.is_deleted,
            body: value.body,
        }
    }
}
impl From<palace_domain::Record> for Record {
    /// Preserves the opaque body and client timestamp without normalization.
    fn from(value: palace_domain::Record) -> Self {
        Self {
            id: value.id,
            updated_at: value.updated_at,
            is_deleted: value.is_deleted,
            body: value.body,
        }
    }
}
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublishedRecord {
    pub owner_id: Uuid,
    pub server_version: Cursor,
    pub record: Record,
}
impl From<palace_domain::PublishedRecord> for PublishedRecord {
    /// Exposes business publication metadata, never authentication state.
    fn from(value: palace_domain::PublishedRecord) -> Self {
        Self {
            owner_id: value.owner_id,
            server_version: Cursor(value.server_version),
            record: value.record.into(),
        }
    }
}
#[derive(Serialize, ToSchema)]
#[serde(tag = "status", content = "record", rename_all = "snake_case")]
pub(crate) enum UploadResult {
    Accepted(PublishedRecord),
    Retained(PublishedRecord),
}
impl From<palace_domain::UploadResult> for UploadResult {
    /// Makes both accepted writes and retained conflicts explicit in the wire union.
    fn from(value: palace_domain::UploadResult) -> Self {
        match value {
            palace_domain::UploadResult::Accepted(record) => Self::Accepted(record.into()),
            palace_domain::UploadResult::Retained(record) => Self::Retained(record.into()),
        }
    }
}
#[derive(Serialize, ToSchema)]
pub(crate) struct SyncPage {
    pub records: Vec<PublishedRecord>,
    pub cursor: Cursor,
}
impl From<palace_domain::SyncPage> for SyncPage {
    /// Preserves the committed page cursor instead of inferring a high-water mark.
    fn from(value: palace_domain::SyncPage) -> Self {
        Self {
            records: value.records.into_iter().map(Into::into).collect(),
            cursor: Cursor(value.cursor),
        }
    }
}
