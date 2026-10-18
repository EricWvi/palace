use crate::Bucket;
use object_store::{
    RetryConfig,
    aws::{AmazonS3, AmazonS3Builder},
};
use palace_domain::MediaLimits;
use std::time::Duration;

/// The S3 object store used in production.
pub type S3Bucket = Bucket<AmazonS3>;

// Every S3 setting must be present together; a partial set is a deployment mistake.
const S3_VARIABLES: [&str; 6] = [
    "PALACE_S3_ENDPOINT",
    "PALACE_S3_REGION",
    "PALACE_S3_BUCKET",
    "PALACE_S3_ACCESS_KEY_ID",
    "PALACE_S3_SECRET_ACCESS_KEY",
    "PALACE_S3_PATH_STYLE",
];
const MAX_BYTES: &str = "PALACE_MEDIA_MAX_BYTES";

/// Whether this deployment stores media, decided once at startup.
#[derive(Debug, PartialEq, Eq)]
pub enum MediaConfig {
    /// No S3 setting is present; media endpoints answer as unavailable.
    Unconfigured,
    S3(S3Config),
}

/// How requests address the bucket: MinIO usually needs path style, AWS accepts either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Addressing {
    Path,
    VirtualHosted,
}

/// A complete S3 connection plus the upload bounds that only matter once media is enabled.
#[derive(PartialEq, Eq)]
pub struct S3Config {
    pub endpoint: url::Url,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub addressing: Addressing,
    pub limits: MediaLimits,
}
impl std::fmt::Debug for S3Config {
    /// Keeps the secret out of startup errors and logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("bucket", &self.bucket)
            .field("access_key_id", &self.access_key_id)
            .field("addressing", &self.addressing)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("S3 media storage is partially configured; missing {}", .0.join(", "))]
    Incomplete(Vec<&'static str>),
    #[error("{0} is invalid: {1}")]
    Invalid(&'static str, String),
}

impl MediaConfig {
    /// Reads the media settings through `lookup` so callers decide where values come from and
    /// tests never mutate the process environment. Empty values count as absent.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let read = |name: &str| lookup(name).filter(|value| !value.is_empty());
        let values = S3_VARIABLES.map(|name| (name, read(name)));
        let missing: Vec<_> = values
            .iter()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| *name)
            .collect();
        if missing.len() == S3_VARIABLES.len() {
            return Ok(Self::Unconfigured);
        }
        let [
            (_, Some(endpoint)),
            (_, Some(region)),
            (_, Some(bucket)),
            (_, Some(access_key_id)),
            (_, Some(secret_access_key)),
            (_, Some(path_style)),
        ] = values
        else {
            return Err(ConfigError::Incomplete(missing));
        };
        let endpoint = url::Url::parse(&endpoint)
            .map_err(|error| ConfigError::Invalid("PALACE_S3_ENDPOINT", error.to_string()))?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(ConfigError::Invalid(
                "PALACE_S3_ENDPOINT",
                "expected an http or https URL".into(),
            ));
        }
        let addressing = match path_style.as_str() {
            "true" => Addressing::Path,
            "false" => Addressing::VirtualHosted,
            _ => {
                return Err(ConfigError::Invalid(
                    "PALACE_S3_PATH_STYLE",
                    "expected true or false".into(),
                ));
            }
        };
        let limits = match read(MAX_BYTES) {
            None => MediaLimits::default(),
            Some(value) => MediaLimits {
                max_bytes: value
                    .parse()
                    .ok()
                    .filter(|bytes| *bytes > 0)
                    .ok_or_else(|| {
                        ConfigError::Invalid(MAX_BYTES, "expected a positive byte count".into())
                    })?,
            },
        };
        Ok(Self::S3(S3Config {
            endpoint,
            region,
            bucket,
            access_key_id,
            secret_access_key,
            addressing,
            limits,
        }))
    }
}

impl S3Config {
    /// Builds the client without contacting the server; a wrong bucket or credential surfaces on
    /// the first upload, not at startup.
    pub fn connect(&self) -> Result<S3Bucket, ConfigError> {
        let store = AmazonS3Builder::new()
            .with_endpoint(self.endpoint.as_str().trim_end_matches('/'))
            .with_allow_http(self.endpoint.scheme() == "http")
            .with_region(&self.region)
            .with_bucket_name(&self.bucket)
            .with_access_key_id(&self.access_key_id)
            .with_secret_access_key(&self.secret_access_key)
            .with_virtual_hosted_style_request(self.addressing == Addressing::VirtualHosted)
            // A user waits on every request; the library default retries for minutes.
            .with_retry(RetryConfig {
                max_retries: 2,
                retry_timeout: Duration::from_secs(/*secs*/ 15),
                ..Default::default()
            })
            .build()
            .map_err(|error| ConfigError::Invalid("PALACE_S3_*", error.to_string()))?;
        Ok(Bucket::new(store))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        move |name| map.get(name).cloned()
    }

    const COMPLETE: [(&str, &str); 6] = [
        ("PALACE_S3_ENDPOINT", "http://minio:9000"),
        ("PALACE_S3_REGION", "us-east-1"),
        ("PALACE_S3_BUCKET", "palace"),
        ("PALACE_S3_ACCESS_KEY_ID", "palace"),
        ("PALACE_S3_SECRET_ACCESS_KEY", "secret"),
        ("PALACE_S3_PATH_STYLE", "true"),
    ];

    /// All settings enable S3, none leaves media unconfigured, and any partial set fails startup.
    /// Core test case:
    /// - `specs/test-cases/server/media/media-storage.md#object-storage-configuration-must-be-all-or-nothing`
    #[test]
    fn s3_settings_are_all_or_nothing() {
        assert_eq!(
            MediaConfig::from_lookup(lookup(&COMPLETE)),
            Ok(MediaConfig::S3(S3Config {
                endpoint: url::Url::parse("http://minio:9000").unwrap(),
                region: "us-east-1".into(),
                bucket: "palace".into(),
                access_key_id: "palace".into(),
                secret_access_key: "secret".into(),
                addressing: Addressing::Path,
                limits: MediaLimits::default(),
            }))
        );
        assert_eq!(
            MediaConfig::from_lookup(lookup(&[("PALACE_MEDIA_MAX_BYTES", "10")])),
            Ok(MediaConfig::Unconfigured)
        );
        assert_eq!(
            MediaConfig::from_lookup(lookup(&[
                ("PALACE_S3_BUCKET", "palace"),
                ("PALACE_S3_REGION", ""),
            ])),
            Err(ConfigError::Incomplete(vec![
                "PALACE_S3_ENDPOINT",
                "PALACE_S3_REGION",
                "PALACE_S3_ACCESS_KEY_ID",
                "PALACE_S3_SECRET_ACCESS_KEY",
                "PALACE_S3_PATH_STYLE",
            ]))
        );
    }

    /// Malformed values are named instead of silently falling back to a default.
    #[test]
    fn invalid_values_name_their_variable() {
        let with = |name: &str, value: &str| {
            let mut pairs: Vec<_> = COMPLETE.to_vec();
            pairs.push((name, value));
            pairs.retain(|(key, other)| *key != name || *other == value);
            MediaConfig::from_lookup(lookup(&pairs))
        };
        for (name, value) in [
            ("PALACE_S3_ENDPOINT", "minio:9000"),
            ("PALACE_S3_PATH_STYLE", "yes"),
            ("PALACE_MEDIA_MAX_BYTES", "0"),
            ("PALACE_MEDIA_MAX_BYTES", "big"),
        ] {
            assert!(
                matches!(with(name, value), Err(ConfigError::Invalid(invalid, _)) if invalid == name),
                "{name}={value}"
            );
        }
        let Ok(MediaConfig::S3(config)) = with("PALACE_MEDIA_MAX_BYTES", "1024") else {
            panic!("a positive limit must be accepted");
        };
        assert_eq!(config.limits, MediaLimits { max_bytes: 1024 });
        assert!(!format!("{config:?}").contains("secret"));
    }
}
