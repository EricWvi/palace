use super::*;
use pretty_assertions::assert_eq;

const ID: &str = "0199f2a4-7c1e-7b3a-9d2e-4f6a8b0c1d2e";
const OTHER: &str = "0199f2a4-7c1e-7b3a-9d2e-4f6a8b0c1d2f";

fn id(text: &str) -> Uuid {
    Uuid::parse_str(text).unwrap()
}

fn east8() -> UtcOffset {
    UtcOffset::from_hms(8, 0, 0).unwrap()
}

/// Reads every carried field and ignores keys outside the model.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-invalid-commit-must-leave-the-index-untouched`
#[test]
fn parses_a_jotting_file() {
    let file = format!(
        "\u{feff}---\nid: {ID}\ncreated: 2026-10-19T08:30:00.250+08:00\ntitle: 读书\ntags: [阅读, rust]\nmood: calm\n---\n\n正文第一行\n\n---\n第二段\n"
    );
    assert_eq!(
        Jotting::parse(file.as_bytes()).unwrap(),
        Jotting {
            id: id(ID),
            created_at: 1_792_369_800_250,
            title: Some("读书".into()),
            tags: vec!["阅读".into(), "rust".into()],
            body: "正文第一行\n\n---\n第二段\n".into(),
        }
    );
    let minimal = format!("---\r\nid: {ID}\r\ncreated: 2026-10-19T00:30:00Z\r\n...\r\n");
    assert_eq!(
        Jotting::parse(minimal.as_bytes()).unwrap(),
        Jotting {
            id: id(ID),
            created_at: 1_792_369_800_000,
            title: None,
            tags: vec![],
            body: String::new(),
        }
    );
}

/// Each malformed file names the field that is wrong.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-invalid-commit-must-leave-the-index-untouched`
#[test]
fn rejects_files_outside_the_format() {
    let created = "created: 2026-10-19T08:30:00+08:00";
    let cases = [
        ("no frontmatter\n".to_owned(), JottingError::NoFrontmatter),
        (
            format!("---\nid: {ID}\n{created}\n"),
            JottingError::NoFrontmatter,
        ),
        ("---\n- a\n---\n".to_owned(), JottingError::NotMapping),
        ("---\n---\n".to_owned(), JottingError::NotMapping),
        (
            format!("---\n{created}\n---\n"),
            JottingError::Missing("id"),
        ),
        (
            format!("---\nid: {ID}\n---\n"),
            JottingError::Missing("created"),
        ),
        (
            format!("---\nid: {}\n{created}\n---\n", ID.to_uppercase()),
            JottingError::Id,
        ),
        (
            format!(
                "---\nid: {}\n{created}\n---\n",
                id("0199f2a4-7c1e-4b3a-9d2e-4f6a8b0c1d2e")
            ),
            JottingError::Id,
        ),
        (format!("---\nid: 7\n{created}\n---\n"), JottingError::Id),
        (
            format!("---\nid: {ID}\ncreated: 2026-10-19\n---\n"),
            JottingError::Created,
        ),
        (
            format!("---\nid: {ID}\ncreated: 2026-10-19T08:30:00\n---\n"),
            JottingError::Created,
        ),
        (
            format!("---\nid: {ID}\ncreated: 2026-10-19T08:30:00.0001+08:00\n---\n"),
            JottingError::Created,
        ),
        (
            format!("---\nid: {ID}\n{created}\ntitle: ' '\n---\n"),
            JottingError::Title,
        ),
        (
            format!("---\nid: {ID}\n{created}\ntitle:\n---\n"),
            JottingError::Title,
        ),
        (
            format!("---\nid: {ID}\n{created}\ntags: a\n---\n"),
            JottingError::Tags,
        ),
        (
            format!("---\nid: {ID}\n{created}\ntags: [a, a]\n---\n"),
            JottingError::Tags,
        ),
        (
            format!("---\nid: {ID}\n{created}\ntags: [2024]\n---\n"),
            JottingError::Tags,
        ),
    ];
    for (file, error) in cases {
        assert_eq!(Jotting::parse(file.as_bytes()), Err(error), "{file}");
    }
    assert_eq!(Jotting::parse(&[0xff]), Err(JottingError::Encoding));
    assert!(matches!(
        Jotting::parse(b"---\nid: [\n---\n"),
        Err(JottingError::Yaml(_))
    ));
}

/// An accepted upload renders to a file that parses back to exactly the same jotting, so the
/// commit containing it changes nothing; the digest ignores the time zone it was rendered in.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-uploaded-jotting-must-survive-until-its-commit`
#[test]
fn uploads_render_to_files_that_parse_back() {
    let awkward = Jotting {
        id: id(ID),
        created_at: 1_792_369_800_250,
        title: Some("a: b # c 'd' \"e\" - [f]".into()),
        tags: vec!["true".into(), "- x".into(), "12".into(), "阅读".into()],
        body: "\n\r\n# 正文\n---\n结尾".into(),
    }
    .upload()
    .unwrap();
    assert_eq!(awkward.body, "# 正文\n---\n结尾");
    for jotting in [
        awkward.clone(),
        Jotting {
            id: id(ID),
            created_at: 0,
            title: None,
            tags: vec![],
            body: String::new(),
        },
    ] {
        for offset in [
            east8(),
            UtcOffset::UTC,
            UtcOffset::from_hms(-3, -30, 0).unwrap(),
        ] {
            let file = jotting.render(offset);
            assert_eq!(Jotting::parse(file.as_bytes()).unwrap(), jotting, "{file}");
        }
    }
    assert_eq!(
        awkward.render(east8()).lines().take(3).collect::<Vec<_>>(),
        [
            "---",
            &format!("id: {ID}"),
            "created: \"2026-10-19T08:30:00.25+08:00\""
        ]
    );
    let digest = awkward.upload_digest();
    let mut other = awkward.clone();
    other.tags.pop();
    assert_ne!(other.upload_digest(), digest);
    assert_eq!(awkward.upload().unwrap().upload_digest(), digest);
}

/// Uploads are checked field by field before anything is written.
#[test]
fn rejects_invalid_uploads() {
    let valid = Jotting {
        id: id(ID),
        created_at: 1_792_369_800_000,
        title: None,
        tags: vec![],
        body: "x".into(),
    };
    let reject = |jotting: Jotting| jotting.upload().unwrap_err().path;
    assert_eq!(
        reject(Jotting {
            id: id("0199f2a4-7c1e-4b3a-9d2e-4f6a8b0c1d2e"),
            ..valid.clone()
        }),
        "id"
    );
    assert_eq!(
        reject(Jotting {
            created_at: i64::MAX,
            ..valid.clone()
        }),
        "created_at"
    );
    assert_eq!(
        reject(Jotting {
            title: Some("  ".into()),
            ..valid.clone()
        }),
        "title"
    );
    assert_eq!(
        reject(Jotting {
            tags: vec!["a".into(), "a".into()],
            ..valid.clone()
        }),
        "tags"
    );
    let large = Jotting {
        body: "x".repeat(JOTTING_BODY_MAX_BYTES + 1),
        ..valid
    };
    assert_eq!(large.upload().unwrap_err().kind, InputErrorKind::Limit);
}

/// A set reports every invalid file and every reused id, naming the file that used it first.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-invalid-commit-must-leave-the-index-untouched`
#[test]
fn census_reports_every_problem_of_a_set() {
    let mut census = JottingCensus::default();
    census.record("a.md", id(ID));
    census.admit("b.md", Err(JottingError::Missing("id")));
    census.record("c/d.md", id(OTHER));
    census.record("e.md", id(ID));
    assert_eq!(
        census.finish(),
        Err(vec![
            JottingProblem::Invalid {
                path: "b.md".into(),
                error: JottingError::Missing("id"),
            },
            JottingProblem::Duplicate {
                path: "e.md".into(),
                id: id(ID),
                first: "a.md".into(),
            },
        ])
    );
    let mut census = JottingCensus::default();
    census.record("a.md", id(ID));
    census.record("b.md", id(OTHER));
    assert_eq!(census.finish(), Ok(2));
}
