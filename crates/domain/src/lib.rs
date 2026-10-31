//! Validated server values independent of transport and persistence.
mod access_token;
mod activity;
mod annotation;
mod article;
mod conversation;
mod document_date;
mod excerpt;
mod frontmatter;
mod habit;
mod import;
mod instant;
mod jotting;
mod local_day;
mod media;
mod sleep;
mod todo;
mod web_page;

pub use access_token::{ACCESS_TOKEN_PREFIX, AccessTokenNote, AccessTokenSecret};
pub use activity::{ActivityInput, ActivityTypeName};
pub use annotation::{
    AnnotationAnchor, AnnotationContent, AnnotationNote, AnnotationStyle, AnnotationTarget,
};
pub use article::{ArticleBody, ArticleMetadata, ArticleMetadataInput, MediaName, article_site};
pub use conversation::{Conversation, Message, Role, SessionId, Source, SourceLinks, read_path};
pub use document_date::{DatePrecision, DocumentDate};
pub use excerpt::{EXCERPT_CHARS, EXCERPT_SOURCE_CHARS, TOC_LINE_CHARS, excerpt, toc_line};
pub use frontmatter::split_frontmatter;
pub use habit::{HabitCount, HabitDays, HabitInput, HabitMark, HabitName, HabitTimes};
pub use import::{
    ImportInput, ImportLimits, ImportRequest, ImportTarget, InputError, InputErrorKind,
    MessageInput, PathInput, UpdateHistory, validate_title,
};
pub use instant::{InstantRange, epoch_instant};
pub use jotting::{JOTTING_BODY_MAX_BYTES, Jotting, JottingCensus, JottingError, JottingProblem};
pub use local_day::LocalDay;
pub use media::{FALLBACK_CONTENT_TYPE, MediaLimits, MediaMetadata, MediaUpload};
pub use sleep::SleepSpan;
pub use todo::{
    TodoBody, TodoLink, TodoListName, TodoPlacement, TodoPlan, TodoPlanInput, TodoPosition,
    TodoTitle,
};
pub use web_page::{WebAnchor, WebPageState, WebPageTitle, WebPageUrl, address_nearness};
mod sync;
pub use sync::{PublishedRecord, Record, ServerVersion, SyncPage, UploadResult, accepts_record};
