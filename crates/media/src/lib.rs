//! Media files: bytes in S3-compatible object storage, identity and ownership in PostgreSQL.
//! Only the server-generated media id ever leaves this crate's callers; object keys, buckets and
//! endpoints stay internal so storage can move without rewriting any reference.
mod config;
mod service;
mod store;

pub use config::{Addressing, ConfigError, MediaConfig, S3Bucket, S3Config};
pub use service::{MediaError, delete, upload};
pub use store::{Bucket, MediaStore, StoreError, object_key};

use palace_domain::MediaLimits;

/// The media capability of a running server.
pub enum MediaStorage {
    /// Media endpoints answer as unavailable without reading the request body.
    Unconfigured,
    Configured {
        store: S3Bucket,
        limits: MediaLimits,
    },
}
impl MediaStorage {
    /// Connects the configured bucket, or keeps media disabled when nothing is configured.
    pub fn connect(config: &MediaConfig) -> Result<Self, ConfigError> {
        Ok(match config {
            MediaConfig::Unconfigured => Self::Unconfigured,
            MediaConfig::S3(s3) => Self::Configured {
                store: s3.connect()?,
                limits: s3.limits,
            },
        })
    }
}
