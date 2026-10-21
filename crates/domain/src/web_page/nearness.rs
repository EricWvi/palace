//! How close an article's address is to the page being bound, so candidates on the same site
//! and section come first.

/// Ranks `url` against `near`: 0 when the hosts differ (ignoring a leading `www.` and case) or
/// either is not a URL, otherwise 1 plus the number of leading path segments the two share.
/// Higher is nearer. Empty segments are skipped, so a trailing slash changes nothing.
pub fn address_nearness(url: &str, near: &str) -> u32 {
    let (Ok(url), Ok(near)) = (url::Url::parse(url), url::Url::parse(near)) else {
        return 0;
    };
    // The URL parser already lowercases the host of http and https addresses.
    let host = |url: &url::Url| {
        url.host_str().map(|host| {
            host.strip_prefix("www.")
                .unwrap_or(host)
                .to_ascii_lowercase()
        })
    };
    if host(&url).is_none() || host(&url) != host(&near) {
        return 0;
    }
    let segments = |url: &url::Url| {
        url.path_segments()
            .map(|segments| {
                segments
                    .filter(|segment| !segment.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let shared = segments(&url)
        .iter()
        .zip(segments(&near).iter())
        .take_while(|(a, b)| a == b)
        .count();
    1 + u32::try_from(shared).unwrap_or(u32::MAX - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Same host first, then the longer shared path, with `www.`, case and trailing slashes
    /// ignored and the query never compared.
    /// Core test case:
    /// - `specs/test-cases/extension/web-annotations.md#candidate-articles-must-rank-the-nearest-address-first`
    #[test]
    fn nearness_counts_host_then_shared_segments() {
        let near = "https://www.Example.com/blog/2026/post?utm_source=x";
        let ranks: Vec<(&str, u32)> = [
            "https://example.com/blog/2026/post",
            "http://EXAMPLE.com/blog/2026/other/",
            "https://example.com/blog/2025/post",
            "https://www.example.com/",
            "https://example.com/news/2026/post",
            "https://other.com/blog/2026/post",
            "https://sub.example.com/blog/2026/post",
            "not a url",
        ]
        .into_iter()
        .map(|url| (url, address_nearness(url, near)))
        .collect();
        assert_eq!(
            ranks,
            vec![
                ("https://example.com/blog/2026/post", 4),
                ("http://EXAMPLE.com/blog/2026/other/", 3),
                ("https://example.com/blog/2025/post", 2),
                ("https://www.example.com/", 1),
                ("https://example.com/news/2026/post", 1),
                ("https://other.com/blog/2026/post", 0),
                ("https://sub.example.com/blog/2026/post", 0),
                ("not a url", 0),
            ]
        );
    }
}
