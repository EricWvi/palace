//! Validated server values independent of transport and persistence.
mod conversation;
mod excerpt;
mod import;

pub use conversation::{Conversation, Message, Role, SessionId, Source, SourceLinks, read_path};
pub use excerpt::{EXCERPT_CHARS, TOC_LINE_CHARS, excerpt, toc_line};
pub use import::{
    ImportInput, ImportLimits, ImportRequest, ImportTarget, InputError, InputErrorKind,
    MessageInput, PathInput, validate_title,
};
mod sync;
pub use sync::{PublishedRecord, Record, ServerVersion, SyncPage, UploadResult, accepts_record};
