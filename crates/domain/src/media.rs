use crate::{InputError, InputErrorKind};
use sha2::{Digest, Sha256};

/// Content type recorded when the client declares none, or one that is not `type/subtype`.
pub const FALLBACK_CONTENT_TYPE: &str = "application/octet-stream";
// File names are display metadata; the same bound as other multipart metadata keeps rows small.
const FILE_NAME_BYTES: usize = 1024;

/// Per-deployment bounds on uploaded media.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaLimits {
    /// One file, whether uploaded alone or embedded in an article.
    pub max_bytes: usize,
    /// One whole article import: its body plus every file it embeds.
    pub article_max_bytes: usize,
}
impl Default for MediaLimits {
    /// The single-file limit matches the legacy dashboard editor that media replaces; an
    /// article may embed several such files, or a short video.
    fn default() -> Self {
        Self {
            max_bytes: 50 * 1024 * 1024,
            article_max_bytes: 256 * 1024 * 1024,
        }
    }
}

/// One validated media file. Its digest and size are computed here from the bytes it holds, so
/// the stored metadata can never disagree with the content written to object storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaUpload {
    bytes: Vec<u8>,
    metadata: MediaMetadata,
}
/// What is recorded about an uploaded file; only [`MediaUpload::new`] can produce it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaMetadata {
    content_type: String,
    file_name: Option<String>,
    byte_size: i64,
    sha256: [u8; 32],
}
impl MediaUpload {
    /// Validates a fully received file and its multipart metadata against the deployment limits.
    pub fn new(
        bytes: Vec<u8>,
        content_type: Option<&str>,
        file_name: Option<&str>,
        limits: MediaLimits,
    ) -> Result<Self, InputError> {
        let too_large = || InputError::new(InputErrorKind::Limit, "file", "file too large");
        if bytes.is_empty() {
            return Err(InputError::new(InputErrorKind::Field, "file", "empty file"));
        }
        if bytes.len() > limits.max_bytes {
            return Err(too_large());
        }
        let file_name = file_name.filter(|name| !name.is_empty());
        if file_name.is_some_and(|name| name.len() > FILE_NAME_BYTES) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "file_name",
                "file name too long",
            ));
        }
        let metadata = MediaMetadata {
            content_type: normalize_content_type(content_type),
            file_name: file_name.map(str::to_owned),
            byte_size: i64::try_from(bytes.len()).map_err(|_| too_large())?,
            sha256: Sha256::digest(&bytes).into(),
        };
        Ok(Self { bytes, metadata })
    }
    /// Separates the content handed to storage from the metadata recorded afterwards, without copying.
    pub fn into_parts(self) -> (Vec<u8>, MediaMetadata) {
        (self.bytes, self.metadata)
    }
}
impl MediaMetadata {
    pub fn content_type(&self) -> &str {
        &self.content_type
    }
    pub fn file_name(&self) -> Option<&str> {
        self.file_name.as_deref()
    }
    pub fn byte_size(&self) -> i64 {
        self.byte_size
    }
    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

/// Keeps only a lowercase `type/subtype` essence. Parameters are dropped because nothing reads
/// them yet, and anything that is not a pair of RFC 9110 tokens falls back to an opaque type
/// rather than rejecting the upload: the declared type is a hint, not a validated property.
fn normalize_content_type(declared: Option<&str>) -> String {
    let token = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
    };
    declared
        .and_then(|value| value.split(';').next())
        .map(|essence| essence.trim().to_ascii_lowercase())
        .filter(|essence| {
            essence
                .split_once('/')
                .is_some_and(|(kind, subtype)| token(kind) && token(subtype))
        })
        .unwrap_or_else(|| FALLBACK_CONTENT_TYPE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Bytes, digest and metadata travel together; declared types are reduced to their essence.
    #[test]
    fn upload_keeps_content_with_its_digest_and_normalized_metadata() {
        let upload = MediaUpload::new(
            b"abc".to_vec(),
            Some(" Image/PNG; charset=binary"),
            Some("截图.png"),
            MediaLimits::default(),
        )
        .unwrap();
        assert_eq!(
            upload,
            MediaUpload {
                bytes: b"abc".to_vec(),
                metadata: MediaMetadata {
                    content_type: "image/png".into(),
                    file_name: Some("截图.png".into()),
                    byte_size: 3,
                    // SHA-256("abc") from FIPS 180-2.
                    sha256: [
                        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde,
                        0x5d, 0xae, 0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c,
                        0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
                    ],
                },
            }
        );
    }

    /// Missing or malformed declarations become an opaque type instead of failing the upload.
    #[test]
    fn unusable_content_types_fall_back_to_octet_stream() {
        for declared in [
            None,
            Some(""),
            Some("image"),
            Some("/png"),
            Some("image/"),
            Some("image/p ng"),
            Some("图片/png"),
        ] {
            assert_eq!(normalize_content_type(declared), FALLBACK_CONTENT_TYPE);
        }
        assert_eq!(
            normalize_content_type(Some("application/vnd.api+json")),
            "application/vnd.api+json"
        );
    }

    /// The size limit is inclusive; empty files and oversized names are refused with their field.
    /// Core test case:
    /// - `specs/test-cases/server/media/media-storage.md#upload-limits-must-be-enforced-by-the-media-route-itself`
    #[test]
    fn upload_bounds_are_checked_per_field() {
        let limits = MediaLimits {
            max_bytes: 3,
            ..MediaLimits::default()
        };
        assert!(MediaUpload::new(b"abc".to_vec(), None, None, limits).is_ok());
        assert_eq!(
            MediaUpload::new(b"abcd".to_vec(), None, None, limits),
            Err(InputError::new(
                InputErrorKind::Limit,
                "file",
                "file too large"
            ))
        );
        assert_eq!(
            MediaUpload::new(Vec::new(), None, None, limits),
            Err(InputError::new(InputErrorKind::Field, "file", "empty file"))
        );
        assert_eq!(
            MediaUpload::new(b"a".to_vec(), None, Some(&"名".repeat(342)), limits),
            Err(InputError::new(
                InputErrorKind::Field,
                "file_name",
                "file name too long"
            ))
        );
        let (_, unnamed) = MediaUpload::new(b"a".to_vec(), None, Some(""), limits)
            .unwrap()
            .into_parts();
        assert_eq!(unnamed.file_name(), None);
    }
}
