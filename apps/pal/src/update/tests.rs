//! Fixtures cover the whole validation pipeline without ever replacing the developer's CLI.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use flate2::{Compression, write::GzEncoder};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::{cell::RefCell, fs, io::Write};

const TARGET: &str = "x86_64-unknown-linux-gnu";
const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";
const CONTENT: &[u8] = b"downloaded CLI";

struct Fixture {
    manifest: Value,
    archive: Vec<u8>,
    requested: RefCell<Vec<String>>,
}

impl Download for Fixture {
    /// Rejects unexpected URLs so tests also prove that downloads stay pinned to one release.
    fn get(&self, url: &str, _limit: u64) -> Result<Vec<u8>, Error> {
        self.requested.borrow_mut().push(url.into());
        if url == format!("{RELEASES}latest/download/latest.json") {
            return Ok(serde_json::to_vec(&self.manifest).unwrap());
        }
        if url == self.manifest["assets"][0]["url"].as_str().unwrap() {
            return Ok(self.archive.clone());
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "unexpected download").into())
    }
}

/// Mirrors the release workflow's one-file gzip archive, including special file fixtures.
fn tarball(name: &str, kind: tar::EntryType, content: &[u8]) -> Vec<u8> {
    let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(kind);
    header.set_size(content.len() as u64);
    header.set_mode(/*mode*/ 0o755);
    if kind.is_symlink() {
        header.set_link_name("elsewhere").unwrap();
    }
    header.set_cksum();
    archive.append_data(&mut header, name, content).unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}

/// Mirrors the Windows release workflow's deflated ZIP archive.
fn zipfile(name: &str, content: &[u8]) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    archive
        .start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(content).unwrap();
    archive.finish().unwrap().into_inner()
}

/// Produces a manifest whose digest covers the supplied bytes, including malformed archives.
fn fixture(target: &str, archive: Vec<u8>) -> Fixture {
    let extension = if target == WINDOWS_TARGET {
        "zip"
    } else {
        "tar.gz"
    };
    let name = format!("pal-v1.2.3-{target}.{extension}");
    Fixture {
        manifest: json!({
            "version": "v1.2.3",
            "assets": [{
                "target": target,
                "name": name,
                "url": format!("{RELEASES}download/v1.2.3/{name}"),
                "sha256": format!("{:x}", Sha256::digest(&archive)),
            }],
        }),
        archive,
        requested: RefCell::new(Vec::new()),
    }
}

/// Both release formats must deliver exactly the verified executable to the installer.
/// Core test: specs/test-cases/cli/update.md#updates-must-verify-the-release-before-replacing-the-executable
#[test]
fn verifies_and_stages_both_release_formats_before_installing() {
    for (target, archive, binary_name) in [
        (
            TARGET,
            tarball("pal", tar::EntryType::Regular, CONTENT),
            "pal",
        ),
        (WINDOWS_TARGET, zipfile("pal.exe", CONTENT), "pal.exe"),
    ] {
        let fixture = fixture(target, archive);
        let mut staged = None;
        let version = install_latest(&fixture, target, |path| {
            assert_eq!(path.file_name().unwrap(), binary_name);
            assert_eq!(fs::read(path).unwrap(), CONTENT);
            staged = Some(path.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(version, "v1.2.3");
        assert!(!staged.unwrap().exists(), "staging must be cleaned up");
        assert_eq!(
            *fixture.requested.borrow(),
            vec![
                format!("{RELEASES}latest/download/latest.json"),
                fixture.manifest["assets"][0]["url"]
                    .as_str()
                    .unwrap()
                    .into(),
            ]
        );
    }
}

/// A bad manifest must fail before downloading or touching any executable.
/// Core test: specs/test-cases/cli/update.md#updates-must-verify-the-release-before-replacing-the-executable
#[test]
fn rejects_untrusted_or_ambiguous_manifest_assets() {
    let valid = fixture(TARGET, tarball("pal", tar::EntryType::Regular, CONTENT));
    let mut variants = Vec::new();
    for (path, value) in [
        ("/version", json!("v../../other")),
        ("/assets/0/target", json!("aarch64-unknown-linux-gnu")),
        ("/assets/0/name", json!("other.tar.gz")),
        ("/assets/0/url", json!("https://example.com/pal.tar.gz")),
        ("/assets/0/sha256", json!("invalid")),
    ] {
        let mut manifest = valid.manifest.clone();
        *manifest.pointer_mut(path).unwrap() = value;
        variants.push(manifest);
    }
    let mut duplicate = valid.manifest.clone();
    duplicate["assets"]
        .as_array_mut()
        .unwrap()
        .push(valid.manifest["assets"][0].clone());
    variants.push(duplicate);
    variants.push(json!({"version": "v1.2.3"}));
    for manifest in variants {
        let fixture = Fixture {
            manifest,
            archive: Vec::new(),
            requested: RefCell::default(),
        };
        assert!(install_latest(&fixture, TARGET, |_| panic!("must not install")).is_err());
        assert_eq!(fixture.requested.borrow().len(), 1);
    }
}

/// Corruption must be caught by the digest before the archive is parsed.
/// Core test: specs/test-cases/cli/update.md#updates-must-verify-the-release-before-replacing-the-executable
#[test]
fn checksum_mismatch_never_reaches_the_installer() {
    let mut fixture = fixture(TARGET, tarball("pal", tar::EntryType::Regular, CONTENT));
    fixture.archive[0] ^= 1;
    assert!(matches!(
        install_latest(&fixture, TARGET, |_| panic!("must not install")),
        Err(Error::Checksum(_))
    ));
}

/// Even correctly checksummed archives cannot install links, nested files, or corrupt data.
/// Core test: specs/test-cases/cli/update.md#updates-must-verify-the-release-before-replacing-the-executable
#[test]
fn invalid_archives_never_reach_the_installer() {
    let mut bad_gzip = tarball("pal", tar::EntryType::Regular, CONTENT);
    let checksum_byte = bad_gzip.len() - 8;
    bad_gzip[checksum_byte] ^= 1;
    for (target, archive) in [
        (TARGET, tarball("other", tar::EntryType::Regular, CONTENT)),
        (
            TARGET,
            tarball("nested/pal", tar::EntryType::Regular, CONTENT),
        ),
        (TARGET, tarball("pal", tar::EntryType::Symlink, b"")),
        (TARGET, tarball("pal", tar::EntryType::Regular, b"")),
        (TARGET, bad_gzip),
        (TARGET, b"not an archive".to_vec()),
        (WINDOWS_TARGET, zipfile("../pal.exe", CONTENT)),
        (WINDOWS_TARGET, zipfile("other.exe", CONTENT)),
        (WINDOWS_TARGET, zipfile("pal.exe", b"")),
        (WINDOWS_TARGET, b"not an archive".to_vec()),
    ] {
        let fixture = fixture(target, archive);
        assert!(install_latest(&fixture, target, |_| panic!("must not install")).is_err());
    }
}

/// Permission errors must surface as installation failures and still clean up staging files.
/// Core test: specs/test-cases/cli/update.md#self-replacement-must-preserve-executable-permissions-and-clean-up-staging
#[test]
fn replacement_errors_are_reported_and_staging_is_removed() {
    let fixture = fixture(TARGET, tarball("pal", tar::EntryType::Regular, CONTENT));
    let mut staged = None;
    let error = install_latest(&fixture, TARGET, |path| {
        staged = Some(path.to_owned());
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "read-only installation",
        ))
    })
    .unwrap_err();
    assert!(matches!(error, Error::Replace(_)));
    assert!(!staged.unwrap().exists());
}

/// A missing release or oversized HTTP body must never be returned as an update artifact.
/// Core test: specs/test-cases/cli/update.md#updates-must-verify-the-release-before-replacing-the-executable
#[test]
fn download_checks_status_and_size() {
    use std::{io::BufRead as _, net::TcpListener, thread};

    for (status, body, limit, succeeds) in [
        ("404 Not Found", "missing release", 100, false),
        ("200 OK", "large body", 3, false),
        ("200 OK", "ok", 2, true),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            for line in io::BufReader::new(&socket).lines() {
                if line.unwrap().is_empty() {
                    break;
                }
            }
            write!(
                socket,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(/*secs*/ 5))
            .build()
            .unwrap();
        let response = Download::get(&client, &url, limit);
        assert_eq!(response.is_ok(), succeeds);
        if succeeds {
            assert_eq!(response.unwrap(), body.as_bytes());
        }
        server.join().unwrap();
    }
}

/// Exercises real self-replacement in a copied test process, preserving the actual test runner.
/// Core test: specs/test-cases/cli/update.md#self-replacement-must-preserve-executable-permissions-and-clean-up-staging
#[cfg(unix)]
#[test]
fn replaces_a_running_executable_after_verification() {
    use std::{os::unix::fs::PermissionsExt, process::Command};

    const CHILD: &str = "PAL_UPDATE_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let fixture = fixture(TARGET, tarball("pal", tar::EntryType::Regular, CONTENT));
        install_latest(&fixture, TARGET, |path| self_replace::self_replace(path)).unwrap();
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("pal-update-test");
    fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(/*mode*/ 0o751)).unwrap();
    let output = Command::new(&executable)
        .args([
            "--exact",
            "update::tests::replaces_a_running_executable_after_verification",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fs::read(&executable).unwrap(), CONTENT);
    assert_eq!(
        fs::metadata(executable).unwrap().permissions().mode() & 0o777,
        0o751
    );
}
