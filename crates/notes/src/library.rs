use crate::{
    CACHE_FILE, NOTE_PAGE_SIZE, Note, NoteListRequest, NotePage, Ripgrep,
    cache::{NoteIndex, note_file},
    search::SearchError,
};
use chrono_tz::Tz;
use palace_domain::InputError;
use palace_logging::{palace_error, palace_info, palace_warn};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError, RwLock},
    time::SystemTime,
};
use tokio::sync::watch;

/// The notes a deployment serves to every signed-in user. Notes belong to the deployment, not
/// to an owner, so nothing here takes an owner scope.
pub struct NoteLibrary {
    shelf: Shelf,
    index: RwLock<Arc<NoteIndex>>,
    // Counts index replacements, so a watcher's effect can be awaited instead of slept on.
    generation: watch::Sender<u64>,
}

/// Whether a notes folder is configured; without one the list is simply empty.
enum Shelf {
    Absent,
    Folder {
        root: PathBuf,
        timezone: Tz,
        search: Ripgrep,
        // The cache file last loaded, to tell whether a poll has anything new to read.
        loaded: Mutex<Option<CacheStamp>>,
    },
}

/// Identifies one version of the cache file; a rename gives a new inode even within one mtime tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CacheStamp {
    modified: SystemTime,
    len: u64,
    inode: u64,
}

/// A note opened for reading.
#[derive(Debug, PartialEq, Eq)]
pub struct NoteContent {
    pub note: Note,
    /// The Markdown body without its frontmatter.
    pub content: String,
}

#[derive(Debug, thiserror::Error)]
pub enum NotesError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error("note not found")]
    NotFound,
    #[error("search failed")]
    SearchFailed,
}

impl NoteLibrary {
    /// A deployment without notes: an empty list that never touches the file system.
    pub fn absent() -> Self {
        Self::with_shelf(Shelf::Absent)
    }

    /// Serves the notes folder at `root`, loading its cache now. A missing or broken cache starts
    /// an empty list rather than stopping the server; the watcher picks up the cache later.
    pub fn open(root: PathBuf, timezone: Tz, search: Ripgrep) -> Self {
        let library = Self::with_shelf(Shelf::Folder {
            root,
            timezone,
            search,
            loaded: Mutex::new(None),
        });
        library.reload();
        library
    }

    fn with_shelf(shelf: Shelf) -> Self {
        Self {
            shelf,
            index: RwLock::default(),
            generation: watch::Sender::new(0),
        }
    }

    /// The folder being served, if any.
    pub fn root(&self) -> Option<&Path> {
        match &self.shelf {
            Shelf::Absent => None,
            Shelf::Folder { root, .. } => Some(root),
        }
    }

    /// Notifies after every replacement of the index.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }

    /// Reads the cache and replaces the whole index with it. Any failure keeps the index as it
    /// was: a cache being fixed must not empty or half-fill a list that worked.
    pub fn reload(&self) {
        let Shelf::Folder {
            root,
            timezone,
            loaded,
            ..
        } = &self.shelf
        else {
            return;
        };
        let file = root.join(CACHE_FILE);
        // Stamp before reading: if the file is replaced in between, the next poll sees a stamp
        // that differs from the one recorded here and reads again.
        let stamp = cache_stamp(&file);
        let bytes = match std::fs::read(&file) {
            Ok(bytes) => bytes,
            Err(error) => {
                palace_warn!(message = "Notes cache unreadable; keeping the current notes", file = %file.display(), error = %error);
                *loaded.lock().unwrap_or_else(PoisonError::into_inner) = stamp;
                return;
            }
        };
        match NoteIndex::parse(&bytes, *timezone) {
            Ok(index) => {
                palace_info!(message = "Notes cache loaded", notes = index.notes().len());
                *self.index.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(index);
                self.generation.send_modify(|generation| *generation += 1);
            }
            Err(error) => {
                palace_error!(message = "Notes cache refused; keeping the current notes", file = %file.display(), error = %error);
            }
        }
        *loaded.lock().unwrap_or_else(PoisonError::into_inner) = stamp;
    }

    /// Reloads only when the cache file differs from the one last read: the fallback for
    /// mounts whose file events never arrive.
    pub fn refresh(&self) {
        let Shelf::Folder { root, loaded, .. } = &self.shelf else {
            return;
        };
        let current = cache_stamp(&root.join(CACHE_FILE));
        let changed = *loaded.lock().unwrap_or_else(PoisonError::into_inner) != current;
        if changed {
            self.reload();
        }
    }

    fn snapshot(&self) -> Arc<NoteIndex> {
        self.index
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Lists one page of notes, latest date first, optionally limited to notes whose title or
    /// file contains the search term. Results never reach beyond the index: a file not yet in
    /// the cache, or deleted since, does not appear.
    pub async fn list(&self, request: &NoteListRequest) -> Result<NotePage, NotesError> {
        let index = self.snapshot();
        let matches = match (&request.search, &self.shelf) {
            (None, _) => None,
            // With nothing indexed, nothing can match; there is no folder to search either.
            (Some(_), _) if index.notes().is_empty() => Some(Default::default()),
            (Some(_), Shelf::Absent) => Some(Default::default()),
            (
                Some(search),
                Shelf::Folder {
                    root,
                    search: ripgrep,
                    ..
                },
            ) => Some(ripgrep.matching_notes(root, search).await.map_err(
                |error: SearchError| {
                    palace_error!(message = "Notes search failed", error = %error);
                    NotesError::SearchFailed
                },
            )?),
        };
        let needle = request
            .search
            .as_ref()
            .map(|search| search.as_str().to_lowercase());
        let mut rows = index
            .notes()
            .iter()
            .filter(|note| {
                request
                    .after
                    .as_ref()
                    .is_none_or(|after| after.precedes(note))
            })
            .filter(|note| match (&needle, &matches) {
                (Some(needle), Some(matches)) => {
                    note.title.to_lowercase().contains(needle) || matches.contains(&note.id)
                }
                (None, _) | (_, None) => true,
            });
        let items: Vec<Note> = rows.by_ref().take(NOTE_PAGE_SIZE).cloned().collect();
        let next_cursor = match (rows.next(), items.last()) {
            (Some(_), Some(last)) => Some(crate::NoteCursor {
                occurred_at: last.occurred_at,
                id: last.id.clone(),
            }),
            (None, _) | (_, None) => None,
        };
        Ok(NotePage {
            items,
            next_cursor,
            total: index.notes().len(),
        })
    }

    /// Opens a note listed in the index. The file is read on every request, so an edited body
    /// shows at once; title and date stay those of the cache until it is rebuilt.
    pub async fn read(&self, id: &str) -> Result<NoteContent, NotesError> {
        let Shelf::Folder { root, .. } = &self.shelf else {
            return Err(NotesError::NotFound);
        };
        // Only ids from the index are opened, and every one was checked to stay inside the
        // folder when the cache loaded; the request never names a path itself.
        let index = self.snapshot();
        let note = index.get(id).ok_or(NotesError::NotFound)?;
        let file = note_file(&note.id)
            .map(|file| root.join(file))
            .ok_or(NotesError::NotFound)?;
        let text = tokio::fs::read_to_string(&file).await.map_err(|error| {
            palace_warn!(message = "Indexed note unreadable", file = %file.display(), error = %error);
            NotesError::NotFound
        })?;
        Ok(NoteContent {
            note: note.clone(),
            content: body(&text).to_owned(),
        })
    }
}

/// Identifies the cache file now present, or `None` when there is none.
fn cache_stamp(file: &Path) -> Option<CacheStamp> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(file).ok()?;
    Some(CacheStamp {
        modified: metadata.modified().ok()?,
        len: metadata.len(),
        inode: metadata.ino(),
    })
}

/// The Markdown after the leading `---` frontmatter block, by the rules the cache script reads it
/// with; text without frontmatter is returned whole.
fn body(text: &str) -> &str {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(|line| line.trim_end_matches(['\r', '\n'])) != Some("---") {
        return text;
    }
    let mut offset = text.find('\n').map_or(text.len(), |at| at + 1);
    for line in lines {
        offset += line.len();
        if matches!(line.trim_end_matches(['\r', '\n']), "---" | "...") {
            return text[offset..].trim_start_matches(['\r', '\n']);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// The body leaves out exactly the frontmatter block, whatever the line endings.
    /// Core test case:
    /// - `specs/test-cases/web/stars/notes.md#note-page-must-read-only-files-listed-in-the-index`
    #[test]
    fn body_leaves_out_the_frontmatter() {
        assert_eq!(body("---\ntitle: x\n---\n\n# 正文\n"), "# 正文\n");
        assert_eq!(body("\u{feff}---\r\ntitle: x\r\n...\r\n正文"), "正文");
        assert_eq!(body("# 没有\n---\n"), "# 没有\n---\n");
        assert_eq!(body("---\ntitle: x\n"), "---\ntitle: x\n");
        assert_eq!(body("---\n---\n正文\n---\n"), "正文\n---\n");
    }
}
