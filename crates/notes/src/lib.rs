//! 摘星 notes: a Markdown folder maintained outside Palace and mounted read-only.
//!
//! A script outside the server collects every note's `title` and `date` into one cache file at
//! the folder's root. The server keeps that cache in memory as the list, searches the folder
//! with ripgrep, and reads a note's file only when it is opened. Palace never writes the folder.
//! See `specs/decisions/web/stars/20261014-notes-from-mounted-folder.md`.
mod cache;
mod library;
mod page;
mod search;
mod watch;

pub use cache::{CACHE_FILE, CacheError, Note};
pub use library::{NoteContent, NoteLibrary, NotesError};
pub use page::{NOTE_PAGE_SIZE, NoteCursor, NoteListRequest, NotePage, NoteSearch};
pub use search::Ripgrep;
pub use watch::{NoteWatch, watch};
