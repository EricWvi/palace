use crate::{InputError, InputErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A lossless wire cursor independent of business timestamps and JavaScript number precision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServerVersion(i64);
impl ServerVersion {
    /// Rejects negative and overflowing cursors before PostgreSQL comparison.
    pub fn new(value: i64) -> Result<Self, InputError> {
        if value < 0 {
            Err(InputError::new(
                InputErrorKind::Field,
                "serverVersion",
                "expected nonnegative bigint",
            ))
        } else {
            Ok(Self(value))
        }
    }
    /// Provides the exact bigint for database comparisons.
    pub fn value(self) -> i64 {
        self.0
    }
}
impl TryFrom<String> for ServerVersion {
    type Error = InputError;
    /// Requires canonical decimal strings, excluding ambiguous signs, padding and floating-point forms.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(InputError::new(
                InputErrorKind::Field,
                "serverVersion",
                "expected canonical decimal string",
            ));
        }
        Self::new(value.parse().map_err(|_| {
            InputError::new(InputErrorKind::Field, "serverVersion", "bigint overflow")
        })?)
    }
}
impl From<ServerVersion> for String {
    /// Serializes even values above 2^53 without lossy floating-point conversion.
    fn from(value: ServerVersion) -> Self {
        value.0.to_string()
    }
}
/// An independently editable record; multi-record conversation structure is outside this protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub id: Uuid,
    pub updated_at: i64,
    pub is_deleted: bool,
    pub body: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublishedRecord {
    pub owner_id: Uuid,
    pub server_version: ServerVersion,
    pub record: Record,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "record", rename_all = "snake_case")]
pub enum UploadResult {
    Accepted(PublishedRecord),
    Retained(PublishedRecord),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncPage {
    pub records: Vec<PublishedRecord>,
    pub cursor: ServerVersion,
}
/// Leaves equal timestamps untouched even when content differs; sequence never breaks business ties.
pub fn accepts_record(existing: Option<&Record>, incoming: &Record) -> bool {
    existing.is_none_or(|existing| incoming.updated_at > existing.updated_at)
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    /// Proves strict whole-record arbitration for normal records and uploaded tombstones alike.
    #[test]
    fn lww_only_accepts_strictly_newer_business_timestamps() {
        let existing = Record {
            id: Uuid::now_v7(),
            updated_at: 1000,
            is_deleted: false,
            body: serde_json::json!({"title":"existing","content":"old"}),
        };
        for is_deleted in [false, true] {
            for (timestamp, accepted) in [(999, false), (1000, false), (1001, true)] {
                let incoming = Record {
                    updated_at: timestamp,
                    is_deleted,
                    body: serde_json::json!({"title":"incoming","content":"new"}),
                    ..existing.clone()
                };
                assert_eq!(accepts_record(Some(&existing), &incoming), accepted);
                assert!(accepts_record(/*existing*/ None, &incoming));
            }
        }
    }
    /// Guards the bigint boundary and rejects numeric JSON versions that a browser could round.
    #[test]
    fn server_version_is_lossless_canonical_decimal() {
        let version = ServerVersion::new(i64::MAX).unwrap();
        let json = serde_json::to_string(&version).unwrap();
        assert_eq!(json, "\"9223372036854775807\"");
        assert_eq!(
            serde_json::from_str::<ServerVersion>(&json).unwrap(),
            version
        );
        for input in [
            "\"-1\"",
            "\"01\"",
            "\"+1\"",
            "\"9223372036854775808\"",
            "1",
            "\"1.0\"",
        ] {
            assert!(serde_json::from_str::<ServerVersion>(input).is_err());
        }
    }
}
