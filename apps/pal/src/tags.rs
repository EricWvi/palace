//! `pal tags`: the tags a folder of Markdown files uses, read from each file's frontmatter. A
//! file without frontmatter or without tags is not tagged and contributes nothing; `draft: true`
//! does not hide a file, because a draft's tags are still in use.
use crate::{
    failure::Failure,
    markdown_tree::{markdown_files, unreadable},
};
use palace_domain::split_frontmatter;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};
use yaml_rust2::{Yaml, YamlLoader};

/// Why a file's frontmatter cannot be read for tags.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Error {
    #[error("file is not UTF-8")]
    Encoding,
    #[error("frontmatter is not valid YAML: {0}")]
    Yaml(String),
    #[error("frontmatter is not a mapping")]
    NotMapping,
    #[error("`tags` must be a list of strings")]
    Tags,
}

/// A tag and the number of Markdown cards using it, counting each card only once.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TagCount {
    pub(crate) tag: String,
    pub(crate) count: usize,
}

/// Every distinct tag with its card count, in the order it first appears when the files are read
/// in path order and each file's tags in written order.
pub(crate) fn collect(folder: &Path) -> Result<Vec<TagCount>, Failure> {
    let mut positions = HashMap::new();
    let mut tags: Vec<TagCount> = Vec::new();
    for (_, path) in markdown_files(folder)? {
        let bytes = fs::read(&path).map_err(unreadable(&path))?;
        let file_tags = read(&bytes).map_err(|error| Failure::Tags {
            path: path.clone(),
            error,
        })?;
        let mut seen_in_card = HashSet::new();
        for tag in file_tags {
            // Repeated frontmatter entries must not inflate the number of cards using a tag.
            if !seen_in_card.insert(tag.clone()) {
                continue;
            }
            let position = *positions.entry(tag.clone()).or_insert_with(|| {
                tags.push(TagCount { tag, count: 0 });
                tags.len() - 1
            });
            tags[position].count += 1;
        }
    }
    Ok(tags)
}

/// The tags of one file, empty when it has no frontmatter, an empty one, or no `tags` value.
/// Anything that is not a list of strings fails rather than being skipped, so a misspelt tag
/// list is reported instead of silently missing from the output.
fn read(bytes: &[u8]) -> Result<Vec<String>, Error> {
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Encoding)?;
    let Some((frontmatter, _)) = split_frontmatter(text) else {
        return Ok(Vec::new());
    };
    let documents =
        YamlLoader::load_from_str(frontmatter).map_err(|e| Error::Yaml(e.to_string()))?;
    let fields = match documents.into_iter().next() {
        None => return Ok(Vec::new()),
        Some(Yaml::Hash(fields)) => fields,
        Some(_) => return Err(Error::NotMapping),
    };
    match fields.get(&Yaml::String("tags".to_owned())) {
        None | Some(Yaml::Null) => Ok(Vec::new()),
        Some(Yaml::Array(items)) => items
            .iter()
            .map(|item| match item {
                Yaml::String(tag) => Some(tag.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(Error::Tags),
        Some(_) => Err(Error::Tags),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Tags are read from every level, deduplicated in first-seen order, and files without
    /// frontmatter or tags are skipped while drafts count. Duplicate entries count once per card.
    #[test]
    fn collects_distinct_tags_in_first_seen_order() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        for (path, text) in [
            ("b.md", "---\ntags: [rust, cli, rust]\n---\n"),
            ("a/z.md", "---\ntags:\n  - zeta\n  - rust\n---\nbody"),
            ("a/draft.md", "---\ndraft: true\ntags: [draft-tag]\n---\n"),
            ("a/untagged.md", "---\ntitle: x\n---\n"),
            ("a/null.md", "---\ntags:\n---\n"),
            ("a/empty.md", "---\n---\n"),
            ("readme.md", "# no frontmatter\ntags: [ignored]\n"),
            ("notes.txt", "---\ntags: [ignored]\n---\n"),
            (".git/c.md", "---\ntags: [ignored]\n---\n"),
        ] {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let expected = [("draft-tag", 1), ("zeta", 1), ("rust", 2), ("cli", 1)]
            .into_iter()
            .map(|(tag, count)| TagCount {
                tag: tag.to_owned(),
                count,
            })
            .collect::<Vec<_>>();
        assert_eq!(collect(root).unwrap(), expected);
    }

    /// A tags value that is not a list of strings names the file instead of being skipped.
    #[test]
    fn refuses_tags_that_are_not_a_list_of_strings() {
        let cases = [
            ("---\ntags: rust\n---\n", Error::Tags),
            ("---\ntags: [rust, 1]\n---\n", Error::Tags),
            ("---\n- rust\n---\n", Error::NotMapping),
        ];
        let read_back: Vec<_> = cases
            .iter()
            .map(|(text, _)| read(text.as_bytes()))
            .collect();
        let expected: Vec<_> = cases.into_iter().map(|(_, error)| Err(error)).collect();
        assert_eq!(read_back, expected);

        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("bad.md");
        fs::write(&file, "---\ntags: rust\n---\n").unwrap();
        match collect(folder.path()) {
            Err(Failure::Tags { path, error }) => assert_eq!((path, error), (file, Error::Tags)),
            other => panic!("expected a tags failure, got {other:?}"),
        }
    }
}
