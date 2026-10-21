use palace_domain::AnnotationTarget;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// How marked text is drawn: four highlighter grounds and two pen strokes.
#[derive(Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AnnotationStyle {
    Yellow,
    Green,
    Blue,
    Rose,
    Underline,
    Wavy,
}

impl From<AnnotationStyle> for palace_domain::AnnotationStyle {
    fn from(value: AnnotationStyle) -> Self {
        match value {
            AnnotationStyle::Yellow => Self::Yellow,
            AnnotationStyle::Green => Self::Green,
            AnnotationStyle::Blue => Self::Blue,
            AnnotationStyle::Rose => Self::Rose,
            AnnotationStyle::Underline => Self::Underline,
            AnnotationStyle::Wavy => Self::Wavy,
        }
    }
}

impl From<palace_domain::AnnotationStyle> for AnnotationStyle {
    fn from(value: palace_domain::AnnotationStyle) -> Self {
        match value {
            palace_domain::AnnotationStyle::Yellow => Self::Yellow,
            palace_domain::AnnotationStyle::Green => Self::Green,
            palace_domain::AnnotationStyle::Blue => Self::Blue,
            palace_domain::AnnotationStyle::Rose => Self::Rose,
            palace_domain::AnnotationStyle::Underline => Self::Underline,
            palace_domain::AnnotationStyle::Wavy => Self::Wavy,
        }
    }
}

/// A new annotation on an article body.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArticleAnnotationCreate {
    /// UTF-16 code units into the rendered body text, inclusive.
    pub start: i64,
    /// Exclusive; after `start`.
    pub end: i64,
    /// The text in `[start, end)`; nonblank, at most 16 KiB, exactly `end - start` UTF-16 units.
    pub quote: String,
    #[serde(default)]
    pub style: Option<AnnotationStyle>,
    /// Plain text, nonblank, at most 16 KiB. A style, a note or both are required.
    #[serde(default)]
    pub note: Option<String>,
}

/// A new annotation on one message of the conversation.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MessageAnnotationCreate {
    /// A message of this conversation, on any of its paths.
    pub message_id: Uuid,
    /// UTF-16 code units into the rendered message text, inclusive.
    pub start: i64,
    /// Exclusive; after `start`.
    pub end: i64,
    /// The text in `[start, end)`; nonblank, at most 16 KiB, exactly `end - start` UTF-16 units.
    pub quote: String,
    #[serde(default)]
    pub style: Option<AnnotationStyle>,
    /// Plain text, nonblank, at most 16 KiB. A style, a note or both are required.
    #[serde(default)]
    pub note: Option<String>,
}

/// The parts an edit replaces together; the anchor never changes. Clearing both is a 400:
/// delete the annotation instead.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnnotationUpdate {
    #[serde(default)]
    pub style: Option<AnnotationStyle>,
    #[serde(default)]
    pub note: Option<String>,
}

/// An annotation as the reading page draws it.
#[derive(Serialize, ToSchema)]
pub(crate) struct Annotation {
    pub id: Uuid,
    /// The annotated message; null on an article.
    #[schema(required = true)]
    pub message_id: Option<Uuid>,
    pub start: i32,
    pub end: i32,
    pub quote: String,
    #[schema(required = true)]
    pub style: Option<AnnotationStyle>,
    #[schema(required = true)]
    pub note: Option<String>,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
}

/// Every annotation on an article or a conversation, oldest first.
#[derive(Serialize, ToSchema)]
pub(crate) struct AnnotationList {
    pub items: Vec<Annotation>,
}

impl From<palace_db::Annotation> for Annotation {
    fn from(value: palace_db::Annotation) -> Self {
        Self {
            id: value.id,
            message_id: match value.target {
                AnnotationTarget::Article(_) => None,
                AnnotationTarget::Message { message, .. } => Some(message),
            },
            start: value.anchor.start(),
            end: value.anchor.end(),
            quote: value.anchor.quote().to_owned(),
            style: value.content.style().map(AnnotationStyle::from),
            note: value.content.note().map(str::to_owned),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<Vec<palace_db::Annotation>> for AnnotationList {
    fn from(value: Vec<palace_db::Annotation>) -> Self {
        Self {
            items: value.into_iter().map(Annotation::from).collect(),
        }
    }
}
