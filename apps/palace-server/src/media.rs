use palace_logging::palace_info;
use palace_media::{MediaConfig, MediaStorage};

/// Connects the object storage named by the `PALACE_S3_*` variables, or keeps media disabled when
/// none is set. A partial set fails startup instead of silently disabling media.
pub(crate) fn media_from_env() -> Result<MediaStorage, Box<dyn std::error::Error>> {
    let config = MediaConfig::from_lookup(|name| std::env::var(name).ok())?;
    match &config {
        MediaConfig::Unconfigured => {
            palace_info!(message = "PALACE_S3_* unset; media storage disabled");
        }
        MediaConfig::S3(s3) => {
            palace_info!(message = "Storing media", endpoint = %s3.endpoint, bucket = %s3.bucket, max_bytes = s3.limits.max_bytes);
        }
    }
    Ok(MediaStorage::connect(&config)?)
}
