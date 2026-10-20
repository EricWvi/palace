use chrono_tz::Tz;
use palace_domain::DocumentDate;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// The cache file the script writes at the root of the notes folder.
pub const CACHE_FILE: &str = ".palace-cache.json";

/// One note as the list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// Path of the Markdown file relative to the folder, without `.md`, separated by `/`.
    pub id: String,
    pub title: String,
    /// Unix epoch milliseconds of the frontmatter `date`.
    pub occurred_at: i64,
}

/// Why a cache was refused; the previous index stays in place whatever the reason.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("cache is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported cache version {0}")]
    Version(u32),
    #[error("note id {0:?} is not a relative path inside the folder")]
    Id(String),
    #[error("note {0:?} appears more than once")]
    Duplicate(String),
    #[error("note {0:?} has a blank title")]
    Title(String),
    #[error("note {id:?} has an unreadable date {date:?}")]
    Date { id: String, date: String },
}

#[derive(Deserialize)]
struct CacheFile {
    version: u32,
    notes: Vec<CacheEntry>,
}

#[derive(Deserialize)]
struct CacheEntry {
    id: String,
    title: String,
    date: String,
}

/// Every note of one cache, in list order: `occurred_at` then id, both descending.
#[derive(Debug, Default)]
pub(crate) struct NoteIndex {
    notes: Vec<Note>,
    positions: HashMap<String, usize>,
}

impl NoteIndex {
    /// Accepts a cache only when every entry is usable, so a bad cache can never replace part of
    /// the list. Dates without a time are midnight in the deployment's time zone.
    pub(crate) fn parse(bytes: &[u8], timezone: Tz) -> Result<Self, CacheError> {
        let file: CacheFile = serde_json::from_slice(bytes)?;
        if file.version != 1 {
            return Err(CacheError::Version(file.version));
        }
        let mut notes = Vec::with_capacity(file.notes.len());
        for entry in file.notes {
            if note_file(&entry.id).is_none() {
                return Err(CacheError::Id(entry.id));
            }
            if entry.title.trim().is_empty() {
                return Err(CacheError::Title(entry.id));
            }
            let Some(occurred_at) = DocumentDate::parse(&entry.date, timezone).map(|date| date.at)
            else {
                return Err(CacheError::Date {
                    id: entry.id,
                    date: entry.date,
                });
            };
            notes.push(Note {
                id: entry.id,
                title: entry.title,
                occurred_at,
            });
        }
        notes.sort_unstable_by(|a, b| {
            (b.occurred_at, b.id.as_str()).cmp(&(a.occurred_at, a.id.as_str()))
        });
        let mut positions = HashMap::with_capacity(notes.len());
        for (position, note) in notes.iter().enumerate() {
            if positions.insert(note.id.clone(), position).is_some() {
                return Err(CacheError::Duplicate(note.id.clone()));
            }
        }
        Ok(Self { notes, positions })
    }

    /// All notes in list order.
    pub(crate) fn notes(&self) -> &[Note] {
        &self.notes
    }

    pub(crate) fn get(&self, id: &str) -> Option<&Note> {
        self.positions
            .get(id)
            .map(|&position| &self.notes[position])
    }
}

/// Maps a note id to its file below the folder, or `None` when the id could name anything
/// outside it. Every id is checked when the cache loads, so reading a note can only ever open a
/// file the cache listed and that lies inside the folder.
pub(crate) fn note_file(id: &str) -> Option<PathBuf> {
    let mut file = PathBuf::new();
    for segment in id.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains(['\\', '\0'])
        {
            return None;
        }
        file.push(segment);
    }
    let name = format!("{}.md", file.file_name()?.to_str()?);
    file.set_file_name(name);
    // A segment like `C:` would still make the joined path absolute on some platforms.
    file.is_relative().then_some(file)
}

/// Maps a note file found below the folder back to its id: the reverse of `note_file`.
pub(crate) fn note_id(file: &Path) -> Option<String> {
    let file = file.strip_prefix(".").unwrap_or(file);
    let segments = file
        .iter()
        .map(|segment| segment.to_str())
        .collect::<Option<Vec<_>>>()?;
    segments.join("/").strip_suffix(".md").map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const SHANGHAI: Tz = chrono_tz::Asia::Shanghai;

    fn cache(notes: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({ "version": 1, "notes": notes })).unwrap()
    }

    /// Orders by date then id, both descending, and reads a bare day in the deployment's zone.
    /// Core test case:
    /// - `specs/test-cases/web/stars/notes.md#note-list-must-page-every-indexed-note-exactly-once-by-its-date`
    #[test]
    fn index_orders_by_date_and_reads_days_in_the_deployment_zone() {
        let index = NoteIndex::parse(
            &cache(serde_json::json!([
                { "id": "a", "title": "A", "date": "2026-10-05" },
                { "id": "c", "title": "C", "date": "2026-10-05" },
                { "id": "b/早", "title": "B", "date": "2026-10-05T00:00:00+08:00" },
                { "id": "old", "title": "Old", "date": "2026-10-04T23:59:59.5Z" },
            ])),
            SHANGHAI,
        )
        .unwrap();
        // 2026-10-05 00:00 in Shanghai is 2026-10-04 16:00 UTC.
        let midnight = 1_791_129_600_000;
        assert_eq!(
            index.notes(),
            [
                Note {
                    id: "old".into(),
                    title: "Old".into(),
                    occurred_at: midnight + 8 * 3_600_000 - 500,
                },
                Note {
                    id: "c".into(),
                    title: "C".into(),
                    occurred_at: midnight,
                },
                Note {
                    id: "b/早".into(),
                    title: "B".into(),
                    occurred_at: midnight,
                },
                Note {
                    id: "a".into(),
                    title: "A".into(),
                    occurred_at: midnight,
                },
            ]
        );
        assert_eq!(index.get("b/早").map(|note| note.title.as_str()), Some("B"));
        assert_eq!(index.get("missing"), None);
    }

    /// One unusable entry refuses the whole cache instead of loading the rest.
    /// Core test case:
    /// - `specs/test-cases/web/stars/notes.md#cache-reload-must-swap-the-whole-index-and-keep-it-when-the-cache-is-broken`
    #[test]
    fn index_refuses_a_cache_with_any_unusable_entry() {
        let refused = |notes: serde_json::Value| {
            NoteIndex::parse(&cache(notes), SHANGHAI)
                .unwrap_err()
                .to_string()
        };
        let good = serde_json::json!({ "id": "ok", "title": "好", "date": "2026-10-05" });
        assert_eq!(
            refused(
                serde_json::json!([good, { "id": "../secret", "title": "x", "date": "2026-10-05" }])
            ),
            r#"note id "../secret" is not a relative path inside the folder"#
        );
        assert_eq!(
            refused(serde_json::json!([good, { "id": "x", "title": " ", "date": "2026-10-05" }])),
            r#"note "x" has a blank title"#
        );
        assert_eq!(
            refused(serde_json::json!([good, { "id": "x", "title": "x", "date": "2026-02-30" }])),
            r#"note "x" has an unreadable date "2026-02-30""#
        );
        assert_eq!(
            refused(serde_json::json!([good, good])),
            r#"note "ok" appears more than once"#
        );
        assert_eq!(
            NoteIndex::parse(br#"{"version":2,"notes":[]}"#, SHANGHAI)
                .unwrap_err()
                .to_string(),
            "unsupported cache version 2"
        );
        assert!(NoteIndex::parse(b"{\"version\":1,", SHANGHAI).is_err());
    }

    /// Ids name files strictly below the folder, and found files map back to the same ids.
    /// Core test case:
    /// - `specs/test-cases/web/stars/notes.md#note-page-must-read-only-files-listed-in-the-index`
    #[test]
    fn ids_name_only_files_below_the_folder() {
        assert_eq!(
            note_file("rust/advisory-lock"),
            Some(Path::new("rust").join("advisory-lock.md"))
        );
        assert_eq!(
            note_file("旅行/周末"),
            Some(Path::new("旅行").join("周末.md"))
        );
        for id in [
            "",
            "/etc/passwd",
            "../x",
            "a/../../x",
            "a//b",
            "./a",
            "a/",
            "a\\..\\b",
            "a\0",
        ] {
            assert_eq!(note_file(id), None, "{id:?}");
        }
        assert_eq!(
            note_id(&Path::new(".").join("旅行").join("周末.md")),
            Some("旅行/周末".into())
        );
        assert_eq!(note_id(Path::new("a.txt")), None);
    }
}
