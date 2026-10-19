//! Only the expected root-level executable is accepted; archive paths are never unpacked.
use super::Error;
use flate2::read::GzDecoder;
use std::{
    fs::File,
    io::{self, Cursor, Read},
    path::Path,
};

const BINARY_LIMIT: u64 = 256 * 1024 * 1024;

/// Streams one regular, nonempty executable into a private staging directory.
pub(super) fn extract(bytes: &[u8], binary_name: &str, destination: &Path) -> Result<(), Error> {
    let invalid = || Error::InvalidRelease("archive must contain only the pal executable".into());
    let mut output = File::create(destination)?;
    if binary_name == "pal.exe" {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
        if archive.len() != 1 {
            return Err(invalid());
        }
        let mut entry = archive.by_index(/*file_number*/ 0)?;
        if entry.name_raw() != binary_name.as_bytes()
            || !entry.is_file()
            || entry.size() == 0
            || entry.size() > BINARY_LIMIT
        {
            return Err(invalid());
        }
        let expected_size = entry.size();
        let copied = io::copy(&mut entry.by_ref().take(BINARY_LIMIT + 1), &mut output)?;
        if copied != expected_size {
            return Err(invalid());
        }
    } else {
        // Limit expanded data as well as the downloaded bytes to bound decompression work.
        let decoder = GzDecoder::new(bytes).take(BINARY_LIMIT + 1024 * 1024);
        let mut archive = tar::Archive::new(decoder);
        let mut entries = archive.entries()?;
        let mut entry = entries.next().ok_or_else(invalid)??;
        if entry.path()?.as_ref() != Path::new(binary_name)
            || !entry.header().entry_type().is_file()
            || entry.size() == 0
            || entry.size() > BINARY_LIMIT
        {
            return Err(invalid());
        }
        let expected_size = entry.size();
        let copied = io::copy(&mut entry, &mut output)?;
        drop(entry);
        if copied != expected_size || entries.next().is_some() {
            return Err(invalid());
        }
        // TAR stops before the gzip trailer; finish reading to validate the gzip checksum too.
        let mut decoder = archive.into_inner();
        io::copy(&mut decoder, &mut io::sink())?;
        if decoder.limit() == 0 {
            return Err(invalid());
        }
    }
    output.sync_all()?;
    Ok(())
}
