//! 摘星 articles: web articles written by others and kept for good. The fields an importer
//! sends are validated here; the body and its `media:` references live in [`body`].
mod body;

pub use body::{ArticleBody, MediaName};

use crate::{DocumentDate, InputError, InputErrorKind, validate_title};
use chrono_tz::Tz;

// A browser handles URLs far longer than this, but a saved article's address never needs to be.
const URL_BYTES: usize = 2048;
const AUTHOR_BYTES: usize = 256;

/// The fields an importer sends besides the body, before validation.
pub struct ArticleMetadataInput {
    pub title: String,
    pub url: String,
    pub author: Option<String>,
    pub date: String,
}

/// Everything about an article except its body; the part that may be edited after import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArticleMetadata {
    title: String,
    url: String,
    author: Option<String>,
    published: DocumentDate,
}

impl ArticleMetadata {
    /// Validates every field; a bare publish day is read in `timezone`. Title and author keep
    /// their original spelling, and a blank author means the article names none.
    pub fn parse(input: ArticleMetadataInput, timezone: Tz) -> Result<Self, InputError> {
        validate_title(&input.title)?;
        // The URL is the article's identity within an owner, so it is kept byte for byte and
        // never normalized: whatever the importer sends again must match it.
        validate_web_url(&input.url, "url")?;
        let author = input.author.filter(|author| !author.trim().is_empty());
        if author
            .as_ref()
            .is_some_and(|author| author.len() > AUTHOR_BYTES)
        {
            return Err(InputError::new(
                InputErrorKind::Field,
                "author",
                "expected an author of at most 256 bytes",
            ));
        }
        let published = DocumentDate::parse(&input.date, timezone).ok_or_else(|| {
            InputError::new(
                InputErrorKind::Field,
                "date",
                "expected YYYY-MM-DD or an RFC 3339 time with an offset",
            )
        })?;
        Ok(Self {
            title: input.title,
            url: input.url,
            author,
            published,
        })
    }

    /// Rebuilds metadata the database already holds; it was validated when it was written.
    pub fn from_stored(
        title: String,
        url: String,
        author: Option<String>,
        published: DocumentDate,
    ) -> Self {
        Self {
            title,
            url,
            author,
            published,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }
    pub fn published(&self) -> DocumentDate {
        self.published
    }
}

/// Accepts an absolute `http`/`https` URL of at most 2048 bytes with no surrounding blanks.
/// Articles and annotated web pages share it, since a page is bound to an article by address.
pub(crate) fn validate_web_url(url: &str, field: &str) -> Result<(), InputError> {
    let invalid = || {
        InputError::new(
            InputErrorKind::Field,
            field,
            "expected an absolute http or https URL of at most 2048 bytes",
        )
    };
    if url.len() > URL_BYTES || url.trim() != url {
        return Err(invalid());
    }
    let parsed = url::Url::parse(url).map_err(|_| invalid())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(invalid());
    }
    Ok(())
}

/// Names the site an article came from: the URL's host without a leading `www.`. Lists show it
/// where an article names no author.
pub fn article_site(url: &str) -> String {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_default();
    host.strip_prefix("www.").map(str::to_owned).unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatePrecision;
    use pretty_assertions::assert_eq;

    const SHANGHAI: Tz = chrono_tz::Asia::Shanghai;

    fn input(url: &str, author: Option<&str>, date: &str) -> ArticleMetadataInput {
        ArticleMetadataInput {
            title: "  标题 ".into(),
            url: url.into(),
            author: author.map(str::to_owned),
            date: date.into(),
        }
    }

    /// Valid fields are kept as sent, a blank author is none, and the date keeps its precision.
    /// Core test case:
    /// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
    #[test]
    fn metadata_keeps_fields_as_sent() {
        assert_eq!(
            ArticleMetadata::parse(
                input("https://lwn.net/Articles/1/", Some(" \t"), "2026-10-05"),
                SHANGHAI
            ),
            Ok(ArticleMetadata {
                title: "  标题 ".into(),
                url: "https://lwn.net/Articles/1/".into(),
                author: None,
                published: DocumentDate {
                    at: 1_791_129_600_000,
                    precision: DatePrecision::Day,
                },
            })
        );
        let named = ArticleMetadata::parse(
            input(
                "http://example.com/a",
                Some(" 某人 "),
                "2026-10-05T08:00:00+08:00",
            ),
            SHANGHAI,
        )
        .unwrap();
        assert_eq!(named.author(), Some(" 某人 "));
        assert_eq!(named.published().precision, DatePrecision::Instant);
    }

    /// Each invalid field is reported under its own name.
    /// Core test case:
    /// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
    #[test]
    fn invalid_fields_are_named() {
        let long_url = format!("https://example.com/{}", "a".repeat(2048));
        for (bad, field) in [
            (input("ftp://example.com/a", None, "2026-10-05"), "url"),
            (input("/relative", None, "2026-10-05"), "url"),
            (input(" https://example.com", None, "2026-10-05"), "url"),
            (input(&long_url, None, "2026-10-05"), "url"),
            (
                input("https://example.com", Some(&"名".repeat(86)), "2026-10-05"),
                "author",
            ),
            (input("https://example.com", None, "10/05/2026"), "date"),
            (
                ArticleMetadataInput {
                    title: " ".into(),
                    ..input("https://example.com", None, "2026-10-05")
                },
                "title",
            ),
        ] {
            assert_eq!(
                ArticleMetadata::parse(bad, SHANGHAI).map_err(|error| error.path),
                Err(field.to_owned())
            );
        }
    }

    /// The site is the host without a leading `www.`.
    #[test]
    fn site_is_the_host_without_www() {
        assert_eq!(article_site("https://www.lwn.net/Articles/1/"), "lwn.net");
        assert_eq!(
            article_site("http://blog.example.com:8080/x"),
            "blog.example.com"
        );
    }
}
