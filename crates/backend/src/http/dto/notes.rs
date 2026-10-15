use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct NoteListQuery {
    /// Case-insensitive literal substring of a note's title or file; blank means no search.
    pub q: Option<String>,
    /// `next_cursor` from the previous page.
    pub cursor: Option<String>,
}

/// One page of the 摘星 note list.
#[derive(Serialize, ToSchema)]
pub(crate) struct NotePage {
    /// Notes, latest date first.
    pub items: Vec<NoteListItem>,
    /// Opaque token for the next page; null on the last page.
    #[schema(required = true)]
    pub next_cursor: Option<String>,
    /// Number of notes in the library, regardless of `q`.
    pub total: i64,
}

/// One note as the list shows it.
#[derive(Serialize, ToSchema)]
pub(crate) struct NoteListItem {
    /// Path of the note's file in the notes folder, without `.md`, separated by `/`.
    pub id: String,
    pub title: String,
    /// Unix epoch milliseconds of the note's frontmatter `date`.
    pub occurred_at: i64,
}

/// A note opened for reading.
#[derive(Serialize, ToSchema)]
pub(crate) struct NoteDetail {
    pub id: String,
    pub title: String,
    /// Unix epoch milliseconds of the note's frontmatter `date`.
    pub occurred_at: i64,
    /// Markdown body without the frontmatter.
    pub content: String,
}

impl From<palace_notes::Note> for NoteListItem {
    fn from(value: palace_notes::Note) -> Self {
        Self {
            id: value.id,
            title: value.title,
            occurred_at: value.occurred_at,
        }
    }
}

impl From<palace_notes::NotePage> for NotePage {
    /// Encodes the cursor as the opaque token clients send back unchanged.
    fn from(value: palace_notes::NotePage) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            next_cursor: value.next_cursor.map(|cursor| cursor.to_string()),
            // A library never holds anywhere near i64::MAX notes.
            total: i64::try_from(value.total).unwrap_or(i64::MAX),
        }
    }
}

impl From<palace_notes::NoteContent> for NoteDetail {
    fn from(value: palace_notes::NoteContent) -> Self {
        Self {
            id: value.note.id,
            title: value.note.title,
            occurred_at: value.note.occurred_at,
            content: value.content,
        }
    }
}
