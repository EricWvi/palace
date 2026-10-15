// Integration-test helpers should fail immediately when setup fails.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Real file system, real ripgrep and real file events: the notes library against a folder laid
//! out the way the cache script and the mount leave it.
use palace_notes::{
    CACHE_FILE, Note, NoteContent, NoteLibrary, NoteListRequest, NotePage, NoteSearch, NotesError,
    Ripgrep, watch,
};
use pretty_assertions::assert_eq;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const SHANGHAI: chrono_tz::Tz = chrono_tz::Asia::Shanghai;
// 2026-10-05 00:00 in Shanghai.
const DAY: i64 = 1_791_129_600_000;

/// Writes files below `root`, creating directories on the way.
fn write(root: &Path, files: &[(&str, &str)]) {
    for (file, text) in files {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

/// Replaces the cache the way the script does: a complete file renamed over the old one.
fn write_cache<S: AsRef<str>>(root: &Path, notes: &[(S, S, S)]) {
    let notes = notes
        .iter()
        .map(|(id, title, date)| {
            serde_json::json!({ "id": id.as_ref(), "title": title.as_ref(), "date": date.as_ref() })
        })
        .collect::<Vec<_>>();
    let temporary = root.join(".cache.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(&serde_json::json!({ "version": 1, "notes": notes })).unwrap(),
    )
    .unwrap();
    fs::rename(temporary, root.join(CACHE_FILE)).unwrap();
}

fn open(root: &Path) -> NoteLibrary {
    NoteLibrary::open(root.to_owned(), SHANGHAI, Ripgrep::default())
}

fn note(id: &str, title: &str, occurred_at: i64) -> Note {
    Note {
        id: id.into(),
        title: title.into(),
        occurred_at,
    }
}

fn search(term: &str) -> NoteListRequest {
    NoteListRequest {
        search: NoteSearch::parse(term),
        after: None,
    }
}

async fn ids(library: &NoteLibrary, request: &NoteListRequest) -> Vec<String> {
    let page = library.list(request).await.unwrap();
    page.items.into_iter().map(|note| note.id).collect()
}

/// Pages cover every note once in date order, also while the cache is replaced between pages.
/// Core test case:
/// - `specs/test-cases/web/stars/notes.md#note-list-must-page-every-indexed-note-exactly-once-by-its-date`
#[tokio::test]
async fn note_list_must_page_every_indexed_note_exactly_once_by_its_date() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    // 120 notes over 12 days, ten a day; the list order is date then id, both descending.
    let entries: Vec<(String, String, String)> = (0..120)
        .map(|n| {
            (
                format!("n{n:03}"),
                format!("笔记 {n}"),
                format!("2026-09-{:02}", 1 + n / 10),
            )
        })
        .collect();
    write_cache(root, &entries);
    let library = open(root);

    let first = library.list(&NoteListRequest::default()).await.unwrap();
    assert_eq!(first.items.len(), 50);
    assert_eq!(first.total, 120);
    // A note dated after everything arrives between two pages: later pages do not shift.
    let mut grown = entries.clone();
    grown.push(("new".into(), "新".into(), "2026-10-05".into()));
    write_cache(root, &grown);
    library.reload();

    let mut seen: Vec<Note> = first.items;
    let mut after = first.next_cursor;
    while let Some(cursor) = after {
        let page = library
            .list(&NoteListRequest {
                search: None,
                after: Some(cursor),
            })
            .await
            .unwrap();
        assert_eq!(page.total, 121);
        seen.extend(page.items);
        after = page.next_cursor;
    }
    let mut expected: Vec<Note> = (0..120)
        .map(|n| {
            note(
                &format!("n{n:03}"),
                &format!("笔记 {n}"),
                DAY - (34 - n / 10) * 86_400_000,
            )
        })
        .collect();
    expected.sort_by(|a, b| (b.occurred_at, &b.id).cmp(&(a.occurred_at, &a.id)));
    assert_eq!(seen, expected);

    // The total counts every note whatever the search.
    let page = library.list(&search("没有这个词")).await.unwrap();
    assert_eq!(
        page,
        NotePage {
            items: vec![],
            next_cursor: None,
            total: 121
        }
    );
}

/// Title and file contents match as case-insensitive literals, within the index and the
/// folder's ignore rules, outside any git repository.
/// Core test case:
/// - `specs/test-cases/web/stars/notes.md#note-search-must-match-literally-and-stay-within-the-index`
#[tokio::test]
async fn note_search_must_match_literally_and_stay_within_the_index() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    write(
        root,
        &[
            (".gitignore", "drafts/\n"),
            (
                "title.md",
                "---\ntitle: Advisory Lock\n---\n正文没有关键词\n",
            ),
            (
                "body.md",
                "---\ntitle: 正文\n---\nPostgreSQL 的 ADVISORY lock。\n",
            ),
            ("tagged.md", "---\ntitle: 标签\ntag: [轮询]\n---\n"),
            (
                "regex.md",
                "---\ntitle: 符号\n---\n写作 a.*b 和 f(x) 以及 -x 选项\n",
            ),
            ("drafts/half.md", "---\ntitle: 草稿\n---\nadvisory 草稿\n"),
            ("unlisted.md", "---\ntitle: 未入 cache\n---\nadvisory\n"),
        ],
    );
    write_cache(
        root,
        &[
            ("title", "Advisory Lock", "2026-10-05"),
            ("body", "正文", "2026-10-04"),
            ("tagged", "标签", "2026-10-03"),
            ("regex", "符号", "2026-10-02"),
            // Listed, but the folder's .gitignore hides it from ripgrep.
            ("drafts/half", "草稿", "2026-10-01"),
            // Listed, but deleted since the cache was built.
            ("gone", "已删除", "2026-09-30"),
        ],
    );
    let library = open(root);

    assert_eq!(ids(&library, &search("advisory")).await, ["title", "body"]);
    assert_eq!(ids(&library, &search("轮询")).await, ["tagged"]);
    assert_eq!(ids(&library, &search(".*")).await, ["regex"]);
    assert_eq!(ids(&library, &search("f(")).await, ["regex"]);
    assert_eq!(ids(&library, &search("-x")).await, ["regex"]);
    assert_eq!(ids(&library, &search("a.b")).await, Vec::<String>::new());
    // A title still matches after its file is gone; the list is the cache's.
    assert_eq!(ids(&library, &search("已删除")).await, ["gone"]);
}

/// A search that cannot finish fails instead of returning what it found so far.
/// Core test case:
/// - `specs/test-cases/web/stars/notes.md#note-search-must-match-literally-and-stay-within-the-index`
#[tokio::test]
async fn note_search_fails_whole_when_ripgrep_cannot_finish() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    write_cache(root, &[("a", "A", "2026-10-05")]);
    for search_with in [
        Ripgrep::new(root.join("no-such-rg"), Duration::from_secs(/*secs*/ 5)),
        Ripgrep::new(hang_forever(root), Duration::from_millis(/*millis*/ 100)),
    ] {
        let library = NoteLibrary::open(root.to_owned(), SHANGHAI, search_with);
        assert!(matches!(
            library.list(&search("a")).await,
            Err(NotesError::SearchFailed)
        ));
    }
}

/// A program that never exits: it reads a FIFO nobody writes to.
fn hang_forever(root: &Path) -> PathBuf {
    let fifo = root.join("never");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(status.success());
    let program = root.join("hang");
    fs::write(
        &program,
        format!("#!/bin/sh\nexec cat {}\n", fifo.display()),
    )
    .unwrap();
    fs::set_permissions(
        &program,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    program
}

/// A renamed cache is picked up by file events; without them, by the poll's stamp check; a
/// broken one leaves the index alone; no cache or no folder starts empty.
/// Core test case:
/// - `specs/test-cases/web/stars/notes.md#cache-reload-must-swap-the-whole-index-and-keep-it-when-the-cache-is-broken`
#[tokio::test]
async fn cache_reload_must_swap_the_whole_index_and_keep_it_when_the_cache_is_broken() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    let library = Arc::new(open(root));
    assert_eq!(
        library
            .list(&NoteListRequest::default())
            .await
            .unwrap()
            .total,
        0
    );
    let listed = || async { ids(&library, &NoteListRequest::default()).await };

    // File events: wait for the index to change, never for a fixed time.
    let _watch = watch(library.clone(), Duration::from_secs(/*secs*/ 3600))
        .unwrap()
        .unwrap();
    let mut changes = library.changes();
    write_cache(root, &[("a", "A", "2026-10-05")]);
    tokio::time::timeout(Duration::from_secs(/*secs*/ 10), changes.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(listed().await, ["a"]);

    // Broken, unknown and missing caches keep the notes that worked.
    for broken in [&b"{\"version\":1,"[..], br#"{"version":9,"notes":[]}"#] {
        fs::write(root.join(CACHE_FILE), broken).unwrap();
        library.reload();
        assert_eq!(listed().await, ["a"]);
    }
    fs::remove_file(root.join(CACHE_FILE)).unwrap();
    library.reload();
    assert_eq!(listed().await, ["a"]);

    // The poll alone: a library nobody watches reloads when the stamp moved, and only then.
    let polled = open(root);
    write_cache(root, &[("b", "B", "2026-10-05")]);
    let mut polled_changes = polled.changes();
    polled.refresh();
    assert!(polled_changes.has_changed().unwrap());
    polled_changes.borrow_and_update();
    polled.refresh();
    assert!(!polled_changes.has_changed().unwrap());
    assert_eq!(ids(&polled, &NoteListRequest::default()).await, ["b"]);

    let absent = NoteLibrary::absent();
    assert_eq!(
        absent.list(&search("a")).await.unwrap(),
        NotePage {
            items: vec![],
            next_cursor: None,
            total: 0
        }
    );
    assert!(
        watch(Arc::new(absent), Duration::from_secs(/*secs*/ 1))
            .unwrap()
            .is_none()
    );
}

/// Opening reads only indexed files, strips the frontmatter and shows the latest body.
/// Core test case:
/// - `specs/test-cases/web/stars/notes.md#note-page-must-read-only-files-listed-in-the-index`
#[tokio::test]
async fn note_page_must_read_only_files_listed_in_the_index() {
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "secret").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    write(
        root,
        &[
            ("旅行/周末.md", "---\ntitle: 周末\n---\n\n# 计划\n"),
            ("unlisted.md", "---\ntitle: 未入 cache\n---\n"),
        ],
    );
    write_cache(
        root,
        &[
            ("旅行/周末", "周末", "2026-10-05"),
            ("gone", "已删除", "2026-10-04"),
        ],
    );
    let library = open(root);

    assert_eq!(
        library.read("旅行/周末").await.unwrap(),
        NoteContent {
            note: note("旅行/周末", "周末", DAY),
            content: "# 计划\n".into(),
        }
    );
    fs::write(
        root.join("旅行").join("周末.md"),
        "---\ntitle: 改过\n---\n新正文\n",
    )
    .unwrap();
    assert_eq!(
        library.read("旅行/周末").await.unwrap(),
        NoteContent {
            note: note("旅行/周末", "周末", DAY),
            content: "新正文\n".into(),
        }
    );
    let relative = Path::new("..")
        .join(outside.path().file_name().unwrap())
        .join("secret");
    for id in [
        relative.to_str().unwrap(),
        &outside.path().join("secret").display().to_string(),
        "unlisted",
        "gone",
        "",
    ] {
        assert!(
            matches!(library.read(id).await, Err(NotesError::NotFound)),
            "{id:?}"
        );
    }
    assert!(matches!(
        NoteLibrary::absent().read("旅行/周末").await,
        Err(NotesError::NotFound)
    ));
}

/// Every note feature works on a folder the server cannot write to, as when mounted read-only.
/// Core test case:
/// - `specs/test-cases/web/stars/notes.md#notes-must-be-read-only-and-readable-by-every-signed-in-user`
#[tokio::test]
async fn notes_work_on_a_folder_the_server_cannot_write() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    write(root, &[("a/b.md", "---\ntitle: B\n---\n只读\n")]);
    write_cache(root, &[("a/b", "B", "2026-10-05")]);
    let mode =
        |path: &Path, mode| fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    for (path, locked) in [
        (root.join("a").join("b.md"), 0o444),
        (root.join(CACHE_FILE), 0o444),
        (root.join("a"), 0o555),
        (root.to_owned(), 0o555),
    ] {
        mode(&path, locked);
    }
    assert!(fs::write(root.join("probe"), "").is_err());

    let library = Arc::new(open(root));
    let _watch = watch(library.clone(), Duration::from_secs(/*secs*/ 3600))
        .unwrap()
        .unwrap();
    assert_eq!(ids(&library, &NoteListRequest::default()).await, ["a/b"]);
    assert_eq!(ids(&library, &search("只读")).await, ["a/b"]);
    assert_eq!(library.read("a/b").await.unwrap().content, "只读\n");

    // Let the temporary directory clean itself up.
    mode(root, 0o755);
    mode(&root.join("a"), 0o755);
}
