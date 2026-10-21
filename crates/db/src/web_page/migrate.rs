//! Moving web annotations into the article their page is bound to. Only the article page can
//! find where a quote sits in the rendered body, so it sends the anchors and the server swaps
//! each web annotation for an article annotation.
use super::{WEB_ANNOTATION_COLUMNS, WebAnnotation, web_annotation};
use crate::{
    Annotation, Database, DbError, OwnerScope,
    annotation::{COLUMNS, annotation},
};
use palace_domain::AnnotationAnchor;
use std::collections::HashSet;
use uuid::Uuid;

// Delete first and insert only for the rows this statement deleted: a concurrent migration of
// the same annotations waits on their row locks, then finds them gone and inserts nothing, so
// each web annotation becomes at most one article annotation without a lock or key of its own.
// Rows of pages not bound to this article, or of another owner, are never deleted and so skipped.
const MIGRATE: &str = "WITH item(source,id,start_offset,end_offset,quote) AS (SELECT * FROM unnest($3::uuid[],$4::uuid[],$5::int[],$6::int[],$7::text[])), moved AS (DELETE FROM web_annotation w USING web_page p WHERE w.owner_id=$1 AND w.id IN (SELECT source FROM item) AND p.owner_id=w.owner_id AND p.id=w.page_id AND p.article_id=$2 RETURNING w.id,w.style,w.note,w.created_at) INSERT INTO annotation(id,owner_id,article_id,start_offset,end_offset,quote,style,note,created_at) SELECT item.id,$1,$2,item.start_offset,item.end_offset,item.quote,moved.style,moved.note,moved.created_at FROM moved JOIN item ON item.source=moved.id RETURNING ";

impl Database {
    /// Lists the annotations left on every page bound to the article, oldest first.
    pub async fn article_web_annotations(
        &self,
        owner: OwnerScope,
        article: Uuid,
    ) -> Result<Vec<WebAnnotation>, DbError> {
        article_exists(self, owner, article).await?;
        sqlx::query(&format!("SELECT {WEB_ANNOTATION_COLUMNS} FROM web_annotation w JOIN web_page p ON p.owner_id=w.owner_id AND p.id=w.page_id WHERE w.owner_id=$1 AND p.article_id=$2 ORDER BY w.id"))
            .bind(owner.id())
            .bind(article)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(web_annotation)
            .collect()
    }

    /// Replaces each listed web annotation with an article annotation at the given anchor, in
    /// one statement: the new annotation keeps the style, note and creation time of the one it
    /// replaces. Listed annotations already gone, or not on a page bound to this article, are
    /// skipped; a failing insert rolls every deletion back. Returns what was created.
    pub async fn migrate_web_annotations(
        &self,
        owner: OwnerScope,
        article: Uuid,
        items: &[(Uuid, AnnotationAnchor)],
    ) -> Result<Vec<Annotation>, DbError> {
        article_exists(self, owner, article).await?;
        // A repeated source would join twice and insert twice; the first anchor wins.
        let mut seen = HashSet::new();
        let items: Vec<_> = items
            .iter()
            .filter(|(source, _)| seen.insert(*source))
            .collect();
        let mut created = sqlx::query(&format!("{MIGRATE}{COLUMNS}"))
            .bind(owner.id())
            .bind(article)
            .bind(items.iter().map(|(source, _)| *source).collect::<Vec<_>>())
            .bind(items.iter().map(|_| Uuid::now_v7()).collect::<Vec<_>>())
            .bind(
                items
                    .iter()
                    .map(|(_, anchor)| anchor.start())
                    .collect::<Vec<_>>(),
            )
            .bind(
                items
                    .iter()
                    .map(|(_, anchor)| anchor.end())
                    .collect::<Vec<_>>(),
            )
            .bind(
                items
                    .iter()
                    .map(|(_, anchor)| anchor.quote())
                    .collect::<Vec<_>>(),
            )
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(annotation)
            .collect::<Result<Vec<_>, _>>()?;
        // New ids were drawn in request order and are time-ordered.
        created.sort_by_key(|annotation| annotation.id);
        Ok(created)
    }
}

/// Fails with not found unless the owner has the article.
async fn article_exists(db: &Database, owner: OwnerScope, article: Uuid) -> Result<(), DbError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM article WHERE owner_id=$1 AND id=$2)")
            .bind(owner.id())
            .bind(article)
            .fetch_one(&db.pool)
            .await?;
    if exists {
        Ok(())
    } else {
        Err(DbError::NotFound)
    }
}
