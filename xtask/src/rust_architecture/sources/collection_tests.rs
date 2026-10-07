use super::collect;
use crate::rust_architecture::{ModuleSize, check_rust_architecture};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Creates a real Git/Cargo workspace without registry dependencies or environment changes.
fn workspace() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let member = root.join("member");
    fs::create_dir_all(member.join("src")).unwrap();
    fs::create_dir(root.join("xtask")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"palace-fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(
        member.join("Cargo.toml"),
        "[package]\nname = \"palace-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[lib]\npath = \"src/entry.rs\"\n",
    )
    .unwrap();
    fs::write(member.join("src").join("entry.rs"), "pub fn run() {}\n").unwrap();
    fs::write(
        root.join("xtask").join("rust-size-baseline.json"),
        "{\"modules\":{}}\n",
    )
    .unwrap();
    let initialized = Command::new("git")
        .current_dir(root)
        .args(["init", "--quiet"])
        .output()
        .unwrap();
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    directory
}

/// Real Cargo roots and Git's untracked files use the same exclusions as graph-only tests.
#[test]
fn collects_untracked_modules_custom_roots_and_test_helpers() {
    let directory = workspace();
    let root = directory.path();
    let member = root.join("member");
    let source = member.join("src");
    fs::create_dir(source.join("tests")).unwrap();
    fs::create_dir(member.join("tests")).unwrap();
    fs::create_dir(member.join("examples")).unwrap();
    fs::write(
        source.join("entry.rs"),
        "#[cfg(test)] mod tests;\n#[cfg(windows)] mod platform;\npub fn run() {}\n",
    )
    .unwrap();
    fs::write(source.join("platform.rs"), "pub fn platform() {}\n").unwrap();
    fs::write(source.join("tests.rs"), "mod fixture;\nfn test() {}\n").unwrap();
    fs::write(source.join("tests").join("fixture.rs"), "fn setup() {}\n").unwrap();
    fs::write(source.join("orphan.rs"), "fn retained() {}\n").unwrap();
    fs::write(source.join("generated.rs"), "// @generated\ninvalid Rust\n").unwrap();
    fs::write(source.join("ignored.rs"), "invalid Rust\n").unwrap();
    fs::write(root.join(".gitignore"), "ignored.rs\n").unwrap();
    fs::write(
        member.join("tests").join("integration.rs"),
        "mod fixture;\nfn test() {}\n",
    )
    .unwrap();
    fs::write(member.join("tests").join("fixture.rs"), "fn setup() {}\n").unwrap();
    fs::write(member.join("examples").join("demo.rs"), "fn main() {}\n").unwrap();

    let expected = [
        (member.join("src").join("entry.rs"), 2, false),
        (member.join("src").join("platform.rs"), 1, false),
        (member.join("src").join("tests.rs"), 0, true),
        (member.join("src").join("tests").join("fixture.rs"), 0, true),
        (member.join("src").join("orphan.rs"), 1, false),
        (member.join("tests").join("integration.rs"), 0, true),
        (member.join("tests").join("fixture.rs"), 0, true),
        (member.join("examples").join("demo.rs"), 1, false),
    ]
    .into_iter()
    .map(|(path, production_lines, test_only)| {
        (
            path.strip_prefix(root).unwrap().to_path_buf(),
            ModuleSize {
                owner: "palace-fixture".into(),
                production_lines,
                test_only,
            },
        )
    })
    .collect::<BTreeMap<PathBuf, ModuleSize>>();
    assert_eq!(collect(root).unwrap(), expected);
}

/// An untracked oversized file fails the real check before it can be committed or wired into a root.
#[test]
fn rejects_untracked_oversized_source_without_rewriting_the_baseline() {
    let directory = workspace();
    let root = directory.path();
    let path = PathBuf::from("member").join("src").join("orphan.rs");
    fs::write(
        root.join(&path),
        "// production comment\n".repeat(/*n*/ 801),
    )
    .unwrap();
    assert_eq!(
        check_rust_architecture(root).unwrap_err().to_string(),
        format!(
            "{}: new oversized production module (801 > 800); split by responsibility",
            path.display()
        )
    );
    assert_eq!(
        fs::read_to_string(root.join("xtask").join("rust-size-baseline.json")).unwrap(),
        "{\"modules\":{}}\n"
    );
}

/// Removed tracked files are absent from accounting instead of causing stale filesystem reads.
#[test]
fn tolerates_unstaged_source_deletions() {
    let directory = workspace();
    let root = directory.path();
    let deleted = root.join("member").join("src").join("deleted.rs");
    fs::write(&deleted, "fn obsolete() {}\n").unwrap();
    let staged = Command::new("git")
        .current_dir(root)
        .args(["add", "--"])
        .arg(&deleted)
        .output()
        .unwrap();
    assert!(staged.status.success());
    fs::remove_file(deleted).unwrap();
    assert_eq!(
        collect(root).unwrap(),
        BTreeMap::from([(
            PathBuf::from("member").join("src").join("entry.rs"),
            ModuleSize {
                owner: "palace-fixture".into(),
                production_lines: 1,
                test_only: false,
            }
        )])
    );
}

/// A generated marker cannot silently exclude a required Cargo target from accounting.
#[test]
fn rejects_missing_cargo_root_accounting() {
    let directory = workspace();
    let entry = directory.path().join("member").join("src").join("entry.rs");
    fs::write(&entry, "// @generated\npub fn run() {}\n").unwrap();
    assert_eq!(
        collect(directory.path()).unwrap_err().to_string(),
        format!(
            "{}: Cargo root is missing from repository source accounting",
            entry.canonicalize().unwrap().display()
        )
    );
}

/// Canonicalization blocks symlink aliases that would assign an external file to a workspace owner.
#[cfg(unix)]
#[test]
fn rejects_source_symlinks_outside_the_workspace() {
    let directory = workspace();
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join("external.rs");
    fs::write(&external, "pub fn external() {}\n").unwrap();
    let alias = directory.path().join("member").join("src").join("alias.rs");
    std::os::unix::fs::symlink(external, &alias).unwrap();
    let canonical_root = directory.path().canonicalize().unwrap();
    assert_eq!(
        collect(directory.path()).unwrap_err().to_string(),
        format!(
            "path escapes its workspace: {}",
            canonical_root
                .join("member")
                .join("src")
                .join("alias.rs")
                .display()
        )
    );
}
