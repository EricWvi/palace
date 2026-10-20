//! An article body and the files it embeds. An importer cannot know the ids Palace will give
//! its files, so the body names each file as `media:<name>`; once the files are stored, every
//! such reference is replaced by the file's `/api/media/<id>` and nothing else in the body moves.
use crate::{InputError, InputErrorKind};
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
use std::{
    collections::{BTreeSet, HashMap},
    ops::Range,
};
use uuid::Uuid;

const BODY_BYTES: usize = 4 * 1024 * 1024;
const NAME_BYTES: usize = 128;
const SCHEME: &str = "media:";

/// The name an import gives one of its files: the part's filename and the `<name>` in
/// `media:<name>`. The narrow alphabet keeps the reference free of anything Markdown escapes or
/// URL-encodes, so the name in the body and the filename always compare byte for byte.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MediaName(String);

impl MediaName {
    /// Accepts 1 to 128 characters from `[A-Za-z0-9._-]`.
    pub fn parse(name: &str) -> Result<Self, InputError> {
        let valid = !name.is_empty()
            && name.len() <= NAME_BYTES
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte));
        if !valid {
            return Err(InputError::new(
                InputErrorKind::Field,
                "file",
                format!("media name {name:?} must be 1-128 characters of A-Z a-z 0-9 . _ -"),
            ));
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated body with the byte range of every `media:` reference in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArticleBody {
    text: String,
    references: Vec<(MediaName, Range<usize>)>,
}

impl ArticleBody {
    /// Checks the size and finds every image whose address is `media:<name>`. Only inline image
    /// syntax counts, as in the rendered page: text in code spans and code blocks is not an
    /// image, and links are not embeds. A reference-style image cannot point at a file, because
    /// its address sits in a definition elsewhere that other links may share.
    pub fn parse(text: String) -> Result<Self, InputError> {
        if text.trim().is_empty() {
            return Err(InputError::new(
                InputErrorKind::Field,
                "content",
                "expected a nonempty body",
            ));
        }
        if text.len() > BODY_BYTES {
            return Err(InputError::new(
                InputErrorKind::Limit,
                "content",
                "body exceeds 4 MiB",
            ));
        }
        // The same extensions as the excerpt and the web renderer, so all three agree on what
        // is an image.
        let options =
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        let mut references = Vec::new();
        // Images open around the current event (an alt text may hold another image), each with
        // its address and where its alt text ends so far.
        let mut open: Vec<(String, LinkType, usize)> = Vec::new();
        for (event, range) in Parser::new_ext(&text, options).into_offset_iter() {
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
            if let Some((dest, link_type, alt_end)) = closed
                && let Some(name) = dest.strip_prefix(SCHEME)
            {
                if !matches!(link_type, LinkType::Inline) {
                    return Err(InputError::new(
                        InputErrorKind::Field,
                        "content",
                        format!("{dest} must be an inline image ![alt]({dest})"),
                    ));
                }
                let name = MediaName::parse(name)?;
                // The address follows the alt text: past the last event inside the image, so a
                // name repeated in the alt text or the title is never taken for it.
                let offset = text[alt_end..range.end].find(&dest).ok_or_else(|| {
                    InputError::new(InputErrorKind::Field, "content", "unreadable image")
                })?;
                let start = alt_end + offset;
                references.push((name, start..start + dest.len()));
            }
        }
        Ok(Self { text, references })
    }

    /// The distinct file names the body embeds.
    pub fn names(&self) -> BTreeSet<&MediaName> {
        self.references.iter().map(|(name, _)| name).collect()
    }

    /// Replaces each reference with the stored file's address. The files must be exactly the
    /// ones the body embeds: a file the body never shows would be kept with nothing to show it.
    pub fn resolve(self, files: &HashMap<MediaName, Uuid>) -> Result<String, InputError> {
        let embedded = self.names();
        if let Some(missing) = embedded.iter().find(|name| !files.contains_key(**name)) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "content",
                format!("{SCHEME}{} has no file", missing.as_str()),
            ));
        }
        if let Some(unused) = files.keys().find(|name| !embedded.contains(name)) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "file",
                format!("{} is not embedded in the body", unused.as_str()),
            ));
        }
        let mut resolved = String::with_capacity(self.text.len());
        let mut copied = 0;
        for (name, range) in &self.references {
            resolved.push_str(&self.text[copied..range.start]);
            resolved.push_str(&format!("/api/media/{}", files[name]));
            copied = range.end;
        }
        resolved.push_str(&self.text[copied..]);
        Ok(resolved)
    }
}

/// Moves the end of every open alt text past `end`; an event inside an image belongs to the
/// alt text of each image around it.
fn extend_alt(open: &mut [(String, LinkType, usize)], end: usize) {
    for (_, _, alt_end) in open {
        *alt_end = (*alt_end).max(end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn files(names: &[&str]) -> HashMap<MediaName, Uuid> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    MediaName::parse(name).unwrap(),
                    Uuid::from_u128(index as u128 + 1),
                )
            })
            .collect()
    }

    /// Only the addresses of inline images change; code, links, alt text and titles that spell
    /// the same reference keep every byte.
    /// Core test case:
    /// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
    #[test]
    fn resolving_rewrites_only_image_addresses() {
        let body = "# 标题\n\n![media:a.png](media:a.png \"media:a.png\")\n\n`![](media:b.mp4)`\n\n```\n![](media:a.png)\n```\n\n[链接](media:a.png) ![*视频*](<media:b.mp4>)\n";
        let resolved = ArticleBody::parse(body.into())
            .unwrap()
            .resolve(&files(&["a.png", "b.mp4"]))
            .unwrap();
        assert_eq!(
            resolved,
            "# 标题\n\n![media:a.png](/api/media/00000000-0000-0000-0000-000000000001 \"media:a.png\")\n\n`![](media:b.mp4)`\n\n```\n![](media:a.png)\n```\n\n[链接](media:a.png) ![*视频*](</api/media/00000000-0000-0000-0000-000000000002>)\n"
        );
    }

    /// Files and references must match one to one; names outside the alphabet and
    /// reference-style images are refused.
    /// Core test case:
    /// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
    #[test]
    fn references_and_files_must_match() {
        let refused = |body: &str, names: &[&str]| {
            ArticleBody::parse(body.into())
                .and_then(|body| body.resolve(&files(names)))
                .map_err(|error| error.path)
        };
        assert_eq!(refused("![](media:a.png)", &[]), Err("content".to_owned()));
        assert_eq!(
            refused("![](media:a.png)", &["a.png", "b.png"]),
            Err("file".to_owned())
        );
        assert_eq!(refused("![](media:a%20b.png)", &[]), Err("file".to_owned()));
        assert_eq!(
            refused("![x][r]\n\n[r]: media:a.png\n", &["a.png"]),
            Err("content".to_owned())
        );
        assert_eq!(refused(" \n", &[]), Err("content".to_owned()));
        assert_eq!(refused("纯文字", &[]), Ok("纯文字".to_owned()));
    }

    /// Names are 1 to 128 characters from a fixed alphabet.
    #[test]
    fn media_names_use_a_fixed_alphabet() {
        for valid in ["a", "cover.jpg", "A_b-0.MP4", &"a".repeat(128)] {
            assert!(MediaName::parse(valid).is_ok(), "{valid}");
        }
        for invalid in ["", "a b", "图.png", "a/b", &"a".repeat(129)] {
            assert!(MediaName::parse(invalid).is_err(), "{invalid}");
        }
    }
}
