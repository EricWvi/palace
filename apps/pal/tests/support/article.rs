//! Local article directories shared by wire-level and real Palace tests.
use std::{fs, path::Path};
use tempfile::TempDir;

pub const METADATA: &str = r#"{"title":"标题","url":"https://example.com/original?keep=1","date":"2026-07-03","author":"作者"}"#;

/// Builds a complete tiny article; unused files deliberately must not enter the upload.
pub fn directory(body: &str) -> TempDir {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    fs::write(root.join("article.json"), METADATA).unwrap();
    fs::write(root.join("post.md"), body).unwrap();
    let assets = root.join("assets");
    fs::create_dir(&assets).unwrap();
    for (name, bytes) in [
        ("a.jpg", &b"image"[..]),
        ("b.mp4", &b"video"[..]),
        ("unused.png", &b"unused"[..]),
    ] {
        fs::write(assets.join(name), bytes).unwrap();
    }
    folder
}

/// Makes process arguments without relying on a developer's working directory.
pub fn args(path: &Path) -> [&str; 3] {
    ["article", "upload", path.to_str().unwrap()]
}
