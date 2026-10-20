//! Updates are staged and verified before the running executable is touched.
mod archive;
#[cfg(test)]
mod proxy_tests;
#[cfg(test)]
mod tests;

use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::Path,
    time::Duration,
};
use url::Url;

const RELEASES: &str = "https://github.com/EricWvi/palace/releases/";
const MANIFEST_LIMIT: u64 = 1024 * 1024;
const ARCHIVE_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("no pal release is available for this platform ({0})")]
    UnsupportedPlatform(String),
    #[error("cannot download the GitHub release: {0}")]
    Http(#[from] reqwest::Error),
    #[error("cannot prepare the downloaded CLI: {0}")]
    Io(#[from] io::Error),
    #[error("invalid latest.json: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("invalid release: {0}")]
    InvalidRelease(String),
    #[error("SHA-256 mismatch for {0}; the current CLI was not changed")]
    Checksum(String),
    #[error("invalid ZIP archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("cannot replace the current CLI (check installation directory permissions): {0}")]
    Replace(io::Error),
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    target: String,
    name: String,
    url: String,
    sha256: String,
}

/// A bounded download source lets tests drive failures before the replacement boundary.
/// Implementations must fail rather than return a partial or oversized response.
trait Download {
    /// Fetches the entire resource subject to the caller's size limit.
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, Error>;
}

impl Download for Client {
    /// Checks HTTP status before reading, so error pages can never become update artifacts.
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        self.get(url)
            .send()?
            .error_for_status()?
            .take(limit + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(Error::InvalidRelease("download exceeds size limit".into()));
        }
        Ok(bytes)
    }
}

/// Downloads only official HTTPS assets, then uses the platform-specific replacement procedure.
pub(crate) fn run() -> Result<String, Error> {
    let target = release_target(std::env::consts::OS, std::env::consts::ARCH)?;
    let source = download_client_builder().build()?;
    install_latest(&source, target, |path| self_replace::self_replace(path))
}

/// Retains reqwest's standard environment proxy discovery, including NO_PROXY exclusions.
fn download_client_builder() -> reqwest::blocking::ClientBuilder {
    Client::builder()
        .user_agent(format!("pal/{}", crate::VERSION))
        .https_only(/*enabled*/ true)
        // GitHub redirects release assets to its storage host.
        .redirect(reqwest::redirect::Policy::limited(/*max*/ 10))
        .connect_timeout(Duration::from_secs(/*secs*/ 15))
        .timeout(Duration::from_secs(/*secs*/ 180))
}

/// Linux releases use static musl binaries even when the current CLI was built with GNU libc.
fn release_target(os: &str, arch: &str) -> Result<&'static str, Error> {
    match (os, arch) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-musl"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("windows", "x86_64") if cfg!(target_env = "msvc") => Ok("x86_64-pc-windows-msvc"),
        _ => Err(Error::UnsupportedPlatform(format!("{os}/{arch}"))),
    }
}

/// Keeps every fallible download and validation step ahead of the single installation call.
fn install_latest(
    source: &impl Download,
    target: &str,
    install: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<String, Error> {
    let manifest_url = format!("{RELEASES}latest/download/latest.json");
    let manifest: Manifest = serde_json::from_slice(&source.get(&manifest_url, MANIFEST_LIMIT)?)?;
    let version = &manifest.version;
    if !version.starts_with('v')
        || version.len() < 2
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(Error::InvalidRelease(
            "invalid version in latest.json".into(),
        ));
    }
    let mut assets = manifest
        .assets
        .iter()
        .filter(|asset| asset.target == target);
    let asset = assets
        .next()
        .ok_or_else(|| Error::UnsupportedPlatform(target.into()))?;
    let extension = if target.ends_with("windows-msvc") {
        "zip"
    } else {
        "tar.gz"
    };
    let expected_name = format!("pal-{version}-{target}.{extension}");
    // Pin the archive to the manifest's release, even if another release becomes latest meanwhile.
    let mut expected_url =
        Url::parse(RELEASES).map_err(|error| Error::InvalidRelease(error.to_string()))?;
    expected_url
        .path_segments_mut()
        .map_err(|()| Error::InvalidRelease("invalid releases URL".into()))?
        .pop_if_empty()
        .extend(["download", version, &expected_name]);
    if assets.next().is_some()
        || asset.name != expected_name
        || asset.url != expected_url.as_str()
        || asset.sha256.len() != 64
        || !asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(Error::InvalidRelease("invalid asset in latest.json".into()));
    }
    let bytes = source.get(&asset.url, ARCHIVE_LIMIT)?;
    if format!("{:x}", Sha256::digest(&bytes)) != asset.sha256.to_ascii_lowercase() {
        return Err(Error::Checksum(asset.name.clone()));
    }
    let staging = tempfile::tempdir()?;
    let binary_name = if extension == "zip" { "pal.exe" } else { "pal" };
    let binary = staging.path().join(binary_name);
    archive::extract(&bytes, binary_name, &binary)?;
    install(&binary).map_err(Error::Replace)?;
    Ok(manifest.version)
}
