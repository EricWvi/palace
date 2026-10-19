//! The jotting index and its sync feed. Every write goes through [`JottingTransaction`], which
//! holds the jotting publication lock from its first statement to commit, so reading the index,
//! deciding what changed and assigning versions all happen against one serialized state.
use crate::{Database, DbError};
use palace_domain::{InputError, InputErrorKind, Jotting, ServerVersion};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

// The jotting feed has its own lock and sequence: its cursors are unrelated to record sync, and
// sharing them would only make the two feeds wait on each other.
const JOTTING_LOCK: i64 = 734_281_002;
const CONTENT: &str = "(extract(epoch FROM created_at)*1000)::bigint AS created_at,title,tags,body";
const CREATED: &str = "timestamptz 'epoch' + $2::bigint * interval '1 millisecond'";

/// Where a live jotting's file is, relative to the jottings folder, and its Git blob id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JottingLocation {
    pub path: String,
    pub blob_oid: Vec<u8>,
}

/// One row of the index as reconciliation sees it: placement only, never content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexedJotting {
    /// Uploaded and written to `inbox/`, but not yet found in any indexed commit.
    Pending {
        id: Uuid,
        location: JottingLocation,
    },
    Committed {
        id: Uuid,
        location: JottingLocation,
    },
    Deleted {
        id: Uuid,
    },
}

/// What an upload finds under its id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExistingJotting {
    /// Digest of the upload that created the jotting; `None` when the agent created it.
    pub upload_digest: Option<[u8; 32]>,
    pub version: ServerVersion,
}

/// One entry of the sync feed: the current state of a jotting changed after the cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JottingChange {
    Live {
        version: ServerVersion,
        jotting: Jotting,
    },
    Deleted {
        id: Uuid,
        version: ServerVersion,
    },
}

/// One page of the sync feed. `next` covers every row read for the page, including tombstones
/// left out of a first sync, so a client advancing to it never reads those rows again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JottingChanges {
    pub changes: Vec<JottingChange>,
    pub next: ServerVersion,
    pub has_more: bool,
}

/// A transaction holding the jotting publication lock. Dropping it without [`commit`] rolls
/// every write back.
///
/// [`commit`]: JottingTransaction::commit
pub struct JottingTransaction<'a> {
    tx: Transaction<'a, Postgres>,
}

impl Database {
    /// Starts the only kind of transaction that writes jottings, taking the publication lock first.
    pub async fn begin_jottings(&self) -> Result<JottingTransaction<'_>, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(JOTTING_LOCK)
            .execute(&mut *tx)
            .await?;
        Ok(JottingTransaction { tx })
    }

    /// Reads committed changes after `since` in version order. A first sync (`since` 0) leaves
    /// tombstones out: a new device has nothing to delete.
    pub async fn jotting_changes(
        &self,
        since: ServerVersion,
        limit: u32,
    ) -> Result<JottingChanges, DbError> {
        if !(1..=1000).contains(&limit) {
            return Err(InputError::new(InputErrorKind::Field, "limit", "expected 1..1000").into());
        }
        let rows = sqlx::query(&format!(
            "SELECT id,server_version,state,{CONTENT} FROM jotting WHERE server_version>$1 ORDER BY server_version LIMIT $2"
        ))
        .bind(since.value())
        .bind(i64::from(limit) + 1)
        .fetch_all(&self.pool)
        .await?;
        let has_more = rows.len() > limit as usize;
        let mut next = since;
        let mut changes = Vec::with_capacity(rows.len());
        for row in rows.into_iter().take(limit as usize) {
            let version = ServerVersion::new(row.try_get("server_version")?)?;
            next = version;
            let id: Uuid = row.try_get("id")?;
            let state: String = row.try_get("state")?;
            if state == "deleted" {
                if since.value() > 0 {
                    changes.push(JottingChange::Deleted { id, version });
                }
            } else {
                changes.push(JottingChange::Live {
                    version,
                    jotting: decode_jotting(id, &row)?,
                });
            }
        }
        Ok(JottingChanges {
            changes,
            next,
            has_more,
        })
    }
}

impl JottingTransaction<'_> {
    /// Every indexed jotting with its placement, tombstones included.
    pub async fn index(&mut self) -> Result<Vec<IndexedJotting>, DbError> {
        let rows = sqlx::query("SELECT id,state,path,blob_oid FROM jotting ORDER BY id")
            .fetch_all(&mut *self.tx)
            .await?;
        rows.into_iter()
            .map(|row| {
                let id = row.try_get("id")?;
                let state: String = row.try_get("state")?;
                let location = || -> Result<JottingLocation, DbError> {
                    Ok(JottingLocation {
                        path: row.try_get("path")?,
                        blob_oid: row.try_get("blob_oid")?,
                    })
                };
                Ok(match state.as_str() {
                    "pending" => IndexedJotting::Pending {
                        id,
                        location: location()?,
                    },
                    "committed" => IndexedJotting::Committed {
                        id,
                        location: location()?,
                    },
                    _ => IndexedJotting::Deleted { id },
                })
            })
            .collect()
    }

    /// The upload digest and version under `id`, tombstones included.
    pub async fn existing(&mut self, id: Uuid) -> Result<Option<ExistingJotting>, DbError> {
        let Some(row) = sqlx::query("SELECT upload_digest,server_version FROM jotting WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut *self.tx)
            .await?
        else {
            return Ok(None);
        };
        let digest: Option<Vec<u8>> = row.try_get("upload_digest")?;
        Ok(Some(ExistingJotting {
            upload_digest: digest.and_then(|digest| digest.try_into().ok()),
            version: ServerVersion::new(row.try_get("server_version")?)?,
        }))
    }

    /// Indexes an upload whose file is already written; it stays pending until a commit has it.
    pub async fn insert_upload(
        &mut self,
        jotting: &Jotting,
        location: &JottingLocation,
    ) -> Result<ServerVersion, DbError> {
        let statement = format!(
            "INSERT INTO jotting(id,state,upload_digest,path,blob_oid,created_at,title,tags,body) VALUES($1,'pending',$8,$3,$4,{CREATED},$5,$6,$7) RETURNING server_version"
        );
        let query =
            bind_jotting(&statement, jotting, location).bind(jotting.upload_digest().to_vec());
        published(query.fetch_optional(&mut *self.tx).await?)
    }

    /// Indexes a jotting first found in a commit.
    pub async fn insert_committed(
        &mut self,
        jotting: &Jotting,
        location: &JottingLocation,
    ) -> Result<ServerVersion, DbError> {
        let statement = format!(
            "INSERT INTO jotting(id,state,path,blob_oid,created_at,title,tags,body) VALUES($1,'committed',$3,$4,{CREATED},$5,$6,$7) RETURNING server_version"
        );
        let query = bind_jotting(&statement, jotting, location);
        published(query.fetch_optional(&mut *self.tx).await?)
    }

    /// Replaces a jotting with its state in a commit, reviving a tombstone. A new version is
    /// assigned only when the content clients see differs.
    pub async fn update_committed(
        &mut self,
        jotting: &Jotting,
        location: &JottingLocation,
    ) -> Result<ServerVersion, DbError> {
        let statement = format!(
            "UPDATE jotting SET state='committed',path=$3,blob_oid=$4,created_at={CREATED},title=$5,tags=$6,body=$7 WHERE id=$1 RETURNING server_version"
        );
        let query = bind_jotting(&statement, jotting, location);
        published(query.fetch_optional(&mut *self.tx).await?)
    }

    /// Records that a commit holds an indexed jotting's exact bytes at `location`. The content is
    /// unchanged, so no version is assigned.
    pub async fn relocate(&mut self, id: Uuid, location: &JottingLocation) -> Result<(), DbError> {
        let updated = sqlx::query("UPDATE jotting SET state='committed',path=$2,blob_oid=$3 WHERE id=$1 AND state<>'deleted'")
            .bind(id)
            .bind(&location.path)
            .bind(&location.blob_oid)
            .execute(&mut *self.tx)
            .await?;
        if updated.rows_affected() == 1 {
            Ok(())
        } else {
            Err(DbError::NotFound)
        }
    }

    /// Turns a live jotting into a tombstone, keeping the row so the deletion reaches clients.
    pub async fn delete(&mut self, id: Uuid) -> Result<ServerVersion, DbError> {
        let version: i64 = sqlx::query_scalar("UPDATE jotting SET state='deleted',path=NULL,blob_oid=NULL,created_at=NULL,title=NULL,tags=NULL,body=NULL WHERE id=$1 RETURNING server_version")
            .bind(id)
            .fetch_optional(&mut *self.tx)
            .await?
            .ok_or(DbError::NotFound)?;
        Ok(ServerVersion::new(version)?)
    }

    /// Makes every write visible at once and releases the publication lock.
    pub async fn commit(self) -> Result<(), DbError> {
        self.tx.commit().await?;
        Ok(())
    }
}

/// Binds a jotting and its location as `$1` to `$7`, the order every write statement uses.
fn bind_jotting<'q>(
    statement: &'q str,
    jotting: &'q Jotting,
    location: &'q JottingLocation,
) -> sqlx::query::QueryScalar<'q, Postgres, i64, sqlx::postgres::PgArguments> {
    sqlx::query_scalar(statement)
        .bind(jotting.id)
        .bind(jotting.created_at)
        .bind(&location.path)
        .bind(&location.blob_oid)
        .bind(&jotting.title)
        .bind(&jotting.tags)
        .bind(&jotting.body)
}

/// The version a write returned; no row means the jotting to update does not exist.
fn published(version: Option<i64>) -> Result<ServerVersion, DbError> {
    Ok(ServerVersion::new(version.ok_or(DbError::NotFound)?)?)
}

/// Reads the content columns of a live row.
fn decode_jotting(id: Uuid, row: &PgRow) -> Result<Jotting, DbError> {
    Ok(Jotting {
        id,
        created_at: row.try_get("created_at")?,
        title: row.try_get("title")?,
        tags: row.try_get("tags")?,
        body: row.try_get("body")?,
    })
}
