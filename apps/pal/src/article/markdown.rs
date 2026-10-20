//! Converts only inline image destinations, retaining the source spelling everywhere else.
use palace_domain::MediaName;
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
use std::collections::BTreeSet;

/// Uses parser offsets to distinguish image destinations from identical code or alt text.
pub(super) fn convert(text: &str) -> Result<(String, BTreeSet<String>), String> {
    if text.len() > 4 * 1024 * 1024 {
        return Err("content: body exceeds 4 MiB".into());
    }
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut open: Vec<(String, LinkType, usize)> = Vec::new();
    let mut replacements = Vec::new();
    let mut names = BTreeSet::new();
    for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
        let closed = match event {
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                ..
            }) => {
                extend_alt(&mut open, range.start);
                open.push((dest_url.into_string(), link_type, range.start + 2));
                None
            }
            Event::End(TagEnd::Image) => {
                let closed = open.pop();
                extend_alt(&mut open, range.end);
                closed
            }
            _ => {
                extend_alt(&mut open, range.end);
                None
            }
        };
        let Some((destination, link_type, alt_end)) = closed else {
            continue;
        };
        if destination.starts_with("http://")
            || destination.starts_with("https://")
            || destination.starts_with("//")
            || destination.starts_with("data:")
        {
            continue;
        }
        if link_type != LinkType::Inline {
            return Err(format!(
                "content: {destination:?} must use an inline image ![alt](assets/name)"
            ));
        }
        let name = destination
            .strip_prefix("assets/")
            .or_else(|| destination.strip_prefix("./assets/"))
            .ok_or_else(|| {
                format!(
                    "content: unsupported image address {destination:?}; expected assets/<filename>"
                )
            })?;
        MediaName::parse(name).map_err(|error| format!("content: {destination:?}: {error}"))?;
        if matches!(name, "." | "..") {
            return Err(format!(
                "content: unsupported image address {destination:?}"
            ));
        }
        // Offset lookup is limited to the destination region, never the alt text or title.
        let offset = text[alt_end..range.end]
            .find(&destination)
            .ok_or_else(|| format!("content: cannot locate image address {destination:?}"))?;
        let start = alt_end + offset;
        replacements.push((start..start + destination.len(), format!("media:{name}")));
        names.insert(name.to_owned());
    }
    replacements.sort_by_key(|(range, _)| range.start);
    let mut result = String::with_capacity(text.len());
    let mut copied = 0;
    for (range, replacement) in replacements {
        result.push_str(&text[copied..range.start]);
        result.push_str(&replacement);
        copied = range.end;
    }
    result.push_str(&text[copied..]);
    Ok((result, names))
}

/// Nested alt text contributes to every enclosing image's destination boundary.
fn extend_alt(open: &mut [(String, LinkType, usize)], end: usize) {
    for (_, _, alt_end) in open {
        *alt_end = (*alt_end).max(end);
    }
}
