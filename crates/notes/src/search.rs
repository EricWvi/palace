use crate::{NoteSearch, cache::note_id};
use palace_logging::palace_warn;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

/// Finds the notes whose file contains a keyword by running ripgrep over the folder.
///
/// ripgrep reads `.gitignore` itself, so the container needs no git, but it applies those files
/// only inside a repository unless told otherwise; the mounted folder is rarely a repository's
/// root. `*.md` is chosen as a file type because a `--glob` would override the ignore files.
/// The cache script enumerates notes with the same rules.
#[derive(Clone, Debug)]
pub struct Ripgrep {
    program: PathBuf,
    timeout: Duration,
}

impl Default for Ripgrep {
    /// The `rg` on `PATH`, given ten seconds per search.
    fn default() -> Self {
        Self::new("rg".into(), Duration::from_secs(/*secs*/ 10))
    }
}

/// The search could not finish; a partial result is never passed off as a complete one.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SearchError {
    #[error("ripgrep could not start: {0}")]
    Start(std::io::Error),
    #[error("ripgrep timed out")]
    Timeout,
    #[error("ripgrep failed with {status}: {stderr}")]
    Failed {
        status: std::process::ExitStatus,
        stderr: String,
    },
}

impl Ripgrep {
    pub fn new(program: PathBuf, timeout: Duration) -> Self {
        Self { program, timeout }
    }

    /// Returns the ids of notes whose file contains the keyword, as a case-insensitive literal.
    pub(crate) async fn matching_notes(
        &self,
        folder: &Path,
        search: &NoteSearch,
    ) -> Result<HashSet<String>, SearchError> {
        let mut command = tokio::process::Command::new(&self.program);
        // Arguments go to the process directly, never through a shell; `-e` keeps a keyword that
        // starts with `-` from being read as an option, and `--no-config` keeps a personal
        // ripgrep configuration from changing which files count.
        command
            .args([
                "--no-config",
                "--files-with-matches",
                "--null",
                "--fixed-strings",
                "--ignore-case",
                "--no-require-git",
                "--type-add",
                "note:*.md",
                "--type",
                "note",
                "-e",
            ])
            .arg(search.as_str())
            .arg(".")
            .current_dir(folder)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = tokio::time::timeout(self.timeout, command.output())
            .await
            .map_err(|_| SearchError::Timeout)?
            .map_err(SearchError::Start)?;
        // ripgrep exits with 1 when nothing matched; anything else besides 0 is an error, even
        // when some files were searched, because the result would be silently incomplete.
        match output.status.code() {
            Some(0) => Ok(output
                .stdout
                .split(|&byte| byte == 0)
                .filter(|file| !file.is_empty())
                .filter_map(|file| {
                    // A name that is not UTF-8 cannot be an id in the cache, so it is never a note.
                    let id = std::str::from_utf8(file).ok().map(Path::new).and_then(note_id);
                    if id.is_none() {
                        palace_warn!(message = "Search matched a file that cannot be a note", file = %String::from_utf8_lossy(file));
                    }
                    id
                })
                .collect()),
            Some(1) => Ok(HashSet::new()),
            _ => Err(SearchError::Failed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }),
        }
    }
}
