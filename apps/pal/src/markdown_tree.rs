//! The `.md` files below a folder, as local commands that read a Markdown repository see them.
use crate::failure::Failure;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Every `.md` file below `folder`, `.git` excluded, as `/`-joined relative paths beside their
/// filesystem paths. Sorted by relative path, the order the server lists a commit's files in, so
/// output and problem reports do not depend on the platform's directory order.
pub(crate) fn markdown_files(folder: &Path) -> Result<Vec<(String, PathBuf)>, Failure> {
    let mut files = Vec::new();
    let mut pending = vec![(folder.to_owned(), String::new())];
    while let Some((directory, relative)) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(unreadable(&directory))? {
            let entry = entry.map_err(unreadable(&directory))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let child = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            let kind = entry.file_type().map_err(unreadable(&path))?;
            if kind.is_dir() && name != ".git" {
                pending.push((path, child));
            } else if kind.is_file() && name.ends_with(".md") {
                files.push((child, path));
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Turns an I/O error into the failure that names the path it happened on.
pub(crate) fn unreadable(path: &Path) -> impl FnOnce(std::io::Error) -> Failure + use<> {
    let path = path.to_owned();
    move |source| Failure::File { path, source }
}
