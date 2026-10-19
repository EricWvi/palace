use palace_db::Database;
use palace_jottings::{JottingService, JottingWatch, Jottings, watch};
use palace_logging::{palace_error, palace_info};
use std::{path::PathBuf, sync::Arc, time::Duration};

// How often HEAD is read, for mounts whose file events never arrive.
const POLL: Duration = Duration::from_secs(/*secs*/ 60);

/// Serves the jottings folder named by `PALACE_JOTTINGS_DIR`, or no jottings when it is unset.
/// A folder that is set but unusable stops startup. The watch must be kept alive for as long as
/// the index should follow the branch.
pub(crate) async fn jottings_from_env(
    database: Database,
    timezone: chrono_tz::Tz,
) -> Result<(JottingService, Option<JottingWatch>), Box<dyn std::error::Error>> {
    let Some(folder) = std::env::var_os("PALACE_JOTTINGS_DIR").map(PathBuf::from) else {
        palace_info!(message = "PALACE_JOTTINGS_DIR unset; serving no jottings");
        return Ok((JottingService::Unconfigured, None));
    };
    let jottings = Arc::new(
        Jottings::open(&folder, database, timezone)
            .map_err(|error| format!("PALACE_JOTTINGS_DIR: {error}"))?,
    );
    palace_info!(message = "Serving jottings", folder = %jottings.folder().display());
    // A refused commit is logged and leaves the index as it was; only an unreachable database
    // or repository stops startup.
    if let Err(error) = jottings.refresh().await {
        palace_error!(message = "Initial jotting reconciliation failed", error = %error);
        return Err(error.into());
    }
    let watch = watch(jottings.clone(), POLL)?;
    Ok((JottingService::Configured(jottings), Some(watch)))
}
