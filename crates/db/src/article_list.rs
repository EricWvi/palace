//! The 摘星 article list: the owner's articles by publish date, newest first, one page at a time.
use crate::{Database, DbError, ListCursor, OwnerScope, SearchTerm, article::stored_metadata};
use palace_domain::ArticleMetadata;
use sqlx::Row;
use uuid::Uuid;

/// Rows per page; one more is read to learn whether another page follows.
pub const ARTICLE_PAGE_SIZE: i64 = 50;

/// One row of the list: what the article is and when it was written, not when it was imported.
#[derive(Debug, PartialEq, Eq)]
pub struct ArticleListItem {
    pub id: Uuid,
    pub metadata: ArticleMetadata,
}

/// One page of the list plus the owner's article count.
#[derive(Debug, PartialEq, Eq)]
pub struct ArticlePage {
    pub items: Vec<ArticleListItem>,
    /// Present only when more rows follow this page; its time is a publish date.
    pub next_cursor: Option<ListCursor>,
    /// Every article of the owner, whatever the search; the kind column shows it as a count.
    pub total: i64,
}

/// What to list: everything or a search, from the start or after a cursor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArticleListRequest {
    pub search: Option<SearchTerm>,
    pub after: Option<ListCursor>,
}

// A blank pattern never reaches the query: without a search `$5` is NULL and matches every row.
// The body is the largest column and carries a trigram index; title and author are short.
const LIST: &str = "SELECT id,title,url,author,(extract(epoch FROM published_at)*1000)::bigint AS published_at,published_precision FROM article WHERE owner_id=$1 AND ($2::bigint IS NULL OR (published_at,id)<(to_timestamp($2::double precision/1000.0),$3)) AND ($5::text IS NULL OR title ILIKE $5 OR author ILIKE $5 OR content ILIKE $5) ORDER BY published_at DESC,id DESC LIMIT $4";

impl Database {
    /// Lists one page of the owner's articles, latest publish date first, optionally limited to
    /// articles whose title, author or body contains the search term.
    pub async fn article_list(
        &self,
        owner: OwnerScope,
        request: &ArticleListRequest,
    ) -> Result<ArticlePage, DbError> {
        let mut tx = self.pool.begin().await?;
        // The count and the page must describe the same moment, as in the conversation list.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let total: i64 = sqlx::query_scalar("SELECT count(*) FROM article WHERE owner_id=$1")
            .bind(owner.id())
            .fetch_one(&mut *tx)
            .await?;
        let rows = sqlx::query(LIST)
            .bind(owner.id())
            .bind(request.after.map(|cursor| cursor.at))
            .bind(request.after.map(|cursor| cursor.id))
            .bind(ARTICLE_PAGE_SIZE + 1)
            .bind(request.search.as_ref().map(SearchTerm::like_pattern))
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        let mut items = rows
            .iter()
            .map(|row| {
                Ok(ArticleListItem {
                    id: row.try_get("id")?,
                    metadata: stored_metadata(row)?,
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        let next_cursor = if items.len() as i64 > ARTICLE_PAGE_SIZE {
            items.truncate(ARTICLE_PAGE_SIZE as usize);
            items.last().map(|last| ListCursor {
                at: last.metadata.published().at,
                id: last.id,
            })
        } else {
            None
        };
        Ok(ArticlePage {
            items,
            next_cursor,
            total,
        })
    }
}
