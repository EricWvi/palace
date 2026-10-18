//! Where `pal` sends requests and with which credential. Everything comes from the environment
//! and is validated before any request, so a misconfiguration never puts the token on the wire.
use palace_domain::AccessTokenSecret;
use reqwest::header::HeaderValue;
use std::{ffi::OsString, net::IpAddr};
use url::{Host, Url};

pub(crate) const URL_VAR: &str = "PAL_URL";
pub(crate) const TOKEN_VAR: &str = "PAL_TOKEN";

/// A validated Palace address and the `Authorization` value for its access token.
#[derive(Debug)]
pub(crate) struct Config {
    base: Url,
    /// Marked sensitive so `Debug` output and reqwest's own diagnostics never show the token.
    authorization: HeaderValue,
}

/// Why the environment cannot be used. No variant carries the token's value.
#[derive(Debug, PartialEq, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("{0} is not set")]
    Missing(&'static str),
    #[error("{URL_VAR} is not a URL: {0}")]
    MalformedUrl(url::ParseError),
    #[error("{URL_VAR} must be an http(s) URL with a host and no credentials, query or fragment")]
    UnsupportedUrl,
    #[error("{URL_VAR} must use https; plain http is allowed only for loopback hosts")]
    InsecureUrl,
    #[error("{TOKEN_VAR} is not a Palace access token (palace_pat_ followed by 43 characters)")]
    InvalidToken,
}

impl Config {
    /// Reads `PAL_URL` and `PAL_TOKEN` through `var`, which tests replace instead of mutating the
    /// process environment. Empty values count as unset so `PAL_TOKEN= pal …` fails loudly.
    pub(crate) fn from_env(
        var: impl Fn(&'static str) -> Option<OsString>,
    ) -> Result<Self, ConfigError> {
        let present = |name: &'static str| {
            var(name)
                .filter(|value| !value.is_empty())
                .ok_or(ConfigError::Missing(name))
        };
        let base = present(URL_VAR)?
            .into_string()
            .map_err(|_| ConfigError::UnsupportedUrl)?;
        let base = Url::parse(&base).map_err(ConfigError::MalformedUrl)?;
        // Credentials in the URL would travel as a second Authorization source, and a query or
        // fragment has no meaning once API path segments are appended.
        if !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err(ConfigError::UnsupportedUrl);
        }
        let loopback = match base.host() {
            Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
            Some(Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
            Some(Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
            None => return Err(ConfigError::UnsupportedUrl),
        };
        match base.scheme() {
            "https" => {}
            "http" if loopback => {}
            "http" => return Err(ConfigError::InsecureUrl),
            _ => return Err(ConfigError::UnsupportedUrl),
        }
        // The server's own parser decides the format, so a truncated paste fails here rather
        // than as an opaque 401.
        let token = present(TOKEN_VAR)?
            .into_string()
            .ok()
            .and_then(|token| AccessTokenSecret::parse(&token))
            .ok_or(ConfigError::InvalidToken)?;
        let mut authorization = HeaderValue::from_str(&format!("Bearer {}", token.expose()))
            .map_err(|_| ConfigError::InvalidToken)?;
        authorization.set_sensitive(true);
        Ok(Self {
            base,
            authorization,
        })
    }

    /// Appends API path segments to the base, keeping any path prefix such as `/palace`.
    pub(crate) fn endpoint(&self, segments: &[&str]) -> Url {
        let mut url = self.base.clone();
        // An http(s) URL with a host always has path segments; the check is for the type only.
        if let Ok(mut path) = url.path_segments_mut() {
            path.pop_if_empty().extend(segments);
        }
        url
    }

    pub(crate) fn authorization(&self) -> &HeaderValue {
        &self.authorization
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use pretty_assertions::assert_eq;

    const TOKEN: &str = "palace_pat_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    /// Builds a config from fixed variables without touching the process environment.
    fn config(url: Option<&str>, token: Option<&str>) -> Result<Config, ConfigError> {
        Config::from_env(|name| match name {
            URL_VAR => url.map(OsString::from),
            TOKEN_VAR => token.map(OsString::from),
            _ => None,
        })
    }

    /// Core test case:
    /// - `specs/test-cases/cli/media-commands.md#the-access-token-must-reach-only-the-configured-palace-host`
    #[test]
    fn rejects_unsafe_configuration_without_echoing_the_token() {
        let truncated = &TOKEN[..TOKEN.len() - 1];
        let cases = [
            (None, Some(TOKEN), ConfigError::Missing(URL_VAR)),
            (Some(""), Some(TOKEN), ConfigError::Missing(URL_VAR)),
            (
                Some("https://palace.test"),
                None,
                ConfigError::Missing(TOKEN_VAR),
            ),
            (
                Some("https://palace.test"),
                Some(""),
                ConfigError::Missing(TOKEN_VAR),
            ),
            (
                Some("palace.test"),
                Some(TOKEN),
                ConfigError::MalformedUrl(url::ParseError::RelativeUrlWithoutBase),
            ),
            (
                Some("http://palace.test"),
                Some(TOKEN),
                ConfigError::InsecureUrl,
            ),
            (
                Some("http://10.0.0.1:8080"),
                Some(TOKEN),
                ConfigError::InsecureUrl,
            ),
            (
                Some("ftp://palace.test"),
                Some(TOKEN),
                ConfigError::UnsupportedUrl,
            ),
            (
                Some("https://u:p@palace.test"),
                Some(TOKEN),
                ConfigError::UnsupportedUrl,
            ),
            (
                Some("https://palace.test/?a=1"),
                Some(TOKEN),
                ConfigError::UnsupportedUrl,
            ),
            (
                Some("https://palace.test/#a"),
                Some(TOKEN),
                ConfigError::UnsupportedUrl,
            ),
            (
                Some("https://palace.test"),
                Some(truncated),
                ConfigError::InvalidToken,
            ),
            (
                Some("https://palace.test"),
                Some("ghp_secret"),
                ConfigError::InvalidToken,
            ),
        ];
        for (url, token, expected) in cases {
            let error = config(url, token).unwrap_err();
            assert!(!error.to_string().contains(&TOKEN[11..]), "{error}");
            assert_eq!(error, expected, "{url:?}");
        }
    }

    #[test]
    fn accepts_https_and_loopback_http() {
        for url in [
            "https://palace.test",
            "http://localhost:8080",
            "http://127.0.0.1:8080",
            "http://127.1.2.3",
            "http://[::1]:8080",
        ] {
            let config = config(Some(url), Some(TOKEN)).unwrap();
            assert_eq!(
                config.authorization().to_str().unwrap(),
                format!("Bearer {TOKEN}")
            );
            assert!(config.authorization().is_sensitive());
            assert!(!format!("{config:?}").contains(TOKEN), "{url}");
        }
    }

    #[test]
    fn endpoints_keep_the_path_prefix() {
        for (base, expected) in [
            ("https://palace.test", "https://palace.test/api/media/x"),
            ("https://palace.test/", "https://palace.test/api/media/x"),
            (
                "https://palace.test/palace",
                "https://palace.test/palace/api/media/x",
            ),
            (
                "https://palace.test/palace/",
                "https://palace.test/palace/api/media/x",
            ),
        ] {
            let config = config(Some(base), Some(TOKEN)).unwrap();
            assert_eq!(config.endpoint(&["api", "media", "x"]).as_str(), expected);
        }
    }
}
