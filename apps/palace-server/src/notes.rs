use palace_logging::palace_info;
use palace_notes::{NoteLibrary, NoteWatch, Ripgrep, watch};
use std::{path::PathBuf, sync::Arc, time::Duration};

// How often the cache's stamp is compared, for mounts whose file events never arrive.
const POLL: Duration = Duration::from_secs(/*secs*/ 60);

/// Serves the notes folder named by `PALACE_NOTES_DIR`, or no notes when it is unset. The watch
/// must be kept alive for as long as the library should follow its cache.
pub(crate) fn notes_from_env(
    timezone: chrono_tz::Tz,
) -> Result<(Arc<NoteLibrary>, Option<NoteWatch>), Box<dyn std::error::Error>> {
    let Some(root) = std::env::var_os("PALACE_NOTES_DIR").map(PathBuf::from) else {
        palace_info!(message = "PALACE_NOTES_DIR unset; serving no notes");
        return Ok((Arc::new(NoteLibrary::absent()), None));
    };
    palace_info!(message = "Serving notes", folder = %root.display());
    let library = Arc::new(NoteLibrary::open(root, timezone, Ripgrep::default()));
    let watch = watch(library.clone(), POLL)?;
    Ok((library, watch))
}
