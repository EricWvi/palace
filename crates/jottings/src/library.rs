use crate::{
    reconcile::{Step, plan, unknown_blobs},
    repo::{GitError, OpenError, Repo},
};
use chrono::{Offset, TimeZone};
use chrono_tz::Tz;
use gix::ObjectId;
use palace_db::{Database, DbError, JottingChanges, JottingLocation};
use palace_domain::{InputError, Jotting, JottingProblem, ServerVersion};
use palace_logging::{palace_error, palace_info};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};
use time::UtcOffset;

/// The folder below the jottings folder where uploads are written for the agent to sort.
pub const INBOX: &str = "inbox";

/// Whether this deployment serves jottings.
pub enum JottingService {
    /// No folder configured: the feed is empty and uploads are refused as unavailable.
    Unconfigured,
    Configured(Arc<Jottings>),
}

/// A configured jottings folder, its index and the commit it was last reconciled with.
pub struct Jottings {
    repo: Arc<Repo>,
    database: Database,
    timezone: Tz,
    // Held for a whole reconciliation, so two triggers never reconcile at once.
    progress: tokio::sync::Mutex<Progress>,
    // Counts reconciliations that looked at a new HEAD, so a watcher's effect can be awaited
    // instead of slept on.
    generation: tokio::sync::watch::Sender<u64>,
}

/// Which commits a reconciliation has already looked at.
#[derive(Default)]
struct Progress {
    /// The last commit looked at, indexed or refused; the same HEAD is not looked at twice.
    seen: Option<ObjectId>,
    /// The commit the index matches.
    indexed: Option<ObjectId>,
}

/// What one reconciliation did.
#[derive(Debug, PartialEq, Eq)]
pub enum Reconciled {
    /// HEAD has not moved since it was last looked at.
    Unchanged,
    /// The index now matches `commit`; `writes` counts the index rows written.
    Indexed { commit: ObjectId, writes: usize },
    /// `commit` was refused as a whole; the index still matches the previous good commit.
    Refused {
        commit: ObjectId,
        problems: Vec<JottingProblem>,
    },
}

/// How an upload ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Uploaded {
    /// Written to the inbox and indexed.
    Created(ServerVersion),
    /// The same upload was already accepted; nothing was written. The version is the jotting's
    /// current one, which may be a later edit or a tombstone.
    Retried(ServerVersion),
}

#[derive(Debug, thiserror::Error)]
pub enum JottingsError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error("the id already belongs to another jotting")]
    Conflict,
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("jotting file unavailable: {0}")]
    File(#[from] std::io::Error),
}

impl Jottings {
    /// Opens the repository holding `folder`. The index is not touched until [`Self::refresh`].
    pub fn open(folder: &Path, database: Database, timezone: Tz) -> Result<Self, OpenError> {
        Ok(Self {
            repo: Arc::new(Repo::open(folder)?),
            database,
            timezone,
            progress: tokio::sync::Mutex::default(),
            generation: tokio::sync::watch::Sender::new(0),
        })
    }

    /// The jottings folder, canonicalized.
    pub fn folder(&self) -> &Path {
        self.repo.folder()
    }

    /// Notifies after every reconciliation of a new HEAD, whether indexed or refused.
    pub fn reconciliations(&self) -> tokio::sync::watch::Receiver<u64> {
        self.generation.subscribe()
    }

    /// Directories whose entries change when HEAD moves; see [`crate::watch`].
    pub(crate) fn ref_dirs(&self) -> (PathBuf, PathBuf, PathBuf) {
        self.repo.ref_dirs()
    }

    /// Reconciles the index with HEAD if HEAD moved since it was last looked at. Reading HEAD is
    /// the only work when nothing was committed.
    pub async fn refresh(&self) -> Result<Reconciled, JottingsError> {
        let mut progress = self.progress.lock().await;
        let repo = self.repo.clone();
        let head = blocking(move || repo.head()).await??;
        if progress.seen == Some(head) {
            return Ok(Reconciled::Unchanged);
        }
        let outcome = self.reconcile(head).await?;
        progress.seen = Some(head);
        match &outcome {
            Reconciled::Indexed { commit, writes } => {
                progress.indexed = Some(*commit);
                palace_info!(message = "Jottings indexed", commit = %commit, writes = *writes);
            }
            Reconciled::Refused { commit, problems } => {
                let problems: Vec<String> = problems.iter().map(ToString::to_string).collect();
                palace_error!(
                    message = "Jotting commit refused; the index stays at the previous commit",
                    commit = %commit,
                    indexed = ?progress.indexed.map(|commit| commit.to_string()),
                    problems = ?problems,
                );
            }
            Reconciled::Unchanged => {}
        }
        self.generation.send_modify(|generation| *generation += 1);
        Ok(outcome)
    }

    /// Makes the index match `commit` in one locked transaction, reading only blobs the index
    /// does not already hold.
    async fn reconcile(&self, commit: ObjectId) -> Result<Reconciled, JottingsError> {
        let repo = self.repo.clone();
        let files = blocking(move || repo.files(commit)).await??;
        let mut tx = self.database.begin_jottings().await?;
        let index = tx.index().await?;
        let unknown = unknown_blobs(&index, &files);
        let repo = self.repo.clone();
        let parsed = blocking(move || {
            unknown
                .into_iter()
                .map(|oid| Ok((oid, Jotting::parse(&repo.read(oid)?))))
                .collect::<Result<HashMap<_, _>, GitError>>()
        })
        .await??;
        let folder = self.folder().to_owned();
        let steps = match plan(&index, &files, &parsed, |path| {
            folder_file(&folder, path).is_file()
        }) {
            Ok(steps) => steps,
            Err(problems) => return Ok(Reconciled::Refused { commit, problems }),
        };
        for step in &steps {
            match step {
                Step::Insert(jotting, location) => {
                    tx.insert_committed(jotting, location).await?;
                }
                Step::Update(jotting, location) => {
                    tx.update_committed(jotting, location).await?;
                }
                Step::Relocate(id, location) => tx.relocate(*id, location).await?,
                Step::Delete(id) => {
                    tx.delete(*id).await?;
                }
            }
        }
        tx.commit().await?;
        Ok(Reconciled::Indexed {
            commit,
            writes: steps.len(),
        })
    }

    /// Accepts a jotting created offline: writes `inbox/<id>.md` and indexes it as pending, or
    /// recognizes a retry of the same upload. The server never commits; the agent does.
    pub async fn upload(&self, jotting: Jotting) -> Result<Uploaded, JottingsError> {
        let id = jotting.id;
        let result = self.write_upload(jotting).await;
        // Clients only learn that the upload can be retried; the cause stays in the log.
        if let Err(error @ (JottingsError::Git(_) | JottingsError::File(_))) = &result {
            palace_error!(message = "Jotting upload failed", id = %id, error = %error);
        }
        result
    }

    /// Validates, writes and indexes one upload under the publication lock.
    async fn write_upload(&self, jotting: Jotting) -> Result<Uploaded, JottingsError> {
        let jotting = jotting.upload()?;
        let offset = self
            .timezone
            .timestamp_millis_opt(jotting.created_at)
            .single()
            .map(|time| time.offset().fix().local_minus_utc())
            .and_then(|seconds| UtcOffset::from_whole_seconds(seconds).ok())
            .unwrap_or(UtcOffset::UTC);
        let file = jotting.render(offset);
        let location = JottingLocation {
            path: format!("{INBOX}/{}.md", jotting.id),
            blob_oid: self.repo.blob_oid(file.as_bytes())?.as_bytes().to_vec(),
        };
        let mut tx = self.database.begin_jottings().await?;
        match tx.existing(jotting.id).await? {
            Some(existing) if existing.upload_digest == Some(jotting.upload_digest()) => {
                return Ok(Uploaded::Retried(existing.version));
            }
            Some(_) => return Err(JottingsError::Conflict),
            None => {}
        }
        // The file is written before the row, so a row always has its file; a failure between
        // the two leaves only the file, which a retry finds identical and keeps.
        let target = folder_file(self.folder(), &location.path);
        let written = blocking(move || write_new(&target, file.as_bytes())).await??;
        if !written {
            return Err(JottingsError::Conflict);
        }
        let version = tx.insert_upload(&jotting, &location).await?;
        tx.commit().await?;
        Ok(Uploaded::Created(version))
    }

    /// One page of the sync feed after `since`.
    pub async fn changes(
        &self,
        since: ServerVersion,
        limit: u32,
    ) -> Result<JottingChanges, JottingsError> {
        Ok(self.database.jotting_changes(since, limit).await?)
    }
}

/// Maps a `/`-separated path relative to the folder onto the file system.
fn folder_file(folder: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(folder.to_owned(), |file, part| file.join(part))
}

/// Writes `bytes` to `target` through a temporary file and rename, so the agent never sees half
/// a file. Returns `false`, writing nothing, when a different file is already there: an upload
/// never overwrites what the agent wrote.
fn write_new(target: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    match std::fs::read(target) {
        Ok(existing) => return Ok(existing == bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let directory = target
        .parent()
        .ok_or_else(|| std::io::Error::other("upload target has no directory"))?;
    std::fs::create_dir_all(directory)?;
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // A dot file without the .md extension is never a jotting, even if it is left behind.
    let temporary = directory.join(format!(".{name}.tmp"));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, target)?;
    Ok(true)
}

/// Runs local Git and file work off the async workers.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, JottingsError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| JottingsError::File(std::io::Error::other(error)))
}
