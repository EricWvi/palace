//! The leading `---` YAML block of a Markdown file. Jottings and `pal tags` split files by the
//! same rule, so a block one of them reads is a block the other reads too.

/// Splits a file into its frontmatter and body; `None` when the file has no closed block.
pub fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    if lines.next()?.trim_end_matches(['\r', '\n']) != "---" {
        return None;
    }
    let start = text.find('\n')? + 1;
    let mut offset = start;
    for line in lines {
        let end = offset + line.len();
        if matches!(line.trim_end_matches(['\r', '\n']), "---" | "...") {
            return Some((
                &text[start..offset],
                text[end..].trim_start_matches(['\r', '\n']),
            ));
        }
        offset = end;
    }
    None
}
