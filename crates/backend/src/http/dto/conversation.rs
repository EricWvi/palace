use super::common::{Role, Source};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
pub(crate) struct Owner {
    pub id: Uuid,
    pub email: String,
    pub identity_id: Uuid,
}
impl From<palace_db::Owner> for Owner {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_db::Owner) -> Self {
        Self {
            id: value.id,
            email: value.email,
            identity_id: value.identity_id,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct Conversation {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub source: Source,
}
impl From<palace_domain::Conversation> for Conversation {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_domain::Conversation) -> Self {
        Self {
            id: value.id,
            owner_id: value.owner_id,
            source: value.source.into(),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct Message {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub conversation_id: Uuid,
    /// Always present; root messages carry null.
    #[schema(required = true)]
    pub parent_message_id: Option<Uuid>,
    pub role: Role,
    pub content: String,
    /// The opening words of `content` as plain text, at most a few dozen characters, for the
    /// reading page's table of contents; derived on each read, never stored.
    pub toc_line: String,
    /// Ordering integer, not a timestamp; i64 values are not generally lossless JS numbers.
    pub created_order: i64,
}
impl From<palace_domain::Message> for Message {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_domain::Message) -> Self {
        Self {
            id: value.id,
            owner_id: value.owner_id,
            conversation_id: value.conversation_id,
            parent_message_id: value.parent_message_id,
            role: value.role.into(),
            toc_line: palace_domain::toc_line(&value.content),
            content: value.content,
            created_order: value.created_order,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ImportResult {
    pub import_id: Uuid,
    pub conversation_id: Uuid,
    pub path_id: Uuid,
    pub head_message_id: Uuid,
    pub created: usize,
    pub reused: usize,
}
impl From<palace_db::ImportResult> for ImportResult {
    /// Copies only the deliberately exposed wire fields from the persisted model.
    fn from(value: palace_db::ImportResult) -> Self {
        Self {
            import_id: value.import_id,
            conversation_id: value.conversation_id,
            path_id: value.path_id,
            head_message_id: value.head_message_id,
            created: value.created,
            reused: value.reused,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationPath {
    pub id: Uuid,
    /// The title of this source session; other paths of the tree have their own.
    pub title: String,
    pub session_id: String,
    pub head_message_id: Uuid,
    /// Unix epoch milliseconds selected by the user.
    pub occurred_at: i64,
    /// Unix epoch milliseconds assigned by the database.
    pub created_at: i64,
    /// Unix epoch milliseconds assigned by the database.
    pub updated_at: i64,
    pub message_count: i64,
    /// A controlled external link, never fetched by the server.
    pub original_link: String,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationDetail {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
    pub paths: Vec<ConversationPath>,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct PathMetadataResponse {
    pub conversation_id: Uuid,
    pub path_id: Uuid,
    pub title: String,
    pub source: Source,
}

/// One timeline entry; `kind` selects the card fields that follow.
#[derive(Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Moment {
    Conversation(ConversationMoment),
    Article(ArticleMoment),
    Sleep(SleepMoment),
    Activity(ActivityMoment),
}
/// An activity moment: time spent on one type of activity, on the day it started.
#[derive(Serialize, ToSchema)]
pub(crate) struct ActivityMoment {
    /// Moment id, equal to the activity's id.
    pub id: Uuid,
    /// Unix epoch milliseconds of the start.
    pub occurred_at: i64,
    pub type_id: Uuid,
    pub type_name: String,
    /// 1 to 720.
    pub minutes: i32,
    #[schema(required = true)]
    pub note: Option<String>,
}
/// A sleep moment: one finished night, on the day it ended.
#[derive(Serialize, ToSchema)]
pub(crate) struct SleepMoment {
    /// Moment id, equal to the night's id.
    pub id: Uuid,
    /// Unix epoch milliseconds of waking.
    pub occurred_at: i64,
    /// Unix epoch milliseconds of the bedtime.
    pub started_at: i64,
}
/// An article moment: the article on the day it was imported.
#[derive(Serialize, ToSchema)]
pub(crate) struct ArticleMoment {
    /// Moment id, equal to the article id.
    pub id: Uuid,
    /// Unix epoch milliseconds of the import.
    pub occurred_at: i64,
    pub title: String,
    #[schema(required = true)]
    pub author: Option<String>,
    /// The URL's host without `www.`, shown where there is no author.
    pub site: String,
}
/// A conversation moment is one path of a conversation tree.
#[derive(Serialize, ToSchema)]
pub(crate) struct ConversationMoment {
    /// Moment id, equal to the path id.
    pub id: Uuid,
    /// Unix epoch milliseconds selected by the user.
    pub occurred_at: i64,
    pub conversation_id: Uuid,
    /// Title of this path.
    pub title: String,
    /// Source of the whole conversation.
    pub source: Source,
    /// Messages from the root to the end of this path.
    pub message_count: i64,
    /// Up to the first two messages of the path as plain text, each at most 120 characters plus an ellipsis.
    pub excerpt: Vec<ExcerptLine>,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct ExcerptLine {
    pub role: Role,
    pub text: String,
}
impl From<palace_db::Moment> for Moment {
    /// Flattens the persisted kind into the wire `kind` tag.
    fn from(value: palace_db::Moment) -> Self {
        match value.detail {
            palace_db::MomentDetail::Conversation(card) => Self::Conversation(ConversationMoment {
                id: value.id,
                occurred_at: value.occurred_at,
                conversation_id: card.conversation_id,
                title: card.title,
                source: card.source.into(),
                message_count: card.message_count,
                excerpt: card
                    .excerpt
                    .into_iter()
                    .map(|line| ExcerptLine {
                        role: line.role.into(),
                        text: line.text,
                    })
                    .collect(),
            }),
            palace_db::MomentDetail::Article(card) => Self::Article(ArticleMoment {
                id: value.id,
                occurred_at: value.occurred_at,
                title: card.title,
                author: card.author,
                site: card.site,
            }),
            palace_db::MomentDetail::Sleep(card) => Self::Sleep(SleepMoment {
                id: value.id,
                occurred_at: value.occurred_at,
                started_at: card.started_at,
            }),
            palace_db::MomentDetail::Activity(card) => Self::Activity(ActivityMoment {
                id: value.id,
                occurred_at: value.occurred_at,
                type_id: card.type_id,
                type_name: card.type_name,
                minutes: card.minutes,
                note: card.note,
            }),
        }
    }
}

/// One entry of a day's outline, in the same order as the timeline returns its cards.
#[derive(Serialize, ToSchema)]
pub(crate) struct MomentOutline {
    /// Moment id, equal to the id of the card it previews.
    pub id: Uuid,
    pub kind: MomentKind,
}
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MomentKind {
    Conversation,
    Article,
    Sleep,
    Activity,
}
impl From<palace_db::MomentOutline> for MomentOutline {
    /// Keeps the public kind vocabulary exhaustive when the database gains a kind.
    fn from(value: palace_db::MomentOutline) -> Self {
        Self {
            id: value.id,
            kind: match value.kind {
                palace_db::MomentKind::Conversation => MomentKind::Conversation,
                palace_db::MomentKind::Article => MomentKind::Article,
                palace_db::MomentKind::Sleep => MomentKind::Sleep,
                palace_db::MomentKind::Activity => MomentKind::Activity,
            },
        }
    }
}
