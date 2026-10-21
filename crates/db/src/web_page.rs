//! Owner-scoped web pages and the annotations the browser extension leaves on them. Like article
//! annotations they are outside the sync domain: the extension writes straight here and the
//! last write wins. Every write to a page's annotations locks the page row first, so binding,
//! annotating and deleting the last annotation of one page serialize.
mod migrate;

use crate::{Database, DbError, OwnerScope, annotation::vanished_target};
use palace_domain::{
    AnnotationContent, AnnotationStyle, WebAnchor, WebPageState, WebPageTitle, WebPageUrl,
};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

/// A page as it may leave the server.
#[derive(Clone, Debug, PartialEq)]
pub struct WebPage {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    pub state: WebPageState,
    /// Unix epoch milliseconds of the last annotation, edit or binding.
    pub updated_at: i64,
}

/// One row of the extension's list.
#[derive(Clone, Debug, PartialEq)]
pub struct WebPageSummary {
    pub page: WebPage,
    /// Annotations still on the page: all of them while unbound, the unmatched ones after.
    pub annotation_count: i64,
}

/// A page with the annotations still on it, oldest first.
#[derive(Clone, Debug, PartialEq)]
pub struct WebPageDetail {
    pub page: WebPage,
    pub annotations: Vec<WebAnnotation>,
}

/// An annotation the extension left on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct WebAnnotation {
    pub id: Uuid,
    pub page_id: Uuid,
    pub anchor: WebAnchor,
    pub content: AnnotationContent,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
}

const PAGE_COLUMNS: &str =
    "p.id,p.url,p.title,p.article_id,(extract(epoch FROM p.updated_at)*1000)::bigint AS updated_at";
pub(crate) const WEB_ANNOTATION_COLUMNS: &str = "w.id,w.page_id,w.quote,w.prefix,w.suffix,w.position,w.style,w.note,(extract(epoch FROM w.created_at)*1000)::bigint AS created_at,(extract(epoch FROM w.updated_at)*1000)::bigint AS updated_at";
const NOW: &str = "date_trunc('milliseconds',now())";

impl Database {
    /// Lists every page the owner annotated or bound, most recently changed first.
    pub async fn web_pages(&self, owner: OwnerScope) -> Result<Vec<WebPageSummary>, DbError> {
        sqlx::query(&format!("SELECT {PAGE_COLUMNS},(SELECT count(*) FROM web_annotation w WHERE w.owner_id=p.owner_id AND w.page_id=p.id) AS annotation_count FROM web_page p WHERE p.owner_id=$1 ORDER BY p.updated_at DESC,p.id DESC"))
            .bind(owner.id())
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| {
                Ok(WebPageSummary {
                    page: web_page(row)?,
                    annotation_count: row.try_get("annotation_count")?,
                })
            })
            .collect()
    }

    /// Opens the owner's page at exactly this address, with its annotations.
    pub async fn web_page_by_url(
        &self,
        owner: OwnerScope,
        url: &WebPageUrl,
    ) -> Result<WebPageDetail, DbError> {
        let mut tx = self.pool.begin().await?;
        // The page and its annotations must describe the same moment.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let page = sqlx::query(&format!(
            "SELECT {PAGE_COLUMNS} FROM web_page p WHERE p.owner_id=$1 AND p.url=$2"
        ))
        .bind(owner.id())
        .bind(url.as_str())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DbError::NotFound)
        .and_then(|row| web_page(&row))?;
        let annotations = sqlx::query(&format!("SELECT {WEB_ANNOTATION_COLUMNS} FROM web_annotation w WHERE w.owner_id=$1 AND w.page_id=$2 ORDER BY w.id"))
            .bind(owner.id())
            .bind(page.id)
            .fetch_all(&mut *tx)
            .await?
            .iter()
            .map(web_annotation)
            .collect::<Result<_, _>>()?;
        tx.commit().await?;
        Ok(WebPageDetail { page, annotations })
    }

    /// Annotates the page at `url`, creating it on its first annotation and recording the tab
    /// title either way. A bound page takes no new annotations.
    pub async fn insert_web_annotation(
        &self,
        owner: OwnerScope,
        url: &WebPageUrl,
        title: &WebPageTitle,
        anchor: &WebAnchor,
        content: &AnnotationContent,
    ) -> Result<WebAnnotation, DbError> {
        let mut tx = self.pool.begin().await?;
        // The upsert locks an existing page row, which is what serializes this with a binding.
        let row = sqlx::query(&format!("INSERT INTO web_page AS p(id,owner_id,url,title) VALUES($1,$2,$3,$4) ON CONFLICT(owner_id,url) DO UPDATE SET title=EXCLUDED.title,updated_at={NOW} RETURNING {PAGE_COLUMNS}"))
            .bind(Uuid::now_v7())
            .bind(owner.id())
            .bind(url.as_str())
            .bind(title.as_str())
            .fetch_one(&mut *tx)
            .await?;
        let page = web_page(&row)?;
        if let WebPageState::Bound(_) = page.state {
            return Err(DbError::WebPageBound);
        }
        let row = sqlx::query(&format!("INSERT INTO web_annotation AS w(id,owner_id,page_id,quote,prefix,suffix,position,style,note) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING {WEB_ANNOTATION_COLUMNS}"))
            .bind(Uuid::now_v7())
            .bind(owner.id())
            .bind(page.id)
            .bind(anchor.quote())
            .bind(anchor.prefix())
            .bind(anchor.suffix())
            .bind(anchor.position())
            .bind(content.style().map(AnnotationStyle::as_str))
            .bind(content.note())
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        web_annotation(&row)
    }

    /// Replaces an annotation's style and note and records the tab title on its page; a blank
    /// title falls back to the page address. Annotations of a bound page no longer change.
    pub async fn update_web_annotation(
        &self,
        owner: OwnerScope,
        id: Uuid,
        content: &AnnotationContent,
        tab_title: &str,
    ) -> Result<WebAnnotation, DbError> {
        let mut tx = self.pool.begin().await?;
        let page = lock_page_of(&mut tx, owner, id).await?;
        if let WebPageState::Bound(_) = page.state {
            return Err(DbError::WebPageBound);
        }
        let url = WebPageUrl::parse(page.url)?;
        let title = WebPageTitle::new(tab_title, &url);
        sqlx::query(&format!(
            "UPDATE web_page SET title=$3,updated_at={NOW} WHERE owner_id=$1 AND id=$2"
        ))
        .bind(owner.id())
        .bind(page.id)
        .bind(title.as_str())
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query(&format!("UPDATE web_annotation AS w SET style=$3,note=$4,updated_at={NOW} WHERE w.owner_id=$1 AND w.id=$2 RETURNING {WEB_ANNOTATION_COLUMNS}"))
            .bind(owner.id())
            .bind(id)
            .bind(content.style().map(AnnotationStyle::as_str))
            .bind(content.note())
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        web_annotation(&row)
    }

    /// Removes an annotation from a page, bound or not. An unbound page exists only for its
    /// annotations, so it goes with its last one in the same transaction.
    pub async fn delete_web_annotation(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        let page = lock_page_of(&mut tx, owner, id).await?;
        sqlx::query("DELETE FROM web_annotation WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let page_change = match page.state {
            WebPageState::Unbound => "DELETE FROM web_page p WHERE p.owner_id=$1 AND p.id=$2 AND NOT EXISTS(SELECT 1 FROM web_annotation w WHERE w.owner_id=p.owner_id AND w.page_id=p.id)".to_owned(),
            WebPageState::Bound(_) => format!("UPDATE web_page SET updated_at={NOW} WHERE owner_id=$1 AND id=$2"),
        };
        sqlx::query(&page_change)
            .bind(owner.id())
            .bind(page.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Binds the page at `url` to one of the owner's articles, or moves an existing binding to
    /// another article, creating the page when the reader never annotated it. Annotations
    /// already migrated stay with the article they moved to.
    pub async fn bind_web_page(
        &self,
        owner: OwnerScope,
        url: &WebPageUrl,
        title: &WebPageTitle,
        article: Uuid,
    ) -> Result<WebPage, DbError> {
        // Selecting from the article keys the binding by owner: another owner's article
        // inserts nothing and is not found.
        let row = sqlx::query(&format!("INSERT INTO web_page AS p(id,owner_id,url,title,article_id) SELECT $1,a.owner_id,$3,$4,a.id FROM article a WHERE a.owner_id=$2 AND a.id=$5 ON CONFLICT(owner_id,url) DO UPDATE SET title=EXCLUDED.title,article_id=EXCLUDED.article_id,updated_at={NOW} RETURNING {PAGE_COLUMNS}"))
            .bind(Uuid::now_v7())
            .bind(owner.id())
            .bind(url.as_str())
            .bind(title.as_str())
            .bind(article)
            .fetch_optional(&self.pool)
            .await
            .map_err(vanished_target)?
            .ok_or(DbError::NotFound)?;
        web_page(&row)
    }
}

/// Locks the page an annotation sits on, so the change and the page's state cannot interleave
/// with a binding or another write to the same page.
async fn lock_page_of(
    tx: &mut Transaction<'_, Postgres>,
    owner: OwnerScope,
    annotation: Uuid,
) -> Result<WebPage, DbError> {
    let row = sqlx::query(&format!("SELECT {PAGE_COLUMNS} FROM web_page p JOIN web_annotation w ON w.owner_id=p.owner_id AND w.page_id=p.id WHERE w.owner_id=$1 AND w.id=$2 FOR UPDATE OF p"))
        .bind(owner.id())
        .bind(annotation)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(DbError::NotFound)?;
    web_page(&row)
}

/// Reads one row selected with [`PAGE_COLUMNS`].
fn web_page(row: &PgRow) -> Result<WebPage, DbError> {
    let state = match row.try_get::<Option<Uuid>, _>("article_id")? {
        Some(article) => WebPageState::Bound(article),
        None => WebPageState::Unbound,
    };
    Ok(WebPage {
        id: row.try_get("id")?,
        url: row.try_get("url")?,
        title: row.try_get("title")?,
        state,
        updated_at: row.try_get("updated_at")?,
    })
}

/// Reads one row selected with [`WEB_ANNOTATION_COLUMNS`].
pub(crate) fn web_annotation(row: &PgRow) -> Result<WebAnnotation, DbError> {
    let style = row
        .try_get::<Option<String>, _>("style")?
        .map(|style| AnnotationStyle::parse(&style).ok_or(DbError::Conflict))
        .transpose()?;
    Ok(WebAnnotation {
        id: row.try_get("id")?,
        page_id: row.try_get("page_id")?,
        anchor: WebAnchor::from_stored(
            row.try_get("quote")?,
            row.try_get("prefix")?,
            row.try_get("suffix")?,
            row.try_get("position")?,
        ),
        content: AnnotationContent::from_stored(style, row.try_get("note")?)
            .ok_or(DbError::Conflict)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
