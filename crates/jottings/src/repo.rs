//! Read-only access to the Git repository holding the jottings folder. Nothing here writes to
//! `.git`; the server learns about the agent's work only through commits.
use gix::{ObjectId, ThreadSafeRepository, bstr::ByteSlice, objs::tree::EntryKind};
use std::path::{Path, PathBuf};

/// Why the configured folder cannot serve jottings. Each one stops startup: a misconfigured
/// folder that still accepted uploads would write files no commit would ever index.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("{folder} is not readable: {source}")]
    Folder {
        folder: PathBuf,
        source: std::io::Error,
    },
    #[error("{folder} is not inside a Git work tree: {reason}")]
    NotInRepository { folder: PathBuf, reason: String },
    #[error("HEAD of {0} is detached; check out a branch")]
    Detached(PathBuf),
    #[error("HEAD of {folder} does not resolve to a commit: {reason}")]
    NoCommit { folder: PathBuf, reason: String },
}

/// A failure reading Git objects after the repository opened.
#[derive(Debug, thiserror::Error)]
#[error("git: {0}")]
pub struct GitError(String);

/// One Markdown file of a commit below the jottings folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TreeFile {
    /// Relative to the jottings folder, separated by `/`.
    pub(crate) path: String,
    pub(crate) oid: ObjectId,
}

/// The repository and where the jottings folder sits in it.
pub(crate) struct Repo {
    repository: ThreadSafeRepository,
    folder: PathBuf,
    /// The folder's path components relative to the work tree root.
    prefix: Vec<String>,
}

impl Repo {
    /// Opens the repository containing `folder`. Ownership checks are skipped: the folder is the
    /// one the deployment configured, and a container user rarely owns a bind-mounted repository.
    pub(crate) fn open(folder: &Path) -> Result<Self, OpenError> {
        let folder = folder.canonicalize().map_err(|source| OpenError::Folder {
            folder: folder.to_owned(),
            source,
        })?;
        let not_in_repository = |reason: String| OpenError::NotInRepository {
            folder: folder.clone(),
            reason,
        };
        let discover = gix::discover::upwards::Options {
            trust: gix::discover::upwards::TrustPolicy::Assume(gix::sec::Trust::Full),
            ..Default::default()
        };
        let repository = gix::discover_opts(&folder, discover, gix::open::Options::isolated())
            .map_err(|error| not_in_repository(error.to_string()))?;
        let workdir = repository
            .workdir()
            .ok_or_else(|| not_in_repository("the repository is bare".into()))?
            .canonicalize()
            .map_err(|error| not_in_repository(error.to_string()))?;
        let prefix = folder
            .strip_prefix(&workdir)
            .map_err(|_| not_in_repository("the folder lies outside the work tree".into()))?
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        match repository.head_name() {
            Ok(Some(_)) => {}
            Ok(None) => return Err(OpenError::Detached(folder)),
            Err(error) => return Err(not_in_repository(error.to_string())),
        }
        let repo = Self {
            repository: repository.into_sync(),
            folder,
            prefix,
        };
        repo.head()
            .map_err(|GitError(reason)| OpenError::NoCommit {
                folder: repo.folder.clone(),
                reason,
            })?;
        Ok(repo)
    }

    /// The jottings folder, canonicalized.
    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    /// The directories whose entries change when a commit moves HEAD: the git dir (HEAD,
    /// packed-refs) and the branch refs below the common dir.
    pub(crate) fn ref_dirs(&self) -> (PathBuf, PathBuf, PathBuf) {
        let repository = self.repository.to_thread_local();
        let common = repository.common_dir();
        (
            repository.git_dir().to_owned(),
            common.to_owned(),
            common.join("refs").join("heads"),
        )
    }

    /// The commit HEAD points to now.
    pub(crate) fn head(&self) -> Result<ObjectId, GitError> {
        let repository = self.repository.to_thread_local();
        let commit = repository.head_commit().map_err(git)?;
        Ok(commit.id)
    }

    /// Every Markdown blob of `commit` below the jottings folder, in tree order. Other entries,
    /// including symlinks and submodules, are not jottings.
    pub(crate) fn files(&self, commit: ObjectId) -> Result<Vec<TreeFile>, GitError> {
        let repository = self.repository.to_thread_local();
        let mut tree = repository
            .find_commit(commit)
            .map_err(git)?
            .tree_id()
            .map_err(git)?
            .detach();
        for part in &self.prefix {
            let found = repository
                .find_tree(tree)
                .map_err(git)?
                .iter()
                .filter_map(Result::ok)
                .find(|entry| entry.mode().is_tree() && entry.filename() == part.as_bytes())
                .map(|entry| entry.object_id());
            match found {
                Some(subtree) => tree = subtree,
                // A commit without the folder simply has no jottings.
                None => return Ok(Vec::new()),
            }
        }
        let mut files = Vec::new();
        let mut pending = vec![(String::new(), tree)];
        while let Some((directory, tree)) = pending.pop() {
            let tree = repository.find_tree(tree).map_err(git)?;
            for entry in tree.iter() {
                let entry = entry.map_err(git)?;
                let name = entry.filename().to_str_lossy();
                let path = if directory.is_empty() {
                    name.into_owned()
                } else {
                    format!("{directory}/{name}")
                };
                match entry.mode().kind() {
                    EntryKind::Tree => pending.push((path, entry.object_id())),
                    EntryKind::Blob | EntryKind::BlobExecutable if path.ends_with(".md") => {
                        files.push(TreeFile {
                            path,
                            oid: entry.object_id(),
                        });
                    }
                    EntryKind::Blob
                    | EntryKind::BlobExecutable
                    | EntryKind::Link
                    | EntryKind::Commit => {}
                }
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// The bytes of one blob.
    pub(crate) fn read(&self, oid: ObjectId) -> Result<Vec<u8>, GitError> {
        let repository = self.repository.to_thread_local();
        let blob = repository.find_blob(oid).map_err(git)?;
        Ok(blob.data.clone())
    }

    /// The blob id Git gives these bytes, so an uploaded file is recognized when it is committed.
    pub(crate) fn blob_oid(&self, bytes: &[u8]) -> Result<ObjectId, GitError> {
        let kind = self.repository.to_thread_local().object_hash();
        gix::objs::compute_hash(kind, gix::objs::Kind::Blob, bytes).map_err(git)
    }
}

/// Keeps only the message of a Git error; the server logs it and never sends it to clients.
fn git(error: impl std::fmt::Display) -> GitError {
    GitError(error.to_string())
}
