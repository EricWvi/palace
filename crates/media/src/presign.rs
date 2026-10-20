//! Presigned GET URLs for reading media straight from object storage (AWS Signature Version 4,
//! query-string form). `object_store` signs with the current time and fixed parameters; Palace
//! needs to choose the signing time, so one media file keeps one URL for a whole hour and the
//! browser cache works, and to set the response type and disposition, so it is written here.
use crate::Addressing;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::fmt::Write;

const ALGORITHM: &str = "AWS4-HMAC-SHA256";
const HOUR_SECONDS: i64 = 60 * 60;
// A URL signed at the top of the hour stays valid for an hour after the hour ends, so even one
// handed out in the hour's last second outlives a page load.
const EXPIRES_SECONDS: i64 = 2 * HOUR_SECONDS;

/// Signs read URLs for one bucket as a browser will reach it.
#[derive(Clone, PartialEq, Eq)]
pub struct Presigner {
    /// The endpoint the browser can reach; the host is part of the signature.
    pub endpoint: url::Url,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub addressing: Addressing,
}
impl std::fmt::Debug for Presigner {
    /// Keeps the secret out of logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presigner")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .finish_non_exhaustive()
    }
}

/// How the browser should treat the bytes it fetches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition<'a> {
    /// Shown in the page: images, video and audio.
    Inline,
    /// Downloaded under this name, so a declared type is never rendered as a page.
    Attachment(&'a str),
}

/// A signed URL and how long a redirect to it may be cached.
#[derive(Debug, PartialEq, Eq)]
pub struct PresignedGet {
    pub url: String,
    /// Seconds until the current signing hour ends, after which a new URL is signed.
    pub max_age: i64,
}

impl Presigner {
    /// Signs a GET of `key` for the hour containing `now` (Unix seconds), asking storage to
    /// answer with `content_type` and `disposition`.
    pub fn presign_get(
        &self,
        key: &str,
        content_type: &str,
        disposition: Disposition,
        now: i64,
    ) -> PresignedGet {
        let hour = now.div_euclid(HOUR_SECONDS) * HOUR_SECONDS;
        let disposition = match disposition {
            Disposition::Inline => "inline".to_owned(),
            Disposition::Attachment(name) => {
                format!(
                    "attachment; filename*=UTF-8''{}",
                    uri_encode(name, Slashes::Encode)
                )
            }
        };
        let url = self.sign(
            key,
            hour,
            EXPIRES_SECONDS,
            &[
                ("response-content-disposition", &disposition),
                ("response-content-type", content_type),
            ],
        );
        PresignedGet {
            url,
            max_age: hour + HOUR_SECONDS - now,
        }
    }

    /// Builds the query-string signature for a GET signed at `signed_at` (Unix seconds).
    fn sign(&self, key: &str, signed_at: i64, expires: i64, extra: &[(&str, &str)]) -> String {
        let (host, path) = self.location(key);
        let timestamp = amz_timestamp(signed_at);
        let day = &timestamp[..8];
        let scope = format!("{day}/{}/s3/aws4_request", self.region);
        let credential = format!("{}/{scope}", self.access_key_id);
        let expires = expires.to_string();
        let mut query: Vec<(String, String)> = [
            ("X-Amz-Algorithm", ALGORITHM),
            ("X-Amz-Credential", credential.as_str()),
            ("X-Amz-Date", timestamp.as_str()),
            ("X-Amz-Expires", expires.as_str()),
            ("X-Amz-SignedHeaders", "host"),
        ]
        .iter()
        .chain(extra)
        .map(|(name, value)| {
            (
                uri_encode(name, Slashes::Encode),
                uri_encode(value, Slashes::Encode),
            )
        })
        .collect();
        query.sort();
        let query = query
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        let canonical = format!("GET\n{path}\n{query}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD");
        let to_sign = format!(
            "{ALGORITHM}\n{timestamp}\n{scope}\n{}",
            hex(&Sha256::digest(canonical))
        );
        let mut signing_key = hmac(
            format!("AWS4{}", self.secret_access_key).as_bytes(),
            day.as_bytes(),
        );
        for part in [self.region.as_str(), "s3", "aws4_request"] {
            signing_key = hmac(&signing_key, part.as_bytes());
        }
        let signature = hex(&hmac(&signing_key, to_sign.as_bytes()));
        format!(
            "{}://{host}{path}?{query}&X-Amz-Signature={signature}",
            self.endpoint.scheme()
        )
    }

    /// Returns the host header and the encoded path for `key` under the configured addressing.
    fn location(&self, key: &str) -> (String, String) {
        let host = self.endpoint.host_str().unwrap_or_default();
        // A default port is absent from the URL and from the host header the browser sends.
        let host = match self.endpoint.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        let base = self.endpoint.path().trim_end_matches('/');
        let key = uri_encode(key, Slashes::Separate);
        match self.addressing {
            Addressing::Path => (
                host,
                format!("{base}/{}/{key}", uri_encode(&self.bucket, Slashes::Encode)),
            ),
            Addressing::VirtualHosted => {
                (format!("{}.{host}", self.bucket), format!("{base}/{key}"))
            }
        }
    }
}

/// Formats Unix seconds as the `YYYYMMDDTHHMMSSZ` timestamp SigV4 signs.
fn amz_timestamp(seconds: i64) -> String {
    let time = time::OffsetDateTime::from_unix_timestamp(seconds)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        time.year(),
        u8::from(time.month()),
        time.day(),
        time.hour(),
        time.minute(),
        time.second()
    )
}

/// What a `/` means in an encoded value.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slashes {
    /// Separates path segments and stays as is.
    Separate,
    /// Is data, like any other reserved character.
    Encode,
}

/// SigV4 URI encoding: everything but unreserved characters is percent-encoded.
fn uri_encode(value: &str, slashes: Slashes) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || b"-_.~".contains(&byte)
            || (byte == b'/' && slashes == Slashes::Separate)
        {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    // HMAC accepts keys of any length, so construction cannot fail.
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap_or_else(|_| unreachable!());
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn aws_example() -> Presigner {
        Presigner {
            endpoint: url::Url::parse("https://s3.amazonaws.com").unwrap(),
            region: "us-east-1".into(),
            bucket: "examplebucket".into(),
            access_key_id: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
            addressing: Addressing::VirtualHosted,
        }
    }

    /// Reproduces the presigned URL example of the S3 SigV4 query-string documentation.
    /// Core test case:
    /// - `specs/test-cases/server/media/media-reading.md#presigned-media-urls-must-be-stable-within-the-hour-and-safe-to-open`
    #[test]
    fn signature_matches_the_aws_example() {
        // 2013-05-24T00:00:00Z
        let url = aws_example().sign("test.txt", 1_369_353_600, 86_400, &[]);
        assert_eq!(
            url,
            "https://examplebucket.s3.amazonaws.com/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20130524%2Fus-east-1%2Fs3%2Faws4_request&X-Amz-Date=20130524T000000Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host&X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"
        );
    }

    /// One media file keeps one URL for the whole hour, and the redirect's cache ends with it.
    /// Core test case:
    /// - `specs/test-cases/server/media/media-reading.md#presigned-media-urls-must-be-stable-within-the-hour-and-safe-to-open`
    #[test]
    fn urls_are_stable_within_the_signing_hour() {
        let signer = Presigner {
            endpoint: url::Url::parse("http://media.example.com:9000").unwrap(),
            addressing: Addressing::Path,
            ..aws_example()
        };
        let at = |now| signer.presign_get("o/m", "image/png", Disposition::Inline, now);
        // 2026-10-20T10:00:00Z, then within the hour, then past it.
        let start = at(1_792_490_400);
        let later = at(1_792_490_400 + 3599);
        let next = at(1_792_490_400 + 3600);
        assert_eq!(later.url, start.url);
        assert_ne!(next.url, start.url);
        assert_eq!(
            (start.max_age, later.max_age, next.max_age),
            (3600, 1, 3600)
        );
        assert!(
            start
                .url
                .starts_with("http://media.example.com:9000/examplebucket/o/m?")
        );
        assert!(
            start
                .url
                .contains("X-Amz-Date=20261020T100000Z&X-Amz-Expires=7200&")
        );
        assert!(
            start.url.contains(
                "&response-content-disposition=inline&response-content-type=image%2Fpng&"
            )
        );
    }

    /// Downloads carry the file name in RFC 6266 form; nothing in it can break the header.
    #[test]
    fn attachments_name_the_file() {
        let url = aws_example()
            .presign_get(
                "o/m",
                "text/html",
                Disposition::Attachment("报告 \"1\".html"),
                0,
            )
            .url;
        assert!(url.contains("response-content-disposition=attachment%3B%20filename%2A%3DUTF-8%27%27%25E6%258A%25A5%25E5%2591%258A%2520%25221%2522.html&"), "{url}");
    }
}
