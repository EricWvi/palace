use super::*;
use pretty_assertions::assert_eq;

fn oid(byte: u8) -> ObjectId {
    ObjectId::from_bytes_or_panic(&[byte; 20])
}

fn id(last: u8) -> Uuid {
    Uuid::from_bytes([
        1, 2, 3, 4, 5, 6, 0x70, 8, 0x80, 10, 11, 12, 13, 14, 15, last,
    ])
}

fn file(path: &str, byte: u8) -> TreeFile {
    TreeFile {
        path: path.into(),
        oid: oid(byte),
    }
}

fn location(path: &str, byte: u8) -> JottingLocation {
    JottingLocation {
        path: path.into(),
        blob_oid: oid(byte).as_bytes().to_vec(),
    }
}

fn jotting(last: u8, body: &str) -> Jotting {
    Jotting {
        id: id(last),
        created_at: 1_792_369_800_000,
        title: None,
        tags: vec![],
        body: body.into(),
    }
}

fn committed(last: u8, path: &str, byte: u8) -> IndexedJotting {
    IndexedJotting::Committed {
        id: id(last),
        location: location(path, byte),
    }
}

/// Moving a file, or committing an upload as written, relocates without reading or publishing;
/// unchanged files produce no step at all.
/// Core test cases:
/// - `specs/test-cases/server/jotting/jotting-sync.md#moving-a-jotting-file-must-keep-its-identity`
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-uploaded-jotting-must-survive-until-its-commit`
#[test]
fn known_blobs_are_relocated_without_parsing() {
    let index = [
        committed(1, "a.md", 1),
        committed(2, "inbox/b.md", 2),
        IndexedJotting::Pending {
            id: id(3),
            location: location("inbox/c.md", 3),
        },
    ];
    let files = [file("a.md", 1), file("2026/b.md", 2), file("inbox/c.md", 3)];
    assert_eq!(unknown_blobs(&index, &files), Vec::<ObjectId>::new());
    assert_eq!(
        plan(&index, &files, &HashMap::new(), |_| true),
        Ok(vec![
            Step::Relocate(id(2), location("2026/b.md", 2)),
            Step::Relocate(id(3), location("inbox/c.md", 3)),
        ])
    );
}

/// New bytes are parsed and become an insert, an update of a live jotting or the revival of a
/// tombstone; committed jottings missing from the commit are deleted.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#moving-a-jotting-file-must-keep-its-identity`
#[test]
fn new_blobs_insert_update_or_revive() {
    let index = [
        committed(1, "a.md", 1),
        IndexedJotting::Deleted { id: id(2) },
        committed(3, "gone.md", 3),
    ];
    let files = [file("moved/a.md", 11), file("b.md", 12), file("new.md", 14)];
    assert_eq!(
        unknown_blobs(&index, &files),
        vec![oid(11), oid(12), oid(14)]
    );
    let parsed = HashMap::from([
        (oid(11), Ok(jotting(1, "edited"))),
        (oid(12), Ok(jotting(2, "back"))),
        (oid(14), Ok(jotting(4, "new"))),
    ]);
    assert_eq!(
        plan(&index, &files, &parsed, |_| true),
        Ok(vec![
            Step::Update(jotting(1, "edited"), location("moved/a.md", 11)),
            Step::Update(jotting(2, "back"), location("b.md", 12)),
            Step::Insert(jotting(4, "new"), location("new.md", 14)),
            Step::Delete(id(3)),
        ])
    );
}

/// A pending upload missing from the commit survives while its inbox file exists and becomes a
/// tombstone once the file is gone.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-uploaded-jotting-must-survive-until-its-commit`
#[test]
fn pending_uploads_wait_for_their_commit() {
    let index = [IndexedJotting::Pending {
        id: id(5),
        location: location("inbox/u.md", 5),
    }];
    assert_eq!(plan(&index, &[], &HashMap::new(), |_| true), Ok(vec![]));
    assert_eq!(
        plan(&index, &[], &HashMap::new(), |path| path != "inbox/u.md"),
        Ok(vec![Step::Delete(id(5))])
    );
}

/// One bad file or reused id refuses the whole commit, reporting every problem.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-invalid-commit-must-leave-the-index-untouched`
#[test]
fn any_problem_refuses_the_whole_commit() {
    let index = [committed(1, "a.md", 1)];
    let files = [
        file("a.md", 1),
        file("copy.md", 1),
        file("bad.md", 2),
        file("fine.md", 3),
    ];
    let parsed = HashMap::from([
        (oid(2), Err(JottingError::Missing("id"))),
        (oid(3), Ok(jotting(9, "fine"))),
    ]);
    assert_eq!(
        plan(&index, &files, &parsed, |_| true),
        Err(vec![
            JottingProblem::Duplicate {
                path: "copy.md".into(),
                id: id(1),
                first: "a.md".into(),
            },
            JottingProblem::Invalid {
                path: "bad.md".into(),
                error: JottingError::Missing("id"),
            },
        ])
    );
}
