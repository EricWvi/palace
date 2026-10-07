//! Path containment for source accounting; aliases cannot move source outside the workspace.

use std::path::{Path, PathBuf};

/// Canonicalizes existing paths before checking the workspace boundary, including symlink targets.
pub(crate) fn resolve_existing(
    workspace: &Path,
    path: &Path,
) -> super::rust_architecture::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(format!("path must be absolute: {}", path.display()).into());
    }
    let resolved = path.canonicalize()?;
    if !resolved.starts_with(workspace) {
        return Err(format!("path escapes its workspace: {}", path.display()).into());
    }
    Ok(resolved)
}

/// Resolves in-memory fixture paths without requiring the fixture files to exist on disk.
#[cfg(test)]
pub(crate) fn normalize_relative(path: &Path) -> Option<PathBuf> {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => return None,
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::{normalize_relative, resolve_existing};
    use pretty_assertions::assert_eq;
    use std::path::{Path, PathBuf};

    /// In-memory module links follow filesystem component rules and cannot escape their root.
    #[test]
    fn normalizes_relative_links_and_rejects_escapes() {
        assert_eq!(
            normalize_relative(&Path::new("src").join("..").join("shared.rs")),
            Some(PathBuf::from("shared.rs"))
        );
        assert_eq!(normalize_relative(Path::new(".")), Some(PathBuf::new()));
        assert_eq!(normalize_relative(&Path::new("..").join("escape.rs")), None);
    }

    /// Relative inputs are rejected even if resolving them from the process directory would work.
    #[test]
    fn rejects_non_absolute_source_paths() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_existing(directory.path(), Path::new("source.rs"))
                .unwrap_err()
                .to_string(),
            "path must be absolute: source.rs"
        );
    }
}
