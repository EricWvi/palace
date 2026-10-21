//! Owner-scoped annotations on article bodies and conversation messages. Annotations are outside
//! the sync domain, so these writes take no publication lock; foreign keys cascade them away with
//! the text they mark.
use crate::{Database, DbError, OwnerScope};
use palace_domain::{AnnotationAnchor, AnnotationContent, AnnotationStyle, AnnotationTarget};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

/// An annotation as it may leave the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub id: Uuid,
    pub target: AnnotationTarget,
    pub anchor: AnnotationAnchor,
    pub content: AnnotationContent,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
}

const COLUMNS: &str = "id,article_id,conversation_id,message_id,start_offset,end_offset,quote,style,note,(extract(epoch FROM created_at)*1000)::bigint AS created_at,(extract(epoch FROM updated_at)*1000)::bigint AS updated_at";

impl Database {
    /// Lists an article's annotations in creation order; another owner's article is not found.
    pub async fn article_annotations(
        &self,
        owner: OwnerScope,
        article: Uuid,
    ) -> Result<Vec<Annotation>, DbError> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM article WHERE owner_id=$1 AND id=$2)")
                .bind(owner.id())
                .bind(article)
                .fetch_one(&self.pool)
                .await?;
        if !exists {
            return Err(DbError::NotFound);
        }
        sqlx::query(&format!(
            "SELECT {COLUMNS} FROM annotation WHERE owner_id=$1 AND article_id=$2 ORDER BY id"
        ))
        .bind(owner.id())
        .bind(article)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(annotation)
        .collect()
    }

    /// Lists the annotations on every message of a conversation, whichever paths share them.
    pub async fn conversation_annotations(
        &self,
        owner: OwnerScope,
        conversation: Uuid,
    ) -> Result<Vec<Annotation>, DbError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM conversation WHERE owner_id=$1 AND id=$2)",
        )
        .bind(owner.id())
        .bind(conversation)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Err(DbError::NotFound);
        }
        sqlx::query(&format!(
            "SELECT {COLUMNS} FROM annotation WHERE owner_id=$1 AND conversation_id=$2 ORDER BY id"
        ))
        .bind(owner.id())
        .bind(conversation)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(annotation)
        .collect()
    }

    /// Annotates text the owner has. The insert selects from the target itself, so a target of
    /// another owner, or a message of another conversation, writes nothing and is not found.
    pub async fn insert_annotation(
        &self,
        owner: OwnerScope,
        target: AnnotationTarget,
        anchor: &AnnotationAnchor,
        content: &AnnotationContent,
    ) -> Result<Annotation, DbError> {
        // Both statements select from the target itself, keyed by the owner, and take the
        // target columns from the row they found.
        let (select, key, conversation) = match target {
            AnnotationTarget::Article(article) => (
                "SELECT $1,owner_id,id,NULL::uuid,NULL::uuid,$3,$4,$5,$6,$7 FROM article WHERE owner_id=$2 AND id=$8 AND $9::uuid IS NULL",
                article,
                None,
            ),
            AnnotationTarget::Message {
                conversation,
                message,
            } => (
                "SELECT $1,owner_id,NULL::uuid,conversation_id,id,$3,$4,$5,$6,$7 FROM message WHERE owner_id=$2 AND id=$8 AND conversation_id=$9",
                message,
                Some(conversation),
            ),
        };
        let row = sqlx::query(&format!(
            "INSERT INTO annotation(id,owner_id,article_id,conversation_id,message_id,start_offset,end_offset,quote,style,note) {select} RETURNING {COLUMNS}"
        ))
        .bind(Uuid::now_v7())
        .bind(owner.id())
        .bind(anchor.start())
        .bind(anchor.end())
        .bind(anchor.quote())
        .bind(content.style().map(AnnotationStyle::as_str))
        .bind(content.note())
        .bind(key)
        .bind(conversation)
        .fetch_optional(&self.pool)
        .await
        .map_err(vanished_target)?
        .ok_or(DbError::NotFound)?;
        annotation(&row)
    }

    /// Replaces the style and the note; the anchor never changes.
    pub async fn update_annotation(
        &self,
        owner: OwnerScope,
        id: Uuid,
        content: &AnnotationContent,
    ) -> Result<Annotation, DbError> {
        let row = sqlx::query(&format!("UPDATE annotation SET style=$3,note=$4,updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND id=$2 RETURNING {COLUMNS}"))
            .bind(owner.id())
            .bind(id)
            .bind(content.style().map(AnnotationStyle::as_str))
            .bind(content.note())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DbError::NotFound)?;
        annotation(&row)
    }

    /// Removes one of the owner's annotations.
    pub async fn delete_annotation(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let removed = sqlx::query("DELETE FROM annotation WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }
}

/// Reads one row selected with [`COLUMNS`].
fn annotation(row: &PgRow) -> Result<Annotation, DbError> {
    let article: Option<Uuid> = row.try_get("article_id")?;
    let conversation: Option<Uuid> = row.try_get("conversation_id")?;
    let message: Option<Uuid> = row.try_get("message_id")?;
    // The target check constraint admits exactly these two shapes.
    let target = match (article, conversation, message) {
        (Some(article), None, None) => AnnotationTarget::Article(article),
        (None, Some(conversation), Some(message)) => AnnotationTarget::Message {
            conversation,
            message,
        },
        _ => return Err(DbError::Conflict),
    };
    let style = row
        .try_get::<Option<String>, _>("style")?
        .map(|style| AnnotationStyle::parse(&style).ok_or(DbError::Conflict))
        .transpose()?;
    Ok(Annotation {
        id: row.try_get("id")?,
        target,
        anchor: AnnotationAnchor::from_stored(
            row.try_get("start_offset")?,
            row.try_get("end_offset")?,
            row.try_get("quote")?,
        ),
        content: AnnotationContent::from_stored(style, row.try_get("note")?)
            .ok_or(DbError::Conflict)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// A target deleted between the select and the insert fails its foreign key; that is the same
/// as never having existed.
fn vanished_target(error: sqlx::Error) -> DbError {
    let missing = error
        .as_database_error()
        .is_some_and(|error| error.code().as_deref() == Some("23503"));
    if missing {
        DbError::NotFound
    } else {
        error.into()
    }
}
