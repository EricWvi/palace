use crate::Note;
use palace_domain::{InputError, InputErrorKind};
use std::{fmt, str::FromStr};

/// Rows per page, the same as the conversation list.
pub const NOTE_PAGE_SIZE: usize = 50;

/// A search keyword with surrounding whitespace removed; an all-blank input is no search at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteSearch(String);

impl NoteSearch {
    /// Returns `None` for blank input, so callers cannot run an empty search by accident.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        (!value.is_empty()).then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The position after the last row of a page. The list order is `(occurred_at, id)` descending,
/// so the next page starts strictly below it, and a cache replaced between two pages cannot make
/// rows repeat or vanish the way an offset would.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteCursor {
    pub occurred_at: i64,
    pub id: String,
}

impl NoteCursor {
    /// Whether a note comes after this position in list order.
    pub(crate) fn precedes(&self, note: &Note) -> bool {
        (note.occurred_at, note.id.as_str()) < (self.occurred_at, self.id.as_str())
    }
}

impl fmt::Display for NoteCursor {
    /// Writes the opaque token clients send back unchanged; the id may contain `_` itself, the
    /// time never does.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_{}", self.occurred_at, self.id)
    }
}

impl FromStr for NoteCursor {
    type Err = InputError;

    /// Accepts only tokens this server could have written.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || InputError::new(InputErrorKind::Field, "cursor", "expected a list cursor");
        let (occurred_at, id) = value.split_once('_').ok_or_else(invalid)?;
        if id.is_empty() {
            return Err(invalid());
        }
        Ok(Self {
            occurred_at: occurred_at.parse().map_err(|_| invalid())?,
            id: id.to_owned(),
        })
    }
}

/// What to list: everything or a search, from the start or after a cursor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteListRequest {
    pub search: Option<NoteSearch>,
    pub after: Option<NoteCursor>,
}

/// One page of the list plus what the page around it needs.
#[derive(Debug, PartialEq, Eq)]
pub struct NotePage {
    pub items: Vec<Note>,
    /// Present only when more rows follow this page.
    pub next_cursor: Option<NoteCursor>,
    /// Every note of the index, whatever the search; the kind column shows it as a count.
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Cursors round-trip ids holding `_` and refuse anything the server could not have written.
    /// Core test case:
    /// - `specs/test-cases/web/stars/notes.md#note-list-must-page-every-indexed-note-exactly-once-by-its-date`
    #[test]
    fn cursors_round_trip_and_refuse_foreign_tokens() {
        let cursor = NoteCursor {
            occurred_at: -5,
            id: "a_b/中_文".into(),
        };
        assert_eq!(cursor.to_string().parse::<NoteCursor>().ok(), Some(cursor));
        for token in ["", "12", "12_", "x_a", "1.5_a"] {
            assert!(token.parse::<NoteCursor>().is_err(), "{token:?}");
        }
        assert_eq!(NoteSearch::parse("  \t"), None);
        assert_eq!(
            NoteSearch::parse(" 锁 ").map(|search| search.as_str().to_owned()),
            Some("锁".into())
        );
    }
}
