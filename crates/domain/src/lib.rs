//! Validated server values independent of transport and persistence.
mod conversation;
mod excerpt;
mod import;
mod media;

pub use conversation::{Conversation, Message, Role, SessionId, Source, SourceLinks, read_path};
pub use excerpt::{EXCERPT_CHARS, EXCERPT_SOURCE_CHARS, TOC_LINE_CHARS, excerpt, toc_line};
pub use import::{
    ImportInput, ImportLimits, ImportRequest, ImportTarget, InputError, InputErrorKind,
    MessageInput, PathInput, UpdateHistory, validate_title,
};
pub use media::{FALLBACK_CONTENT_TYPE, MediaLimits, MediaMetadata, MediaUpload};
mod sync;
pub use sync::{PublishedRecord, Record, ServerVersion, SyncPage, UploadResult, accepts_record};
