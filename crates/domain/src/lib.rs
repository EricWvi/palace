//! Validated server values independent of transport and persistence.
mod conversation;
mod import;

pub use conversation::{Conversation, Message, Role, SessionId, Source, SourceLinks, read_path};
pub use import::{
    ImportInput, ImportLimits, ImportRequest, ImportTarget, InputError, InputErrorKind,
    MessageInput, PathInput, validate_title,
};
mod sync;
pub use sync::{PublishedRecord, Record, ServerVersion, SyncPage, UploadResult, accepts_record};
