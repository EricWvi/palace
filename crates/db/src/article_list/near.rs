//! The article list ordered by how near each article's address is to a page the reader is
//! binding. Nearness comes from parsing both URLs, which SQL cannot do the way the browser
//! does, so the page reads every matching article's address and time, ranks them here, and
//! loads only the rows of the requested page.
use super::{ARTICLE_PAGE_SIZE, ArticleListItem};
use crate::{DbError, OwnerScope, SearchTerm, article::stored_metadata};
use palace_domain::{InputError, InputErrorKind, address_nearness};
use sqlx::{Postgres, Row, Transaction};
use std::{cmp::Reverse, fmt, str::FromStr};
use uuid::Uuid;

/// The position after the last row of a near-ordered page. Rows are ordered by nearness, then
/// import time, then id, all descending, so the next page starts strictly below this key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NearCursor {
    /// [`address_nearness`] of the row.
    pub nearness: u32,
    /// Unix epoch milliseconds of the row's import.
    pub at: i64,
    pub id: Uuid,
}

impl fmt::Display for NearCursor {
    /// Writes the opaque token clients send back unchanged.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_{}_{}", self.nearness, self.at, self.id)
    }
}

impl FromStr for NearCursor {
    type Err = InputError;

    /// Accepts only tokens this order could have written; a publish-date cursor has one part
    /// fewer and is refused.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || InputError::new(InputErrorKind::Field, "cursor", "expected a list cursor");
        let mut parts = value.splitn(3, '_');
        let mut next = || parts.next().ok_or_else(invalid);
        Ok(Self {
            nearness: next()?.parse().map_err(|_| invalid())?,
            at: next()?.parse().map_err(|_| invalid())?,
            id: next()?.parse().map_err(|_| invalid())?,
        })
    }
}

// Same search as the publish-date list.
const KEYS: &str = "SELECT a.id,a.url,(extract(epoch FROM m.occurred_at)*1000)::bigint AS occurred_at FROM article a JOIN moment m ON m.owner_id=a.owner_id AND m.id=a.id WHERE a.owner_id=$1 AND ($2::text IS NULL OR a.title ILIKE $2 OR a.author ILIKE $2 OR a.content ILIKE $2)";
const ROWS: &str = "SELECT id,title,url,author,(extract(epoch FROM published_at)*1000)::bigint AS published_at,published_precision FROM article WHERE owner_id=$1 AND id=ANY($2)";

/// Reads one near-ordered page inside the list's snapshot, with the cursor of the next page.
pub(super) async fn page(
    tx: &mut Transaction<'_, Postgres>,
    owner: OwnerScope,
    search: Option<&SearchTerm>,
    near: &str,
    after: Option<NearCursor>,
) -> Result<(Vec<ArticleListItem>, Option<NearCursor>), DbError> {
    let mut keys = sqlx::query(KEYS)
        .bind(owner.id())
        .bind(search.map(SearchTerm::like_pattern))
        .fetch_all(&mut **tx)
        .await?
        .iter()
        .map(|row| {
            Ok(NearCursor {
                nearness: address_nearness(row.try_get("url")?, near),
                at: row.try_get("occurred_at")?,
                id: row.try_get("id")?,
            })
        })
        .collect::<Result<Vec<_>, DbError>>()?;
    keys.retain(|key| after.is_none_or(|after| *key < after));
    keys.sort_unstable_by_key(|key| Reverse(*key));
    let next_cursor = if keys.len() as i64 > ARTICLE_PAGE_SIZE {
        keys.truncate(ARTICLE_PAGE_SIZE as usize);
        keys.last().copied()
    } else {
        None
    };
    let ids: Vec<Uuid> = keys.iter().map(|key| key.id).collect();
    let mut rows = sqlx::query(ROWS)
        .bind(owner.id())
        .bind(&ids)
        .fetch_all(&mut **tx)
        .await?
        .iter()
        .map(|row| {
            Ok(ArticleListItem {
                id: row.try_get("id")?,
                metadata: stored_metadata(row)?,
            })
        })
        .collect::<Result<Vec<_>, DbError>>()?;
    rows.sort_by_key(|item| ids.iter().position(|id| *id == item.id));
    Ok((rows, next_cursor))
}
