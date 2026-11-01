//! Web edits and deletions of jottings: the index side of a write the server has already made
//! to the work tree, and the record reconciliation uses to keep that write until a commit
//! carries it. Every statement runs in a [`JottingTransaction`], under the publication lock.
//! See `specs/decisions/server/jotting/20261101-edit-and-delete-jottings-from-the-web-in-the-work-tree-until-committed.md`.
use crate::{
    DbError, JottingLocation, JottingTransaction,
    jotting::{CREATED, bind_jotting, published},
};
use palace_domain::{Jotting, ServerVersion};
use sqlx::Row;
use uuid::Uuid;

/// What a web write finds under its id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebTarget {
    Live(LiveTarget),
    /// A tombstone: the jotting was deleted after the page read it.
    Deleted,
}

/// A live jotting as a web write starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveTarget {
    pub version: ServerVersion,
    pub stage: JottingStage,
    /// What the web shows: an edit that changes none of it writes nothing.
    pub jotting: Jotting,
    pub location: JottingLocation,
}

/// Whether a commit has ever held the jotting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JottingStage {
    /// An upload still waiting in `inbox/` for its first commit; the upload rules keep it, so
    /// a web write to it needs no record.
    Pending,
    Committed,
}

/// One web write no commit carries yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebWrite {
    pub id: Uuid,
    /// Where the work tree holds the web's result.
    pub path: String,
    /// The jotting's blob in the commit the web write started from.
    pub base_oid: Vec<u8>,
    /// The blob the web wrote; `None` for a deletion.
    pub written_oid: Option<Vec<u8>>,
}

impl JottingTransaction<'_> {
    /// The jotting a web write names, or `None` when no jotting ever had the id.
    pub async fn web_target(&mut self, id: Uuid) -> Result<Option<WebTarget>, DbError> {
        let Some(row) = sqlx::query(
            "SELECT state,server_version,path,blob_oid,(extract(epoch FROM created_at)*1000)::bigint AS created_at,title,tags,body FROM jotting WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(&mut *self.tx)
        .await?
        else {
            return Ok(None);
        };
        let state: String = row.try_get("state")?;
        let stage = match state.as_str() {
            "pending" => JottingStage::Pending,
            "committed" => JottingStage::Committed,
            _ => return Ok(Some(WebTarget::Deleted)),
        };
        Ok(Some(WebTarget::Live(LiveTarget {
            version: ServerVersion::new(row.try_get("server_version")?)?,
            stage,
            jotting: Jotting {
                id,
                created_at: row.try_get("created_at")?,
                title: row.try_get("title")?,
                tags: row.try_get("tags")?,
                body: row.try_get("body")?,
            },
            location: JottingLocation {
                path: row.try_get("path")?,
                blob_oid: row.try_get("blob_oid")?,
            },
        })))
    }

    /// Indexes the file a web edit wrote at `target`'s path and publishes it. For a committed
    /// jotting it records the write, keeping the base of the first write when there are several.
    pub async fn save_web_edit(
        &mut self,
        target: &LiveTarget,
        jotting: &Jotting,
        blob_oid: &[u8],
    ) -> Result<ServerVersion, DbError> {
        let location = JottingLocation {
            path: target.location.path.clone(),
            blob_oid: blob_oid.to_vec(),
        };
        let statement = format!(
            "UPDATE jotting SET path=$3,blob_oid=$4,created_at={CREATED},title=$5,tags=$6,body=$7 WHERE id=$1 AND state<>'deleted' RETURNING server_version"
        );
        let version = published(
            bind_jotting(&statement, jotting, &location)
                .fetch_optional(&mut *self.tx)
                .await?,
        )?;
        self.record_web_write(target, Some(blob_oid)).await?;
        Ok(version)
    }

    /// Turns the jotting whose file a web deletion removed into a tombstone and records it.
    pub async fn save_web_delete(&mut self, target: &LiveTarget) -> Result<ServerVersion, DbError> {
        let version = self.delete(target.jotting.id).await?;
        self.record_web_write(target, None).await?;
        Ok(version)
    }

    /// Records a web write to a committed jotting; an upload never committed needs none.
    async fn record_web_write(
        &mut self,
        target: &LiveTarget,
        written_oid: Option<&[u8]>,
    ) -> Result<(), DbError> {
        match target.stage {
            JottingStage::Pending => Ok(()),
            // While a web write is recorded, the index holds the web's blob, not the commit's;
            // the base stays the one the first write started from.
            JottingStage::Committed => {
                sqlx::query(
                    "INSERT INTO jotting_web_write(jotting_id,path,base_oid,written_oid) VALUES($1,$2,$3,$4)
                     ON CONFLICT(jotting_id) DO UPDATE SET path=EXCLUDED.path,written_oid=EXCLUDED.written_oid,written_at=date_trunc('milliseconds',now())",
                )
                .bind(target.jotting.id)
                .bind(&target.location.path)
                .bind(&target.location.blob_oid)
                .bind(written_oid)
                .execute(&mut *self.tx)
                .await?;
                Ok(())
            }
        }
    }

    /// Every web write no commit carries yet.
    pub async fn web_writes(&mut self) -> Result<Vec<WebWrite>, DbError> {
        let rows = sqlx::query(
            "SELECT jotting_id,path,base_oid,written_oid FROM jotting_web_write ORDER BY jotting_id",
        )
        .fetch_all(&mut *self.tx)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(WebWrite {
                    id: row.try_get("jotting_id")?,
                    path: row.try_get("path")?,
                    base_oid: row.try_get("base_oid")?,
                    written_oid: row.try_get("written_oid")?,
                })
            })
            .collect()
    }

    /// A commit left a web write out and the work tree still holds it at `path`, where the
    /// commit now has the jotting: the record, and a live jotting, follow the file there.
    pub async fn follow_web_write(&mut self, id: Uuid, path: &str) -> Result<(), DbError> {
        sqlx::query("UPDATE jotting_web_write SET path=$2 WHERE jotting_id=$1")
            .bind(id)
            .bind(path)
            .execute(&mut *self.tx)
            .await?;
        sqlx::query("UPDATE jotting SET path=$2 WHERE id=$1 AND state<>'deleted'")
            .bind(id)
            .bind(path)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }

    /// The commit decides the jotting again: it carries the web write, changed the jotting
    /// itself, or the work tree undid the write.
    pub async fn forget_web_write(&mut self, id: Uuid) -> Result<(), DbError> {
        sqlx::query("DELETE FROM jotting_web_write WHERE jotting_id=$1")
            .bind(id)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }
}
