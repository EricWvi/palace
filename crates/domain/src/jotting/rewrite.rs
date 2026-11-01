//! Rewriting a jotting file edited on the web. Only `title`, `tags` and the body change; every
//! other byte of the file stays as the agent wrote it, so the diff the agent reviews before
//! committing shows only the web's edit.
//! See `specs/decisions/server/jotting/20261101-edit-and-delete-jottings-from-the-web-in-the-work-tree-until-committed.md` D2.
use super::Jotting;
use yaml_rust2::{Yaml, YamlLoader};

impl Jotting {
    /// Puts this jotting's title, tags and body into `file`, an existing file of the same
    /// jotting. `None` when `file` has no frontmatter or the result would not parse back to
    /// exactly this jotting, for instance because the file's `id` or `created` differ.
    pub fn rewrite(&self, file: &str) -> Option<String> {
        let (bom, text) = match file.strip_prefix('\u{feff}') {
            Some(text) => ("\u{feff}", text),
            None => ("", file),
        };
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        if lines.first()?.trim_end_matches(['\r', '\n']) != "---" {
            return None;
        }
        let close = lines
            .iter()
            .skip(1)
            .position(|line| matches!(line.trim_end_matches(['\r', '\n']), "---" | "..."))?
            + 1;
        let newline = if lines[0].ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let title = self
            .title
            .as_ref()
            .map(|title| format!("title: {}{newline}", scalar(title, Context::Block)));
        let tags = (!self.tags.is_empty()).then(|| {
            let tags: Vec<String> = self
                .tags
                .iter()
                .map(|tag| scalar(tag, Context::Flow))
                .collect();
            format!("tags: [{}]{newline}", tags.join(", "))
        });

        let has = |name: &str| lines[1..close].iter().any(|line| key(line) == Some(name));
        let (had_title, had_tags) = (has("title"), has("tags"));
        let mut head = vec![lines[0].to_owned()];
        let (mut title, mut tags) = (title, tags);
        let mut index = 1;
        while index < close {
            let end = block_end(&lines, index, close);
            let block = lines[index..end].iter().map(|line| (*line).to_owned());
            match key(lines[index]) {
                // A field this edit owns is replaced where it stands, or dropped when cleared; a
                // new `tags` follows the title.
                Some("title") => {
                    head.extend(title.take());
                    if !had_tags {
                        head.extend(tags.take());
                    }
                }
                Some("tags") => head.extend(tags.take()),
                // New fields go right after `created`.
                Some("created") => {
                    head.extend(block);
                    if !had_title {
                        head.extend(title.take());
                        if !had_tags {
                            head.extend(tags.take());
                        }
                    }
                }
                _ => head.extend(block),
            }
            index = end;
        }
        // A file without `created` does not parse anyway; whatever is left goes last.
        head.extend(title);
        head.extend(tags);
        let closing = lines[close];
        let closing = if closing.ends_with('\n') {
            closing.to_owned()
        } else {
            format!("{closing}{newline}")
        };
        head.push(closing);

        let rest: String = lines[close + 1..].concat();
        // The blank lines between the frontmatter and the body are the file's own, even none.
        let separator = &rest[..rest.len() - rest.trim_start_matches(['\r', '\n']).len()];
        let rewritten = format!("{bom}{}{separator}{}", head.concat(), self.body);
        (Self::parse(rewritten.as_bytes()).ok()? == *self).then_some(rewritten)
    }
}

/// Where a scalar is written: a block value after `key: `, or an item of a flow sequence.
#[derive(Clone, Copy)]
enum Context {
    Block,
    Flow,
}

/// Writes `value` plain when it reads back as the same string, which keeps ordinary titles and
/// tags as the agent would type them, and as a double-quoted JSON string otherwise: JSON strings
/// are valid YAML double-quoted scalars, so `true`, `12`, `a: b` or `[x]` stay strings.
fn scalar(value: &str, context: Context) -> String {
    let document = match context {
        Context::Block => format!("value: {value}"),
        Context::Flow => format!("value: [{value}]"),
    };
    let expected = Yaml::String(value.to_owned());
    let plain = !value.is_empty()
        && value.trim() == value
        && !value.contains(['\n', '\r', '"', '\''])
        && YamlLoader::load_from_str(&document)
            .ok()
            .and_then(|documents| documents.into_iter().next())
            .is_some_and(|document| match (&document["value"], context) {
                (read, Context::Block) => *read == expected,
                (Yaml::Array(items), Context::Flow) => items == std::slice::from_ref(&expected),
                (_, Context::Flow) => false,
            });
    if plain {
        value.to_owned()
    } else {
        serde_json::to_string(value).unwrap_or_default()
    }
}

/// The top-level key a frontmatter line starts, if it starts one.
fn key(line: &str) -> Option<&str> {
    if line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    let (key, _) = line.split_once(':')?;
    Some(key.trim_end())
}

/// Where the block starting at `start` ends: a key's value may continue on indented lines or,
/// for a block sequence, on `-` lines at the key's own indentation.
fn block_end(lines: &[&str], start: usize, close: usize) -> usize {
    let mut end = start + 1;
    if key(lines[start]).is_none() {
        return end;
    }
    while end < close && lines[end].starts_with([' ', '\t', '-']) {
        end += 1;
    }
    end
}
