//! Owner-scoped articles and the media they embed. Articles are outside the sync domain, so
//! these writes take no publication lock; object storage is never called inside a transaction,
//! so an import stores its files first and registers them here together with the article.
use crate::{Database, DbError, OwnerScope};
use palace_domain::{ArticleMetadata, DatePrecision, DocumentDate, MediaMetadata};
use sqlx::{Postgres, Row, postgres::PgRow};
use uuid::Uuid;

/// An article as it may leave the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Article {
    /// Equal to the id of the article's moment.
    pub id: Uuid,
    pub metadata: ArticleMetadata,
    pub content: String,
    /// Unix epoch milliseconds of the import, the article's place on the timeline.
    pub occurred_at: i64,
    /// The media the body embeds, oldest first.
    pub media: Vec<ArticleMedia>,
}

/// One embedded file, with the type the page needs to show it as an image or a video.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArticleMedia {
    pub id: Uuid,
    pub content_type: String,
}

/// An object already written to storage, to be registered as media of a new article.
pub struct StoredMedia<'a> {
    pub id: Uuid,
    pub object_key: &'a str,
    pub metadata: &'a MediaMetadata,
}

/// What an import did: the URL identifies an article within an owner, so importing a known
/// URL hands back the article already there instead of a second one.
#[derive(Debug, PartialEq, Eq)]
pub enum ArticleImport {
    Created(Article),
    Existing(Article),
}

const COLUMNS: &str = "a.id,a.title,a.content,a.url,a.author,(extract(epoch FROM a.published_at)*1000)::bigint AS published_at,a.published_precision,(extract(epoch FROM m.occurred_at)*1000)::bigint AS occurred_at";

impl Database {
    /// Finds the owner's article with exactly this URL.
    pub async fn article_by_url(
        &self,
        owner: OwnerScope,
        url: &str,
    ) -> Result<Option<Article>, DbError> {
        let row = sqlx::query(&format!("SELECT {COLUMNS} FROM article a JOIN moment m ON m.owner_id=a.owner_id AND m.id=a.id WHERE a.owner_id=$1 AND a.url=$2"))
            .bind(owner.id())
            .bind(url)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => Ok(Some(self.with_media(owner, row).await?)),
            None => Ok(None),
        }
    }

    /// Opens one of the owner's articles; other owners' ids are indistinguishable from absent ones.
    pub async fn article(&self, owner: OwnerScope, id: Uuid) -> Result<Article, DbError> {
        let row = sqlx::query(&format!("SELECT {COLUMNS} FROM article a JOIN moment m ON m.owner_id=a.owner_id AND m.id=a.id WHERE a.owner_id=$1 AND a.id=$2"))
            .bind(owner.id())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DbError::NotFound)?;
        self.with_media(owner, row).await
    }

    /// Writes the article, its moment and its stored media in one transaction, so the media
    /// rows exist exactly when the article does. When another import committed the same URL
    /// first, nothing is written and that article is returned; the caller still holds the
    /// objects it stored and must remove them.
    pub async fn insert_article(
        &self,
        owner: OwnerScope,
        metadata: &ArticleMetadata,
        content: &str,
        media: &[StoredMedia<'_>],
    ) -> Result<ArticleImport, DbError> {
        let id = Uuid::now_v7();
        match self
            .write_article(owner, id, metadata, content, media)
            .await
        {
            Ok(()) => Ok(ArticleImport::Created(self.article(owner, id).await?)),
            Err(DbError::Conflict) => self
                .article_by_url(owner, metadata.url())
                .await?
                .map(ArticleImport::Existing)
                .ok_or(DbError::Conflict),
            Err(error) => Err(error),
        }
    }

    /// Replaces everything but the body. Another article of the owner already at the new URL
    /// is a conflict and leaves both unchanged.
    pub async fn update_article(
        &self,
        owner: OwnerScope,
        id: Uuid,
        metadata: &ArticleMetadata,
    ) -> Result<Article, DbError> {
        let published = metadata.published();
        let updated = sqlx::query("UPDATE article SET title=$3,url=$4,author=$5,published_at=to_timestamp($6::double precision/1000.0),published_precision=$7 WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .bind(metadata.title())
            .bind(metadata.url())
            .bind(metadata.author())
            .bind(published.at)
            .bind(precision_text(published.precision))
            .execute(&self.pool)
            .await
            .map_err(url_conflict)?;
        if updated.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        self.article(owner, id).await
    }

    /// Lists the object keys of an article's media, so they can be removed before its rows.
    pub async fn article_object_keys(
        &self,
        owner: OwnerScope,
        id: Uuid,
    ) -> Result<Vec<String>, DbError> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM article WHERE owner_id=$1 AND id=$2)")
                .bind(owner.id())
                .bind(id)
                .fetch_one(&self.pool)
                .await?;
        if !exists {
            return Err(DbError::NotFound);
        }
        Ok(sqlx::query_scalar("SELECT md.object_key FROM article_media am JOIN media md ON md.owner_id=am.owner_id AND md.id=am.media_id WHERE am.owner_id=$1 AND am.article_id=$2 ORDER BY md.id")
            .bind(owner.id())
            .bind(id)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Removes the article, its moment and its media rows in one transaction, once the caller
    /// has removed the objects. An article a concurrent delete already removed is not found.
    pub async fn delete_article(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        let media: Vec<Uuid> = sqlx::query_scalar(
            "DELETE FROM article_media WHERE owner_id=$1 AND article_id=$2 RETURNING media_id",
        )
        .bind(owner.id())
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM media WHERE owner_id=$1 AND id=ANY($2)")
            .bind(owner.id())
            .bind(&media)
            .execute(&mut *tx)
            .await?;
        let removed = sqlx::query("DELETE FROM article WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if removed.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        sqlx::query("DELETE FROM moment WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Runs the insert transaction; a URL the owner already has rolls everything back.
    async fn write_article(
        &self,
        owner: OwnerScope,
        id: Uuid,
        metadata: &ArticleMetadata,
        content: &str,
        media: &[StoredMedia<'_>],
    ) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        // The moment's time is the database clock: an article lands on the day it was imported.
        sqlx::query("INSERT INTO moment(id,owner_id,kind,occurred_at) VALUES($1,$2,'article',date_trunc('milliseconds',now()))")
            .bind(id)
            .bind(owner.id())
            .execute(&mut *tx)
            .await?;
        let published = metadata.published();
        sqlx::query("INSERT INTO article(id,owner_id,title,content,url,author,published_at,published_precision) VALUES($1,$2,$3,$4,$5,$6,to_timestamp($7::double precision/1000.0),$8)")
            .bind(id)
            .bind(owner.id())
            .bind(metadata.title())
            .bind(content)
            .bind(metadata.url())
            .bind(metadata.author())
            .bind(published.at)
            .bind(precision_text(published.precision))
            .execute(&mut *tx)
            .await
            .map_err(url_conflict)?;
        for stored in media {
            insert_media_row(&mut tx, owner, stored).await?;
            sqlx::query("INSERT INTO article_media(owner_id,article_id,media_id) VALUES($1,$2,$3)")
                .bind(owner.id())
                .bind(id)
                .bind(stored.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Completes an article row with its media list.
    async fn with_media(&self, owner: OwnerScope, row: PgRow) -> Result<Article, DbError> {
        let id: Uuid = row.try_get("id")?;
        let media = sqlx::query("SELECT md.id,md.content_type FROM article_media am JOIN media md ON md.owner_id=am.owner_id AND md.id=am.media_id WHERE am.owner_id=$1 AND am.article_id=$2 ORDER BY md.id")
            .bind(owner.id())
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                Ok(ArticleMedia {
                    id: row.try_get("id")?,
                    content_type: row.try_get("content_type")?,
                })
            })
            .collect::<Result<_, DbError>>()?;
        Ok(Article {
            id,
            metadata: stored_metadata(&row)?,
            content: row.try_get("content")?,
            occurred_at: row.try_get("occurred_at")?,
            media,
        })
    }
}

/// Registers one stored object inside the article's transaction.
async fn insert_media_row(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    owner: OwnerScope,
    stored: &StoredMedia<'_>,
) -> Result<(), DbError> {
    let metadata = stored.metadata;
    sqlx::query("INSERT INTO media(id,owner_id,object_key,content_type,byte_size,sha256,file_name) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(stored.id)
        .bind(owner.id())
        .bind(stored.object_key)
        .bind(metadata.content_type())
        .bind(metadata.byte_size())
        .bind(metadata.sha256().as_slice())
        .bind(metadata.file_name())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Reads the editable fields of an article row selected with the list or detail columns.
pub(crate) fn stored_metadata(row: &PgRow) -> Result<ArticleMetadata, DbError> {
    let precision = match row.try_get::<String, _>("published_precision")?.as_str() {
        "day" => DatePrecision::Day,
        "instant" => DatePrecision::Instant,
        // The check constraint admits no other precision.
        _ => return Err(DbError::Conflict),
    };
    Ok(ArticleMetadata::from_stored(
        row.try_get("title")?,
        row.try_get("url")?,
        row.try_get("author")?,
        DocumentDate {
            at: row.try_get("published_at")?,
            precision,
        },
    ))
}

/// Spells a precision as the `published_precision` column does.
fn precision_text(precision: DatePrecision) -> &'static str {
    match precision {
        DatePrecision::Day => "day",
        DatePrecision::Instant => "instant",
    }
}

/// Reports a URL the owner already has as a conflict; other failures stay storage errors.
fn url_conflict(error: sqlx::Error) -> DbError {
    let duplicate = error
        .as_database_error()
        .is_some_and(|error| error.constraint() == Some("article_owner_id_url_key"));
    if duplicate {
        DbError::Conflict
    } else {
        error.into()
    }
}
