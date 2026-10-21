//! Validated server values independent of transport and persistence.
mod access_token;
mod annotation;
mod article;
mod conversation;
mod document_date;
mod excerpt;
mod import;
mod jotting;
mod media;

pub use access_token::{ACCESS_TOKEN_PREFIX, AccessTokenNote, AccessTokenSecret};
pub use annotation::{
    AnnotationAnchor, AnnotationContent, AnnotationNote, AnnotationStyle, AnnotationTarget,
};
pub use article::{ArticleBody, ArticleMetadata, ArticleMetadataInput, MediaName, article_site};
pub use conversation::{Conversation, Message, Role, SessionId, Source, SourceLinks, read_path};
pub use document_date::{DatePrecision, DocumentDate};
pub use excerpt::{EXCERPT_CHARS, EXCERPT_SOURCE_CHARS, TOC_LINE_CHARS, excerpt, toc_line};
pub use import::{
    ImportInput, ImportLimits, ImportRequest, ImportTarget, InputError, InputErrorKind,
    MessageInput, PathInput, UpdateHistory, validate_title,
};
pub use jotting::{JOTTING_BODY_MAX_BYTES, Jotting, JottingCensus, JottingError, JottingProblem};
pub use media::{FALLBACK_CONTENT_TYPE, MediaLimits, MediaMetadata, MediaUpload};
mod sync;
pub use sync::{PublishedRecord, Record, ServerVersion, SyncPage, UploadResult, accepts_record};
