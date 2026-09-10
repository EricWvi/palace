use palace_domain::{
    PublishedRecord, Record, ServerVersion, SyncPage, UploadResult, accepts_record,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("local persistence failed")]
    Storage(#[from] rusqlite::Error),
    #[error("invalid stored or received record")]
    Json(#[from] serde_json::Error),
    #[error("invalid owner, cursor or upload acknowledgement")]
    Protocol,
    #[error("sync transport failed")]
    Transport,
    #[error("local synchronization mutex poisoned")]
    Poisoned,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mutation {
    pub record: Record,
    pub generation: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalRecord {
    pub mutation: Mutation,
    pub pending: bool,
}
/// One local owner scope; all record, queue and cursor writes share a SQLite transaction.
pub struct Replica {
    pub(crate) connection: Connection,
    owner: Uuid,
}
impl Replica {
    /// Opens persistent local state without treating cached data as authentication credentials.
    pub fn open(path: &std::path::Path, owner: Uuid) -> Result<Self, SyncError> {
        Self::from_connection(Connection::open(path)?, owner)
    }
    /// Initializes owner-keyed tables; separate owner scopes can safely share a physical database.
    pub fn from_connection(connection: Connection, owner: Uuid) -> Result<Self, SyncError> {
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS sync_scope(owner_id TEXT PRIMARY KEY,cursor INTEGER NOT NULL DEFAULT 0,generation INTEGER NOT NULL DEFAULT 0); CREATE TABLE IF NOT EXISTS local_record(owner_id TEXT NOT NULL,id TEXT NOT NULL,record TEXT NOT NULL,generation INTEGER NOT NULL,pending INTEGER NOT NULL,PRIMARY KEY(owner_id,id),FOREIGN KEY(owner_id) REFERENCES sync_scope(owner_id)); CREATE TABLE IF NOT EXISTS derived_job(owner_id TEXT NOT NULL,id TEXT NOT NULL,PRIMARY KEY(owner_id,id),FOREIGN KEY(owner_id,id) REFERENCES local_record(owner_id,id) DEFERRABLE INITIALLY DEFERRED);")?;
        connection.execute(
            "INSERT OR IGNORE INTO sync_scope(owner_id) VALUES(?1)",
            [owner.to_string()],
        )?;
        Ok(Self { connection, owner })
    }
    /// Persists each business edit with an owner-wide generation that survives deletion and recreation.
    pub fn edit(&mut self, record: Record) -> Result<Mutation, SyncError> {
        let tx = self.connection.transaction()?;
        let owner = self.owner.to_string();
        let generation: i64 = tx.query_row(
            "UPDATE sync_scope SET generation=generation+1 WHERE owner_id=?1 RETURNING generation",
            [&owner],
            |row| row.get(/*idx*/ 0),
        )?;
        let mutation = Mutation { record, generation };
        tx.execute("INSERT INTO local_record(owner_id,id,record,generation,pending) VALUES(?1,?2,?3,?4,1) ON CONFLICT(owner_id,id) DO UPDATE SET record=excluded.record,generation=excluded.generation,pending=1",params![owner,mutation.record.id.to_string(),serde_json::to_string(&mutation.record)?,generation])?;
        tx.execute(
            "INSERT OR IGNORE INTO derived_job(owner_id,id) VALUES(?1,?2)",
            params![owner, mutation.record.id.to_string()],
        )?;
        tx.commit()?;
        Ok(mutation)
    }
    /// Returns a stable upload snapshot of persisted pending mutations.
    pub fn pending(&self) -> Result<Vec<Mutation>, SyncError> {
        let mut statement=self.connection.prepare("SELECT record,generation FROM local_record WHERE owner_id=?1 AND pending=1 ORDER BY generation LIMIT 100")?;
        let rows = statement.query_map([self.owner.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.map(|row| {
            let (record, generation) = row?;
            Ok(Mutation {
                record: serde_json::from_str(&record)?,
                generation,
            })
        })
        .collect()
    }
    /// Reads a complete local record for conflict display and deterministic verification.
    pub fn record(&self, id: Uuid) -> Result<Option<LocalRecord>, SyncError> {
        read_record(&self.connection, self.owner, id)
    }
    /// Returns the durable cursor, never an upload response version.
    pub fn cursor(&self) -> Result<ServerVersion, SyncError> {
        let value = self.connection.query_row(
            "SELECT cursor FROM sync_scope WHERE owner_id=?1",
            [self.owner.to_string()],
            |row| row.get(/*idx*/ 0),
        )?;
        ServerVersion::new(value).map_err(|_| SyncError::Protocol)
    }
    /// Confirms exactly the sent generation, then applies current server state against current local state.
    pub fn acknowledge(
        &mut self,
        snapshot: &Mutation,
        result: &UploadResult,
    ) -> Result<(), SyncError> {
        let remote = match result {
            UploadResult::Accepted(remote) | UploadResult::Retained(remote) => remote,
        };
        if remote.owner_id != self.owner || remote.record.id != snapshot.record.id {
            return Err(SyncError::Protocol);
        }
        let tx = self.connection.transaction()?;
        let current = read_record(&tx, self.owner, snapshot.record.id)?;
        // Missing local state may be a received deletion; a delayed upload response cannot recreate it.
        if let Some(current) = current {
            if current.mutation == *snapshot {
                tx.execute(
                    "UPDATE local_record SET pending=0 WHERE owner_id=?1 AND id=?2",
                    params![self.owner.to_string(), snapshot.record.id.to_string()],
                )?;
            }
            apply_remote(&tx, self.owner, remote)?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Atomically applies a strictly ordered page, derived work and its exact last processed cursor.
    pub fn apply_page(&mut self, page: &SyncPage) -> Result<(), SyncError> {
        let tx = self.connection.transaction()?;
        let owner = self.owner.to_string();
        let cursor: i64 = tx.query_row(
            "SELECT cursor FROM sync_scope WHERE owner_id=?1",
            [&owner],
            |row| row.get(/*idx*/ 0),
        )?;
        // A response replay after commit is harmless; the durable cursor already proves page processing.
        if page.cursor.value() <= cursor && !page.records.is_empty() {
            let mut previous = 0;
            for remote in &page.records {
                if remote.owner_id != self.owner || remote.server_version.value() <= previous {
                    return Err(SyncError::Protocol);
                }
                previous = remote.server_version.value();
            }
            if previous != page.cursor.value() {
                return Err(SyncError::Protocol);
            }
            return Ok(());
        }
        let mut last = cursor;
        for remote in &page.records {
            if remote.owner_id != self.owner || remote.server_version.value() <= last {
                return Err(SyncError::Protocol);
            }
            apply_remote(&tx, self.owner, remote)?;
            last = remote.server_version.value();
        }
        if page.cursor.value() != last {
            return Err(SyncError::Protocol);
        }
        tx.execute(
            "UPDATE sync_scope SET cursor=?2 WHERE owner_id=?1",
            params![owner, last],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Exposes durable pending index work for consumers, without changing business timestamps.
    pub fn derived_jobs(&self) -> Result<Vec<String>, SyncError> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM derived_job WHERE owner_id=?1 ORDER BY id")?;
        Ok(statement
            .query_map([self.owner.to_string()], |row| row.get(/*idx*/ 0))?
            .collect::<Result<Vec<_>, _>>()?)
    }
}
/// Reads within either the caller's transaction or its connection without breaking atomic acknowledgement.
fn read_record(
    connection: &Connection,
    owner: Uuid,
    id: Uuid,
) -> Result<Option<LocalRecord>, SyncError> {
    let value = connection
        .query_row(
            "SELECT record,generation,pending FROM local_record WHERE owner_id=?1 AND id=?2",
            params![owner.to_string(), id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            },
        )
        .optional()?;
    value
        .map(|(record, generation, pending)| {
            Ok(LocalRecord {
                mutation: Mutation {
                    record: serde_json::from_str(&record)?,
                    generation,
                },
                pending,
            })
        })
        .transpose()
}
/// Deletes unconditionally for tombstones; normal state replaces only a strictly older business value.
fn apply_remote(
    connection: &Connection,
    owner: Uuid,
    remote: &PublishedRecord,
) -> Result<(), SyncError> {
    let id = remote.record.id.to_string();
    let owner_key = owner.to_string();
    if remote.record.is_deleted {
        connection.execute(
            "DELETE FROM local_record WHERE owner_id=?1 AND id=?2",
            params![owner_key, id],
        )?;
        connection.execute(
            "DELETE FROM derived_job WHERE owner_id=?1 AND id=?2",
            params![owner_key, id],
        )?;
    } else {
        let current = read_record(connection, owner, remote.record.id)?;
        if accepts_record(
            current.as_ref().map(|current| &current.mutation.record),
            &remote.record,
        ) {
            let generation:i64=connection.query_row("UPDATE sync_scope SET generation=generation+1 WHERE owner_id=?1 RETURNING generation",[&owner_key],|row|row.get(/*idx*/ 0))?;
            connection.execute("INSERT INTO local_record(owner_id,id,record,generation,pending) VALUES(?1,?2,?3,?4,0) ON CONFLICT(owner_id,id) DO UPDATE SET record=excluded.record,generation=excluded.generation,pending=0",params![owner_key,id,serde_json::to_string(&remote.record)?,generation])?;
            connection.execute(
                "INSERT OR IGNORE INTO derived_job(owner_id,id) VALUES(?1,?2)",
                params![owner_key, id],
            )?;
        }
    }
    Ok(())
}
