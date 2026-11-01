//! Web edits and deletions: the server rewrites or removes the jotting's file in the work tree,
//! then indexes and publishes the result at once. It still never commits; reconciliation keeps
//! the web's result until a commit carries it (see `reconcile`).
//! See `specs/decisions/server/jotting/20261101-edit-and-delete-jottings-from-the-web-in-the-work-tree-until-committed.md`.
use super::{Jottings, JottingsError, blocking, folder_file};
use palace_db::{LiveTarget, WebTarget};
use palace_domain::{Jotting, ServerVersion};
use palace_logging::palace_error;
use std::{io::Write, path::Path};
use uuid::Uuid;

/// What a web edit sets; `id` and `created` are not the web's to change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JottingEdit {
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub body: String,
}

impl Jottings {
    /// Sets a jotting's title, tags and body, as of `version`. The file keeps every other byte.
    /// An edit that changes nothing the web shows writes nothing and returns the current version.
    pub async fn edit(
        &self,
        id: Uuid,
        version: ServerVersion,
        edit: JottingEdit,
    ) -> Result<ServerVersion, JottingsError> {
        let result = self.write_edit(id, version, edit).await;
        log_failure("Jotting edit failed", id, &result);
        result
    }

    /// Deletes a jotting's file, as of `version`, and publishes the deletion.
    pub async fn delete(&self, id: Uuid, version: ServerVersion) -> Result<(), JottingsError> {
        let result = self.write_delete(id, version).await;
        log_failure("Jotting deletion failed", id, &result);
        result
    }

    /// Checks, rewrites and indexes one edit under the publication lock. The file is written
    /// before the row, as for uploads: a failure in between leaves the index as it was, and a
    /// retry with the same version finds the jotting's own file and writes the same bytes.
    async fn write_edit(
        &self,
        id: Uuid,
        version: ServerVersion,
        edit: JottingEdit,
    ) -> Result<ServerVersion, JottingsError> {
        let mut tx = self.database.begin_jottings().await?;
        let target = current(tx.web_target(id).await?, version)?;
        let file = folder_file(self.folder(), &target.location.path);
        let text = {
            let file = file.clone();
            blocking(move || std::fs::read(file)).await?
        };
        let existing = own_file(text, id)?;
        // The file decides `created`: the agent may have changed it without committing yet.
        let edited = Jotting {
            id,
            created_at: existing.jotting.created_at,
            title: edit.title.filter(|title| !title.trim().is_empty()),
            tags: edit.tags,
            body: edit.body,
        }
        .upload()?;
        if edited == target.jotting {
            return Ok(target.version);
        }
        let rewritten = edited.rewrite(&existing.text).ok_or(JottingsError::Moved)?;
        let blob = self.repo.blob_oid(rewritten.as_bytes())?;
        blocking(move || replace(&file, rewritten.as_bytes())).await??;
        let version = tx.save_web_edit(&target, &edited, blob.as_bytes()).await?;
        tx.commit().await?;
        Ok(version)
    }

    /// Checks, removes and indexes one deletion under the publication lock.
    async fn write_delete(&self, id: Uuid, version: ServerVersion) -> Result<(), JottingsError> {
        let mut tx = self.database.begin_jottings().await?;
        let target = current(tx.web_target(id).await?, version)?;
        let file = folder_file(self.folder(), &target.location.path);
        let text = {
            let file = file.clone();
            blocking(move || std::fs::read(file)).await?
        };
        own_file(text, id)?;
        blocking(move || std::fs::remove_file(file)).await??;
        tx.save_web_delete(&target).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// The live jotting a web write names, if it is still at the version the page read.
fn current(target: Option<WebTarget>, version: ServerVersion) -> Result<LiveTarget, JottingsError> {
    match target {
        None => Err(palace_db::DbError::NotFound.into()),
        Some(WebTarget::Deleted) => Err(JottingsError::Changed),
        Some(WebTarget::Live(target)) if target.version != version => Err(JottingsError::Changed),
        Some(WebTarget::Live(target)) => Ok(target),
    }
}

/// A jotting file as read from the work tree.
struct OwnFile {
    text: String,
    jotting: Jotting,
}

/// The file at the indexed path, if it is still this jotting's: a missing file, one that does
/// not parse, or another jotting's means the agent is moving files, and writing there would
/// hit the wrong place.
fn own_file(read: std::io::Result<Vec<u8>>, id: Uuid) -> Result<OwnFile, JottingsError> {
    let bytes = match read {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(JottingsError::Moved);
        }
        Err(error) => return Err(error.into()),
    };
    let jotting = Jotting::parse(&bytes).map_err(|_| JottingsError::Moved)?;
    if jotting.id != id {
        return Err(JottingsError::Moved);
    }
    let text = String::from_utf8(bytes).map_err(|_| JottingsError::Moved)?;
    Ok(OwnFile { text, jotting })
}

/// Replaces `target` with `bytes` through a temporary file and rename, so the agent never sees
/// half a file.
fn replace(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let directory = target
        .parent()
        .ok_or_else(|| std::io::Error::other("jotting file has no directory"))?;
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // A dot file without the .md extension is never a jotting, even if it is left behind.
    let temporary = directory.join(format!(".{name}.tmp"));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, target)
}

/// Logs a write that failed on the server's side; clients only learn that it can be retried.
fn log_failure<T>(message: &'static str, id: Uuid, result: &Result<T, JottingsError>) {
    if let Err(error @ (JottingsError::Git(_) | JottingsError::File(_))) = result {
        palace_error!(message = message, id = %id, error = %error);
    }
}
