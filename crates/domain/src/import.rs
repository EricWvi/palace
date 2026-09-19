use crate::{Role, SessionId, Source};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputErrorKind {
    Syntax,
    Field,
    Limit,
}
#[derive(Clone, Debug, thiserror::Error, Serialize, PartialEq, Eq)]
#[error("{kind:?} at {path}: {message}")]
pub struct InputError {
    pub kind: InputErrorKind,
    pub path: String,
    pub message: String,
}
impl InputError {
    pub fn new(kind: InputErrorKind, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind,
            path: path.into(),
            message: message.into(),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MessageInput {
    pub role: Role,
    pub content: String,
}
#[derive(Clone, Copy, Debug)]
pub struct ImportLimits {
    pub bytes: usize,
    pub messages: usize,
    pub content_bytes: usize,
    pub depth: usize,
}
impl Default for ImportLimits {
    /// Bounds parsing work and transaction size for the first server deployment.
    fn default() -> Self {
        Self {
            bytes: 8 * 1024 * 1024,
            messages: 10_000,
            content_bytes: 1024 * 1024,
            depth: 32,
        }
    }
}
/// Both browser entry points pass their original history bytes through this same boundary.
#[derive(Clone, Debug)]
pub struct ImportInput {
    pub title: String,
    pub source: Source,
    pub session_id: String,
    pub history: Vec<u8>,
    pub idempotency_key: String,
    pub occurred_at: i64,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum ImportTarget {
    Conversation {
        title: String,
        source: Source,
        session_id: SessionId,
    },
    Branch {
        conversation_id: Uuid,
        session_id: SessionId,
    },
    Update {
        conversation_id: Uuid,
        path_id: Uuid,
    },
}
/// Carries the common upload independently of conversation metadata and path identity.
pub struct PathInput {
    pub history: Vec<u8>,
    pub idempotency_key: String,
    pub occurred_at: i64,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ImportRequest {
    target: ImportTarget,
    messages: Vec<MessageInput>,
    idempotency_key: String,
    digest: Vec<u8>,
    occurred_at: i64,
}
impl ImportRequest {
    /// Validates all input before persistence and ignores noncontract message fields.
    pub fn parse(input: ImportInput, limits: ImportLimits) -> Result<Self, InputError> {
        Self::parse_target(
            ImportTarget::Conversation {
                title: input.title,
                source: input.source,
                session_id: SessionId::try_from(input.session_id)?,
            },
            PathInput {
                history: input.history,
                idempotency_key: input.idempotency_key,
                occurred_at: input.occurred_at,
            },
            limits,
        )
    }
    /// Shares strict content validation across creation, branching and append-only updates.
    pub fn parse_target(
        target: ImportTarget,
        input: PathInput,
        limits: ImportLimits,
    ) -> Result<Self, InputError> {
        if input.history.len() > limits.bytes {
            return Err(InputError::new(
                InputErrorKind::Limit,
                "history",
                "upload too large",
            ));
        }
        if let ImportTarget::Conversation { title, .. } = &target {
            validate_title(title)?;
        }
        // Bound browser epoch milliseconds to calendar years 0001 through 9999.
        if !(-62_135_596_800_000..=253_402_300_799_999).contains(&input.occurred_at) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "occurred_at",
                "invalid epoch milliseconds",
            ));
        }
        if input.idempotency_key.trim().is_empty() || input.idempotency_key.len() > 128 {
            return Err(InputError::new(
                InputErrorKind::Field,
                "idempotency_key",
                "expected 1..128 bytes",
            ));
        }
        // Scan before deserialization so ignored metadata cannot bypass the nesting limit.
        let mut depth = 0_usize;
        let mut in_string = false;
        let mut escaped = false;
        for &byte in &input.history {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }
            } else {
                match byte {
                    b'"' => in_string = true,
                    b'[' | b'{' => {
                        depth += 1;
                        if depth > limits.depth {
                            return Err(InputError::new(
                                InputErrorKind::Limit,
                                "history",
                                "JSON nesting too deep",
                            ));
                        }
                    }
                    b']' | b'}' => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        let mut deserializer = serde_json::Deserializer::from_slice(&input.history);
        let messages: Vec<MessageInput> = serde_path_to_error::deserialize(&mut deserializer)
            .map_err(|error| {
                let kind = if error.inner().is_data() {
                    InputErrorKind::Field
                } else {
                    InputErrorKind::Syntax
                };
                InputError::new(
                    kind,
                    format!("history{}", error.path()),
                    error.inner().to_string(),
                )
            })?;
        deserializer
            .end()
            .map_err(|e| InputError::new(InputErrorKind::Syntax, "history", e.to_string()))?;
        if messages.is_empty() {
            return Err(InputError::new(
                InputErrorKind::Field,
                "history",
                "expected nonempty array",
            ));
        }
        if messages.len() > limits.messages {
            return Err(InputError::new(
                InputErrorKind::Limit,
                "history",
                "too many messages",
            ));
        }
        for (index, message) in messages.iter().enumerate() {
            if message.content.len() > limits.content_bytes {
                return Err(InputError::new(
                    InputErrorKind::Limit,
                    format!("history[{index}].content"),
                    "content too large",
                ));
            }
        }
        // Length framing prevents ambiguous concatenations; raw input detects key reuse even when extra fields differ.
        let mut hash = Sha256::new();
        let identity = serde_json::to_vec(&target)
            .map_err(|error| InputError::new(InputErrorKind::Field, "target", error.to_string()))?;
        for bytes in [
            identity.as_slice(),
            input.history.as_slice(),
            &input.occurred_at.to_be_bytes(),
        ] {
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        }
        Ok(Self {
            occurred_at: input.occurred_at,
            target,
            messages,
            idempotency_key: input.idempotency_key,
            digest: hash.finalize().to_vec(),
        })
    }
    /// Returns the user-selected conversation occurrence time as epoch milliseconds.
    pub fn occurred_at(&self) -> i64 {
        self.occurred_at
    }
    /// Prevents branch imports from redefining conversation metadata or source identity.
    pub fn target(&self) -> &ImportTarget {
        &self.target
    }
    /// Exposes only validated role/content pairs.
    pub fn messages(&self) -> &[MessageInput] {
        &self.messages
    }
    /// Scopes retries independently from content deduplication.
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }
    /// Identifies the complete submitted request without retaining a second body.
    pub fn digest(&self) -> &[u8] {
        &self.digest
    }
}
/// Applies the same metadata boundary to initial imports and later title edits without changing identity.
pub fn validate_title(title: &str) -> Result<(), InputError> {
    if title.trim().is_empty() || title.len() > 1024 {
        return Err(InputError::new(
            InputErrorKind::Field,
            "title",
            "expected nonempty title of at most 1024 bytes",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    /// Title edits preserve original display text but never permit unusable or unbounded metadata.
    #[test]
    fn title_edits_share_import_validation() {
        for title in ["", " ", "\n\t"] {
            assert!(validate_title(title).is_err());
        }
        assert!(validate_title(&"a".repeat(1025)).is_err());
        assert!(validate_title("  新标题  ").is_ok());
        assert!(validate_title(&"a".repeat(1024)).is_ok());
    }
    /// Builds identical byte inputs for either browser acquisition method.
    fn input(history: &[u8]) -> ImportInput {
        ImportInput {
            occurred_at: 1_700_000_000_000,
            title: "Title".into(),
            source: Source::Chatgpt,
            session_id: "s".into(),
            history: history.to_vec(),
            idempotency_key: "k".into(),
        }
    }
    /// A retry cannot silently change the chosen time, and out-of-calendar values fail early.
    #[test]
    fn occurrence_time_is_validated_and_part_of_idempotency() {
        let mut first = input(br#"[{"role":"user","content":"hello"}]"#);
        let original = ImportRequest::parse(first.clone(), ImportLimits::default()).unwrap();
        first.occurred_at += 1;
        let changed = ImportRequest::parse(first.clone(), ImportLimits::default()).unwrap();
        assert_ne!(original.digest(), changed.digest());
        assert_eq!(changed.occurred_at(), first.occurred_at);
        first.occurred_at = i64::MAX;
        assert_eq!(
            ImportRequest::parse(first, ImportLimits::default())
                .unwrap_err()
                .path,
            "occurred_at"
        );
    }
    /// Retains exact Markdown, empty messages, extra-field tolerance and consecutive user messages.
    #[test]
    fn preserves_linear_input_exactly() {
        let bytes =
            br#"[{"role":"user","content":" A\r\n","turnId":123},{"role":"user","content":""}]"#;
        let request = ImportRequest::parse(input(bytes), ImportLimits::default()).unwrap();
        assert_eq!(
            request.messages(),
            &[
                MessageInput {
                    role: Role::User,
                    content: " A\r\n".into()
                },
                MessageInput {
                    role: Role::User,
                    content: "".into()
                }
            ]
        );
        assert_eq!(
            request,
            ImportRequest::parse(input(bytes), ImportLimits::default()).unwrap()
        );
        assert_eq!(
            serde_json::to_value(request.messages()).unwrap(),
            serde_json::json!([{"role":"user","content":" A\r\n"},{"role":"user","content":""}])
        );
    }
    /// Accepts every checked-in exporter sample through the same minimal format without retaining extras.
    #[test]
    fn accepts_all_source_export_samples() {
        for bytes in [
            include_bytes!("../../../automa/chatgpt/conversation.json").as_slice(),
            include_bytes!("../../../automa/chatgpt/conversation-2.json").as_slice(),
            include_bytes!("../../../automa/gemini/conversation.json").as_slice(),
            include_bytes!("../../../automa/grok/conversation.json").as_slice(),
        ] {
            let raw: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            let expected: Vec<MessageInput> = raw
                .as_array()
                .unwrap()
                .iter()
                .map(|item| MessageInput {
                    role: serde_json::from_value(item["role"].clone()).unwrap(),
                    content: item["content"].as_str().unwrap().to_owned(),
                })
                .collect();
            assert_eq!(
                ImportRequest::parse(input(bytes), ImportLimits::default())
                    .unwrap()
                    .messages(),
                expected
            );
        }
    }
    /// Distinguishes malformed JSON, invalid fields and every configurable capacity bound.
    #[test]
    fn reports_position_and_limits_before_writes() {
        for (bytes, kind) in [
            (b"[".as_slice(), InputErrorKind::Syntax),
            (b"[]", InputErrorKind::Field),
            (br#"[{"role":"tool","content":"x"}]"#, InputErrorKind::Field),
            (br#"[{"role":"user","content":1}]"#, InputErrorKind::Field),
        ] {
            assert_eq!(
                ImportRequest::parse(input(bytes), ImportLimits::default())
                    .unwrap_err()
                    .kind,
                kind
            );
        }
        let error = ImportRequest::parse(
            input(br#"[{"role":"user","content":1}]"#),
            ImportLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.path, "history[0].content");
        let bytes = br#"[{"role":"user","content":"x","extra":[[]]}]"#;
        for limits in [
            ImportLimits {
                bytes: 1,
                ..Default::default()
            },
            ImportLimits {
                messages: 0,
                ..Default::default()
            },
            ImportLimits {
                content_bytes: 0,
                ..Default::default()
            },
            ImportLimits {
                depth: 3,
                ..Default::default()
            },
        ] {
            assert_eq!(
                ImportRequest::parse(input(bytes), limits).unwrap_err().kind,
                InputErrorKind::Limit
            );
        }
    }
}
