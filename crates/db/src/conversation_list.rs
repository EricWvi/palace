use crate::{Database, DbError, OwnerScope};
use palace_domain::{InputError, InputErrorKind, Source};
use sqlx::Row;
use std::{fmt, str::FromStr};
use uuid::Uuid;

/// Rows per page; one more is read to learn whether another page follows.
pub const CONVERSATION_PAGE_SIZE: i64 = 50;

/// One row of the 摘星 conversation list: a path, named by its own title and dated by its last
/// import or append, so the row shows exactly what opening it will show.
#[derive(Debug, PartialEq, Eq)]
pub struct ConversationListItem {
    /// Path id, equal to the id of the path's conversation moment.
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub title: String,
    pub source: Source,
    /// Unix epoch milliseconds; paths store `updated_at` truncated to milliseconds.
    pub updated_at: i64,
}

/// One page of the list plus what the page around it needs.
#[derive(Debug, PartialEq, Eq)]
pub struct ConversationPage {
    pub items: Vec<ConversationListItem>,
    /// Present only when more rows follow this page.
    pub next_cursor: Option<PathCursor>,
    /// Every path of the owner, whatever the search; the kind column shows it as a count.
    pub total: i64,
}

/// The position after the last row of a page: the list order is `(updated_at, id)` descending,
/// so the next page starts strictly below it and rows inserted meanwhile cannot shift it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathCursor {
    pub updated_at: i64,
    pub id: Uuid,
}

impl fmt::Display for PathCursor {
    /// Writes the opaque token clients send back unchanged.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_{}", self.updated_at, self.id)
    }
}

impl FromStr for PathCursor {
    type Err = InputError;

    /// Accepts only tokens this server could have written.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || InputError::new(InputErrorKind::Field, "cursor", "expected a list cursor");
        let (updated_at, id) = value.split_once('_').ok_or_else(invalid)?;
        Ok(Self {
            updated_at: updated_at.parse().map_err(|_| invalid())?,
            id: id.parse().map_err(|_| invalid())?,
        })
    }
}

/// A search keyword with surrounding whitespace removed; an all-blank input is no search at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchTerm(String);

impl SearchTerm {
    /// Returns `None` for blank input, so callers cannot run an empty search by accident.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        (!value.is_empty()).then(|| Self(value.to_owned()))
    }

    /// Builds an unanchored ILIKE pattern that matches the term literally: `%`, `_` and the
    /// default escape character `\` in user input must never act as wildcards.
    fn like_pattern(&self) -> String {
        let mut pattern = String::with_capacity(self.0.len() + 2);
        pattern.push('%');
        for c in self.0.chars() {
            if matches!(c, '%' | '_' | '\\') {
                pattern.push('\\');
            }
            pattern.push(c);
        }
        pattern.push('%');
        pattern
    }
}

/// What to list: everything or a search, from the start or after a cursor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversationListRequest {
    pub search: Option<SearchTerm>,
    pub after: Option<PathCursor>,
}

const LIST: &str = "SELECT p.id,p.conversation_id,p.title,c.source,(extract(epoch FROM p.updated_at)*1000)::bigint AS updated_at FROM conversation_path p JOIN conversation c ON c.owner_id=p.owner_id AND c.id=p.conversation_id WHERE p.owner_id=$1 AND ($2::bigint IS NULL OR (p.updated_at,p.id)<(to_timestamp($2::bigint/1000.0),$3)) ORDER BY p.updated_at DESC,p.id DESC LIMIT $4";

// A message on a path is an ancestor-or-self of the path's head, so instead of walking every
// path up to its root, walk down from the matching messages: a path matches when its head is
// one of them or below one. Only subtrees under a match are visited, and the trigram index
// narrows the matches themselves.
const SEARCH: &str = "WITH RECURSIVE hit(conversation_id,id) AS (SELECT conversation_id,id FROM message WHERE owner_id=$1 AND content ILIKE $5 UNION SELECT m.conversation_id,m.id FROM hit h JOIN message m ON m.owner_id=$1 AND m.conversation_id=h.conversation_id AND m.parent_message_id=h.id) SELECT p.id,p.conversation_id,p.title,c.source,(extract(epoch FROM p.updated_at)*1000)::bigint AS updated_at FROM conversation_path p JOIN conversation c ON c.owner_id=p.owner_id AND c.id=p.conversation_id WHERE p.owner_id=$1 AND ($2::bigint IS NULL OR (p.updated_at,p.id)<(to_timestamp($2::bigint/1000.0),$3)) AND (p.title ILIKE $5 OR EXISTS(SELECT 1 FROM hit h WHERE h.conversation_id=p.conversation_id AND h.id=p.head_message_id)) ORDER BY p.updated_at DESC,p.id DESC LIMIT $4";

impl Database {
    /// Lists one page of the owner's paths, newest change first, optionally limited to paths
    /// whose title or messages contain the search term.
    pub async fn conversation_list(
        &self,
        owner: OwnerScope,
        request: &ConversationListRequest,
    ) -> Result<ConversationPage, DbError> {
        let mut tx = self.pool.begin().await?;
        // The count and the page must describe the same moment, or a concurrent import could
        // show a total that disagrees with the rows.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let total: i64 =
            sqlx::query_scalar("SELECT count(*) FROM conversation_path WHERE owner_id=$1")
                .bind(owner.id())
                .fetch_one(&mut *tx)
                .await?;
        let pattern = request.search.as_ref().map(SearchTerm::like_pattern);
        let query = sqlx::query(if pattern.is_some() { SEARCH } else { LIST })
            .bind(owner.id())
            .bind(request.after.map(|cursor| cursor.updated_at))
            .bind(request.after.map(|cursor| cursor.id))
            .bind(CONVERSATION_PAGE_SIZE + 1);
        let query = match pattern {
            Some(pattern) => query.bind(pattern),
            None => query,
        };
        let rows = query.fetch_all(&mut *tx).await?;
        tx.commit().await?;

        let mut items = rows
            .into_iter()
            .map(|row| {
                let source: Source =
                    serde_json::from_value(serde_json::Value::String(row.try_get("source")?))
                        .map_err(|_| DbError::Conflict)?;
                Ok(ConversationListItem {
                    id: row.try_get("id")?,
                    conversation_id: row.try_get("conversation_id")?,
                    title: row.try_get("title")?,
                    source,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        let next_cursor = if items.len() as i64 > CONVERSATION_PAGE_SIZE {
            items.truncate(CONVERSATION_PAGE_SIZE as usize);
            items.last().map(|last| PathCursor {
                updated_at: last.updated_at,
                id: last.id,
            })
        } else {
            None
        };
        Ok(ConversationPage {
            items,
            next_cursor,
            total,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Wildcards and the escape character in a keyword match only themselves.
    /// Core test case:
    /// - `specs/test-cases/web/stars/contents-page.md#search-must-run-on-enter-and-match-only-the-paths-own-title-and-messages`
    #[test]
    fn search_terms_are_trimmed_and_escaped_literally() {
        assert_eq!(SearchTerm::parse("  \t "), None);
        assert_eq!(
            SearchTerm::parse("  分支 ").map(|term| term.like_pattern()),
            Some("%分支%".to_owned())
        );
        assert_eq!(
            SearchTerm::parse(r"50%_a\b").map(|term| term.like_pattern()),
            Some(r"%50\%\_a\\b%".to_owned())
        );
    }

    /// A cursor round-trips through its token, and anything else is a field error.
    #[test]
    fn cursors_round_trip_and_reject_foreign_tokens() {
        let cursor = PathCursor {
            updated_at: 1_759_843_443_123,
            id: Uuid::from_u128(7),
        };
        assert_eq!(cursor.to_string().parse::<PathCursor>(), Ok(cursor));
        for token in [
            "",
            "123",
            "abc_00000000-0000-0000-0000-000000000007",
            "123_xyz",
        ] {
            assert_eq!(
                token.parse::<PathCursor>(),
                Err(InputError::new(
                    InputErrorKind::Field,
                    "cursor",
                    "expected a list cursor"
                ))
            );
        }
    }
}
