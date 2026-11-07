//! Article export: the directory `pal article upload` reads, as one zip, so an exported article
//! can be imported again with its images and videos. Object storage has no CORS, so a browser
//! cannot fetch the bytes itself; the server assembles the archive.
use crate::{MediaError, MediaStore};
use bytes::Bytes;
use chrono_tz::Tz;
use palace_db::{Article, Database, OwnerScope};
use palace_domain::export_body;
use palace_logging::palace_error;
use std::{
    collections::HashMap,
    io::{Cursor, Write},
};
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// An exported article, ready to send.
#[derive(Debug, PartialEq, Eq)]
pub struct ArticleArchive {
    /// The article's title and `.zip`, before any encoding a header needs.
    pub file_name: String,
    pub bytes: Vec<u8>,
}

/// Reads an article and its media into an archive. It reads everything before writing, so a
/// missing object fails the export instead of leaving a partial file. An article that embeds
/// nothing needs no storage. The import limit bounds the memory an export can hold.
pub async fn export_article<S: MediaStore>(
    database: &Database,
    store: Option<&S>,
    owner: OwnerScope,
    id: Uuid,
    timezone: Tz,
) -> Result<ArticleArchive, MediaError> {
    let article = database.article(owner, id).await?;
    let stored = database.article_files(owner, id).await?;
    let mut names = HashMap::with_capacity(stored.len());
    let mut files = Vec::with_capacity(stored.len());
    if !stored.is_empty() {
        let store = store.ok_or(MediaError::Unconfigured)?;
        for file in stored {
            let content = store.get(&file.object_key).await?;
            // An import always names its files, and only with the `MediaName` alphabet, so a
            // name is safe as an archive path. The column allows none for media uploaded alone.
            let name = file.file_name.unwrap_or_else(|| file.id.to_string());
            names.insert(file.id, name.clone());
            files.push((name, content));
        }
    }
    let file_name = format!("{}.zip", article.metadata.title());
    let document = document(&article, timezone)?;
    let body = export_body(&article.content, &names);
    // Checksumming and deflating are CPU work too long for an async worker.
    let bytes = tokio::task::spawn_blocking(move || write_archive(&document, &body, files))
        .await
        .map_err(archive_failed)?
        .map_err(archive_failed)?;
    Ok(ArticleArchive { file_name, bytes })
}

/// Spells `article.json`. The date is in the form an import takes, so only a day stays a day.
fn document(article: &Article, timezone: Tz) -> Result<Vec<u8>, MediaError> {
    serde_json::to_vec_pretty(&serde_json::json!({
        "title": article.metadata.title(),
        "url": article.metadata.url(),
        "date": article.metadata.published().to_text(timezone),
        "author": article.metadata.author(),
    }))
    .map_err(archive_failed)
}

/// Logs the cause here, since the API reports every such failure the same way.
fn archive_failed(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> MediaError {
    let error = error.into();
    palace_error!(message = "Article export failed", error = %error);
    MediaError::Archive(error)
}

/// Writes the three parts of the directory. Files are dropped as they are written, so the
/// archive never needs a second copy of them in memory.
fn write_archive(
    document: &[u8],
    body: &str,
    files: Vec<(String, Bytes)>,
) -> Result<Vec<u8>, zip::result::ZipError> {
    let text = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    // Images and videos are compressed already; deflating them again would only cost time.
    let media = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(true);
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    archive.start_file("article.json", text)?;
    archive.write_all(document)?;
    archive.start_file("article.md", text)?;
    archive.write_all(body.as_bytes())?;
    for (name, content) in files {
        archive.start_file(format!("assets/{name}"), media)?;
        archive.write_all(&content)?;
    }
    Ok(archive.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::Read;

    /// Every part lands under the path `pal article upload` looks for, byte for byte.
    /// Core test case:
    /// - `specs/test-cases/web/stars/articles.md#exporting-an-article-must-write-the-directory-pal-uploads`
    #[test]
    fn archive_holds_the_upload_directory() {
        let bytes = write_archive(
            b"{}",
            "![](assets/a.png)\n",
            vec![("a.png".into(), Bytes::from_static(b"png"))],
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut entries = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let mut content = String::new();
            entry.read_to_string(&mut content).unwrap();
            entries.push((entry.name().unwrap().to_string(), content));
        }
        assert_eq!(
            entries,
            [
                ("article.json".to_owned(), "{}".to_owned()),
                ("article.md".to_owned(), "![](assets/a.png)\n".to_owned()),
                ("assets/a.png".to_owned(), "png".to_owned()),
            ]
        );
    }
}
