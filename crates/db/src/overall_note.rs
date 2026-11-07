//! 总批 of articles, conversation paths and web pages. Like annotations they are outside the sync
//! domain, but a write names the version it replaces: a note is a growing block of text, and a
//! last write that wins would erase a whole paragraph written elsewhere. Every write locks the
//! row of the document it belongs to first, so writes to one note and a binding serialize.
use crate::{
    Database, DbError, OwnerScope,
    web_page::{PAGE_COLUMNS, web_page},
};
use palace_domain::{OverallNoteText, WebPageState, WebPageTitle, WebPageUrl};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

/// The version of a document without an overall note; the sequence starts above it.
const NONE: i64 = 0;

/// A document that takes an overall note directly. A web page is addressed by its URL instead,
/// because its record may not exist yet; see [`Database::save_web_page_overall_note`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverallNoteTarget {
    Article(Uuid),
    Path { conversation: Uuid, path: Uuid },
}

/// An overall note as it stands; a document without one reads as no text at version 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverallNote {
    pub text: Option<String>,
    pub version: i64,
}

impl OverallNote {
    /// What a document without a note reads as.
    pub(crate) fn absent() -> Self {
        Self {
            text: None,
            version: NONE,
        }
    }
}

/// Where a note sits: the one key column of `overall_note` a statement addresses.
#[derive(Clone, Copy)]
pub(crate) enum Slot {
    Article(Uuid),
    Path(Uuid),
    Page(Uuid),
}

impl Slot {
    fn column(self) -> &'static str {
        match self {
            Self::Article(_) => "article_id",
            Self::Path(_) => "path_id",
            Self::Page(_) => "page_id",
        }
    }

    fn id(self) -> Uuid {
        match self {
            Self::Article(id) | Self::Path(id) | Self::Page(id) => id,
        }
    }
}

impl Database {
    /// Opens a document's overall note; another owner's document is not found.
    pub async fn overall_note(
        &self,
        owner: OwnerScope,
        target: OverallNoteTarget,
    ) -> Result<OverallNote, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = locate(&mut tx, owner, target, "FOR KEY SHARE").await?;
        let note = read(&mut tx, owner, row).await?;
        tx.commit().await?;
        Ok(note.unwrap_or_else(OverallNote::absent))
    }

    /// Replaces a document's overall note if it still is at `expected`; no text clears it.
    pub async fn save_overall_note(
        &self,
        owner: OwnerScope,
        target: OverallNoteTarget,
        expected: i64,
        text: Option<&OverallNoteText>,
    ) -> Result<OverallNote, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = locate(&mut tx, owner, target, "FOR NO KEY UPDATE").await?;
        let conversation = match target {
            OverallNoteTarget::Article(_) => None,
            OverallNoteTarget::Path { conversation, .. } => Some(conversation),
        };
        let saved = write(&mut tx, owner, conversation, row, expected, text).await?;
        tx.commit().await?;
        Ok(saved)
    }

    /// Replaces the note of the page at `url`, creating the page with its first note and
    /// recording the tab title either way. Clearing the last thing on a page removes the page,
    /// which exists only for what was left on it. A bound page has no note of its own.
    pub async fn save_web_page_overall_note(
        &self,
        owner: OwnerScope,
        url: &WebPageUrl,
        title: &WebPageTitle,
        expected: i64,
        text: Option<&OverallNoteText>,
    ) -> Result<OverallNote, DbError> {
        let mut tx = self.pool.begin().await?;
        let page = match text {
            // The upsert locks an existing page row, which serializes this with a binding.
            Some(_) => Some(
                sqlx::query(&format!("INSERT INTO web_page AS p(id,owner_id,url,title) VALUES($1,$2,$3,$4) ON CONFLICT(owner_id,url) DO UPDATE SET title=EXCLUDED.title,updated_at=date_trunc('milliseconds',now()) RETURNING {PAGE_COLUMNS}"))
                    .bind(Uuid::now_v7())
                    .bind(owner.id())
                    .bind(url.as_str())
                    .bind(title.as_str())
                    .fetch_one(&mut *tx)
                    .await?,
            ),
            // Clearing must not create a page only to remove it again.
            None => sqlx::query(&format!(
                "SELECT {PAGE_COLUMNS} FROM web_page p WHERE p.owner_id=$1 AND p.url=$2 FOR UPDATE OF p"
            ))
            .bind(owner.id())
            .bind(url.as_str())
            .fetch_optional(&mut *tx)
            .await?,
        };
        let Some(page) = page else {
            return if expected == NONE {
                Ok(OverallNote::absent())
            } else {
                Err(DbError::OverallNoteChanged)
            };
        };
        let page = web_page(&page)?;
        if let WebPageState::Bound(_) = page.state {
            return Err(DbError::WebPageBound);
        }
        let saved = write(
            &mut tx,
            owner,
            NO_CONVERSATION,
            Slot::Page(page.id),
            expected,
            text,
        )
        .await?;
        if text.is_none() {
            sqlx::query("DELETE FROM web_page p WHERE p.owner_id=$1 AND p.id=$2 AND NOT EXISTS(SELECT 1 FROM web_annotation w WHERE w.owner_id=p.owner_id AND w.page_id=p.id)")
                .bind(owner.id())
                .bind(page.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(saved)
    }
}

// The conversation a new row names; only a path has one.
const NO_CONVERSATION: Option<Uuid> = None;

/// Finds and locks the document, so a missing one is not found whatever the note's state.
async fn locate(
    tx: &mut Transaction<'_, Postgres>,
    owner: OwnerScope,
    target: OverallNoteTarget,
    lock: &str,
) -> Result<Slot, DbError> {
    let found = match target {
        OverallNoteTarget::Article(id) => {
            sqlx::query(&format!(
                "SELECT 1 FROM article WHERE owner_id=$1 AND id=$2 {lock}"
            ))
            .bind(owner.id())
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
        }
        OverallNoteTarget::Path { conversation, path } => {
            sqlx::query(&format!(
                "SELECT 1 FROM conversation_path WHERE owner_id=$1 AND id=$2 AND conversation_id=$3 {lock}"
            ))
            .bind(owner.id())
            .bind(path)
            .bind(conversation)
            .fetch_optional(&mut **tx)
            .await?
        }
    };
    found.ok_or(DbError::NotFound)?;
    Ok(match target {
        OverallNoteTarget::Article(id) => Slot::Article(id),
        OverallNoteTarget::Path { path, .. } => Slot::Path(path),
    })
}

/// Reads the note of a located document.
pub(crate) async fn read(
    tx: &mut Transaction<'_, Postgres>,
    owner: OwnerScope,
    row: Slot,
) -> Result<Option<OverallNote>, DbError> {
    let column = row.column();
    sqlx::query(&format!(
        "SELECT text,version FROM overall_note WHERE owner_id=$1 AND {column}=$2"
    ))
    .bind(owner.id())
    .bind(row.id())
    .fetch_optional(&mut **tx)
    .await?
    .map(|found| {
        Ok(OverallNote {
            text: Some(found.try_get("text")?),
            version: found.try_get("version")?,
        })
    })
    .transpose()
}

/// Applies a save under the document's lock: nothing is written unless the note still is at
/// `expected`, and every write takes a new version from the sequence.
async fn write(
    tx: &mut Transaction<'_, Postgres>,
    owner: OwnerScope,
    conversation: Option<Uuid>,
    row: Slot,
    expected: i64,
    text: Option<&OverallNoteText>,
) -> Result<OverallNote, DbError> {
    let current = read(tx, owner, row).await?;
    if current.as_ref().map_or(NONE, |note| note.version) != expected {
        return Err(DbError::OverallNoteChanged);
    }
    let column = row.column();
    match (current, text) {
        (None, None) => Ok(OverallNote::absent()),
        (Some(_), None) => {
            sqlx::query(&format!(
                "DELETE FROM overall_note WHERE owner_id=$1 AND {column}=$2"
            ))
            .bind(owner.id())
            .bind(row.id())
            .execute(&mut **tx)
            .await?;
            Ok(OverallNote::absent())
        }
        (Some(_), Some(text)) => {
            let version = sqlx::query_scalar(&format!("UPDATE overall_note SET text=$3,version=nextval('overall_note_version'),updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND {column}=$2 RETURNING version"))
                .bind(owner.id())
                .bind(row.id())
                .bind(text.as_str())
                .fetch_one(&mut **tx)
                .await?;
            Ok(OverallNote {
                text: Some(text.as_str().to_owned()),
                version,
            })
        }
        (None, Some(text)) => {
            let version = sqlx::query_scalar(&format!("INSERT INTO overall_note(id,owner_id,{column},conversation_id,text) VALUES($1,$2,$3,$4,$5) RETURNING version"))
                .bind(Uuid::now_v7())
                .bind(owner.id())
                .bind(row.id())
                .bind(conversation)
                .bind(text.as_str())
                .fetch_one(&mut **tx)
                .await?;
            Ok(OverallNote {
                text: Some(text.as_str().to_owned()),
                version,
            })
        }
    }
}

/// Moves a page's note into the article it is being bound to, appending to any note already
/// there. Runs in the binding's transaction, which holds the page row; the article's lock
/// serializes this with a reader saving that article's note.
pub(crate) async fn carry_into_article(
    tx: &mut Transaction<'_, Postgres>,
    owner: OwnerScope,
    page: Uuid,
    article: Uuid,
) -> Result<(), DbError> {
    let Some(incoming) = read(tx, owner, Slot::Page(page))
        .await?
        .and_then(|note| note.text)
    else {
        return Ok(());
    };
    let incoming = OverallNoteText::from_stored(incoming);
    let row = locate(
        tx,
        owner,
        OverallNoteTarget::Article(article),
        "FOR NO KEY UPDATE",
    )
    .await?;
    let current = read(tx, owner, row).await?;
    let (expected, merged) = match current {
        Some(OverallNote {
            text: Some(text),
            version,
        }) => (
            version,
            OverallNoteText::from_stored(text).merged(&incoming)?,
        ),
        _ => (NONE, incoming),
    };
    write(tx, owner, NO_CONVERSATION, row, expected, Some(&merged)).await?;
    sqlx::query("DELETE FROM overall_note WHERE owner_id=$1 AND page_id=$2")
        .bind(owner.id())
        .bind(page)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
