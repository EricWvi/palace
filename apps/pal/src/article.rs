//! A self-contained article directory becomes one upload without writing back to its source.
mod markdown;

use chrono::{DateTime, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    fs::File,
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
#[error("invalid article at {}: {message}", path.display())]
pub(crate) struct Error {
    path: PathBuf,
    message: String,
}

impl Error {
    /// Retains the source location so every local rejection points at something fixable.
    fn at(path: &Path, message: impl ToString) -> Self {
        Self {
            path: path.to_owned(),
            message: message.to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    title: String,
    url: String,
    date: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    author: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Fields {
    #[serde(flatten)]
    metadata: Metadata,
    content: String,
}

pub(crate) struct MediaFile {
    pub name: String,
    pub file: File,
    pub length: u64,
}

pub(crate) struct Upload {
    pub fields: Fields,
    pub files: Vec<MediaFile>,
}

/// Checks the complete input before any HTTP request; open media handles are carried to upload.
pub(crate) fn prepare(directory: &Path) -> Result<Upload, Error> {
    let root = directory
        .canonicalize()
        .map_err(|error| Error::at(directory, error))?;
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&root).map_err(|error| Error::at(&root, error))? {
        let entry = entry.map_err(|error| Error::at(&root, error))?;
        if entry.path().extension().is_some_and(|ext| ext == "md") {
            candidates.push(entry.path());
        }
    }
    candidates.sort();
    let [body_path] = candidates.as_slice() else {
        return Err(Error::at(
            &root,
            format!("expected exactly one top-level .md file; found {candidates:?}"),
        ));
    };
    let metadata_path = root.join("article.json");
    let metadata: Metadata = serde_json::from_reader(open_regular(&metadata_path)?)
        .map_err(|error| Error::at(&metadata_path, error))?;
    validate_metadata(&metadata, &metadata_path)?;
    let mut body = String::new();
    // Bound the read before parsing, rather than allocating an arbitrarily large source file.
    use io::Read;
    open_regular(body_path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut body)
        .map_err(|error| Error::at(body_path, error))?;
    let (content, names) =
        markdown::convert(&body).map_err(|message| Error::at(body_path, message))?;
    palace_domain::ArticleBody::parse(content.clone())
        .map_err(|error| Error::at(body_path, error))?;
    let mut files = Vec::new();
    if !names.is_empty() {
        let assets = root.join("assets");
        let kind = fs::symlink_metadata(&assets).map_err(|error| Error::at(&assets, error))?;
        if !kind.is_dir() {
            return Err(Error::at(
                &assets,
                "expected a directory, not a symbolic link",
            ));
        }
        for name in names {
            let path = assets.join(&name);
            let file = open_regular(&path)?;
            let length = file
                .metadata()
                .map_err(|error| Error::at(&path, error))?
                .len();
            files.push(MediaFile { name, file, length });
        }
    }
    Ok(Upload {
        fields: Fields { metadata, content },
        files,
    })
}

/// Rejects links and special files before opening, so local media cannot escape the directory.
fn open_regular(path: &Path) -> Result<File, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|error| Error::at(path, error))?;
    if !metadata.is_file() {
        return Err(Error::at(
            path,
            "expected a regular file, not a symbolic link",
        ));
    }
    File::open(path).map_err(|error| Error::at(path, error))
}

/// Validates stable API rules without resolving a publish day using the CLI machine's timezone.
fn validate_metadata(metadata: &Metadata, path: &Path) -> Result<(), Error> {
    palace_domain::validate_title(&metadata.title).map_err(|error| Error::at(path, error))?;
    let valid_url = url::Url::parse(&metadata.url)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
    if !valid_url || metadata.url.trim() != metadata.url || metadata.url.len() > 2048 {
        return Err(Error::at(
            path,
            "url: expected an absolute http/https URL of at most 2048 bytes without surrounding whitespace",
        ));
    }
    let date = &metadata.date;
    let valid_date = if date.len() == 10 {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .is_ok_and(|day| day.format("%Y-%m-%d").to_string() == *date)
    } else {
        DateTime::parse_from_rfc3339(date).is_ok()
    };
    if !valid_date {
        return Err(Error::at(
            path,
            "date: expected YYYY-MM-DD or RFC 3339 with an offset",
        ));
    }
    if metadata
        .author
        .as_ref()
        .is_some_and(|author| !author.trim().is_empty() && author.len() > 256)
    {
        return Err(Error::at(path, "author: exceeds 256 UTF-8 bytes"));
    }
    Ok(())
}
