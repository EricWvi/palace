//! Embed repository identity so CLI versions describe the source rather than Cargo's placeholder.
use std::{env, path::Path, process::Command};

/// Rebuilds metadata after commits or tag changes, including in worktrees and shallow CI checkouts.
fn main() {
    println!("cargo:rerun-if-env-changed=PAL_RELEASE_TAG");
    for name in ["HEAD", "refs", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name])
            && Path::new(&path).exists()
        {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    let hash = git(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "unknown".into());
    // CI supplies the triggering tag because multiple release tags can point at the same commit.
    let tag = env::var("PAL_RELEASE_TAG")
        .ok()
        .filter(|tag| !tag.is_empty())
        .or_else(|| git(&["describe", "--tags", "--exact-match", "HEAD"]))
        .unwrap_or_else(|| "untagged".into());
    println!("cargo:rustc-env=PAL_BUILD_VERSION=built on {hash} (tag: {tag})");
}

/// Treats missing Git metadata as an unavailable identity, allowing source archives to build.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
