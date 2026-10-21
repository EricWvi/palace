//! Web pages the reader annotated in the browser extension, before or after binding them to an
//! imported article. A page is identified by its address; its annotations anchor by quote and
//! surrounding text, because offsets into a live page mean nothing once the page changes.
mod nearness;

pub use nearness::address_nearness;

use crate::{InputError, InputErrorKind, annotation::QUOTE_BYTES, article::validate_web_url};
use uuid::Uuid;

// Same as an article title: the tab title stands in for one in the extension's list.
const TITLE_BYTES: usize = 1024;
// Enough surrounding text to tell repeated quotes apart, short enough to survive small edits
// of the page around them. Counted in UTF-16 code units, as the browser slices strings.
const CONTEXT_UNITS: usize = 32;

/// A page address as the browser shows it, without the fragment; never normalized further,
/// so each address the reader visits is its own page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebPageUrl(String);

impl WebPageUrl {
    /// Accepts the rules of an article URL; a fragment is refused rather than stripped, so a
    /// client that forgot to strip it cannot create a second page for the same address.
    pub fn parse(url: String) -> Result<Self, InputError> {
        validate_web_url(&url, "url")?;
        if url.contains('#') {
            return Err(InputError::new(
                InputErrorKind::Field,
                "url",
                "expected the page address without its fragment",
            ));
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The tab title recorded for the extension's list. It is whatever the site set, so instead of
/// refusing an odd one, a blank title falls back to the address and a long one is cut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebPageTitle(String);

impl WebPageTitle {
    pub fn new(title: &str, url: &WebPageUrl) -> Self {
        let title = match title.trim() {
            "" => url.as_str(),
            trimmed => trimmed,
        };
        // Cut on a character boundary at or below the byte limit.
        let mut end = title.len().min(TITLE_BYTES);
        while !title.is_char_boundary(end) {
            end -= 1;
        }
        Self(title[..end].to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether a page still takes annotations of its own, or has been handed to an article.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebPageState {
    Unbound,
    Bound(Uuid),
}

/// Where a web annotation sits on its page: the W3C text quote selector (the quote with the text
/// just before and after it) plus where the quote starts as a fraction of the page text.
#[derive(Clone, Debug, PartialEq)]
pub struct WebAnchor {
    quote: String,
    prefix: String,
    suffix: String,
    position: f64,
}

impl WebAnchor {
    /// Validates each part; the quote follows the article annotation's limit so a migrated one
    /// always fits.
    pub fn parse(
        quote: String,
        prefix: String,
        suffix: String,
        position: f64,
    ) -> Result<Self, InputError> {
        if quote.trim().is_empty() || quote.len() > QUOTE_BYTES {
            return Err(InputError::new(
                InputErrorKind::Field,
                "quote",
                "expected nonblank marked text of at most 16 KiB",
            ));
        }
        for (field, text) in [("prefix", &prefix), ("suffix", &suffix)] {
            if text.encode_utf16().count() > CONTEXT_UNITS {
                return Err(InputError::new(
                    InputErrorKind::Field,
                    field,
                    "expected at most 32 UTF-16 code units of surrounding text",
                ));
            }
        }
        if !(0.0..=1.0).contains(&position) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "position",
                "expected a fraction between 0 and 1",
            ));
        }
        Ok(Self {
            quote,
            prefix,
            suffix,
            position,
        })
    }

    /// Rebuilds an anchor the database already holds; it was validated when it was written.
    pub fn from_stored(quote: String, prefix: String, suffix: String, position: f64) -> Self {
        Self {
            quote,
            prefix,
            suffix,
            position,
        }
    }

    pub fn quote(&self) -> &str {
        &self.quote
    }
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
    pub fn suffix(&self) -> &str {
        &self.suffix
    }
    pub fn position(&self) -> f64 {
        self.position
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn url(text: &str) -> WebPageUrl {
        WebPageUrl::parse(text.into()).unwrap()
    }

    /// Addresses follow the article URL rules and arrive without a fragment.
    /// Core test case:
    /// - `specs/test-cases/extension/web-annotations.md#a-bound-web-page-must-reject-new-and-changed-web-annotations`
    #[test]
    fn addresses_are_absolute_without_fragment() {
        assert_eq!(
            WebPageUrl::parse("https://example.com/a?utm=1".into()),
            Ok(WebPageUrl("https://example.com/a?utm=1".into()))
        );
        for text in [
            "https://example.com/a#part",
            "ftp://example.com/a",
            "/relative",
            " https://example.com/a",
        ] {
            assert_eq!(
                WebPageUrl::parse(text.into()).map_err(|error| error.path),
                Err("url".to_owned())
            );
        }
    }

    /// A blank tab title shows the address, and a long one is cut on a character boundary.
    /// Core test case:
    /// - `specs/test-cases/extension/web-annotations.md#a-bound-web-page-must-reject-new-and-changed-web-annotations`
    #[test]
    fn titles_fall_back_to_the_address_and_fit_the_limit() {
        let address = url("https://example.com/a");
        assert_eq!(
            WebPageTitle::new("  ", &address),
            WebPageTitle("https://example.com/a".into())
        );
        assert_eq!(
            WebPageTitle::new(" 标题 ", &address),
            WebPageTitle("标题".into())
        );
        // 342 three-byte characters are 1026 bytes; 341 of them fit.
        assert_eq!(
            WebPageTitle::new(&"字".repeat(342), &address),
            WebPageTitle("字".repeat(341))
        );
    }

    /// Each part of the selector is checked on its own, and context counts UTF-16 code units.
    /// Core test case:
    /// - `specs/test-cases/extension/web-annotations.md#a-bound-web-page-must-reject-new-and-changed-web-annotations`
    #[test]
    fn anchors_check_quote_context_and_position() {
        assert_eq!(
            WebAnchor::parse("引文".into(), String::new(), "后文".into(), 1.0),
            Ok(WebAnchor {
                quote: "引文".into(),
                prefix: String::new(),
                suffix: "后文".into(),
                position: 1.0,
            })
        );
        let long_quote = "a".repeat(QUOTE_BYTES + 1);
        // 16 emoji are 32 UTF-16 code units; 17 are 34.
        let fits = "😀".repeat(16);
        let overflows = "😀".repeat(17);
        assert!(WebAnchor::parse("q".into(), fits.clone(), fits, 0.5).is_ok());
        for (quote, prefix, suffix, position, field) in [
            (" \n", "", "", 0.5, "quote"),
            (long_quote.as_str(), "", "", 0.5, "quote"),
            ("q", overflows.as_str(), "", 0.5, "prefix"),
            ("q", "", overflows.as_str(), 0.5, "suffix"),
            ("q", "", "", -0.1, "position"),
            ("q", "", "", 1.5, "position"),
            ("q", "", "", f64::NAN, "position"),
        ] {
            assert_eq!(
                WebAnchor::parse(quote.into(), prefix.into(), suffix.into(), position)
                    .map_err(|error| error.path),
                Err(field.to_owned())
            );
        }
    }
}
