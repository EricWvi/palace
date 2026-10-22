//! `pal jottings check`: the server's verdict on a jottings folder before it is committed. It
//! judges the work tree with the same parser and census the server applies to a commit, so a
//! folder that passes is a commit the server indexes.
use crate::{
    failure::Failure,
    markdown_tree::{markdown_files, unreadable},
};
use palace_domain::{Jotting, JottingCensus};
use std::{fs, path::Path};

/// Checks every `.md` file below `folder`, `.git` excluded, and returns how many jottings it
/// holds, or every problem that would make the server refuse the commit.
pub(crate) fn check(folder: &Path) -> Result<usize, Failure> {
    let mut census = JottingCensus::default();
    // The files come sorted by path like a commit's; judging them in the same order names the
    // same file as the first holder of a reused id.
    for (relative, path) in markdown_files(folder)? {
        let bytes = fs::read(&path).map_err(unreadable(&path))?;
        census.admit(&relative, Jotting::parse(&bytes));
    }
    census.finish().map_err(Failure::Jottings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use palace_domain::{JottingError, JottingProblem};
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    /// The work tree is judged like a commit: every problem in path order, `.git` skipped,
    /// non-Markdown files ignored.
    /// Core test case:
    /// - `specs/test-cases/server/jotting/jotting-sync.md#an-invalid-commit-must-leave-the-index-untouched`
    #[test]
    fn reports_what_the_server_would_refuse() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        let id = Uuid::now_v7();
        let jotting = format!("---\nid: {id}\ncreated: 2026-10-19T08:30:00+08:00\n---\n");
        for (path, text) in [
            ("a/x.md", jotting.as_str()),
            ("a.md", jotting.as_str()),
            ("b.md", "no frontmatter"),
            ("notes.txt", "ignored"),
            (".git/c.md", "ignored"),
        ] {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        match check(root) {
            Err(Failure::Jottings(problems)) => assert_eq!(
                problems,
                [
                    JottingProblem::Duplicate {
                        path: "a/x.md".into(),
                        id,
                        first: "a.md".into(),
                    },
                    JottingProblem::Invalid {
                        path: "b.md".into(),
                        error: JottingError::NoFrontmatter,
                    },
                ]
            ),
            other => panic!("expected problems, got {other:?}"),
        }
        fs::remove_file(root.join("a").join("x.md")).unwrap();
        fs::remove_file(root.join("b.md")).unwrap();
        assert_eq!(check(root).unwrap(), 1);
    }
}
