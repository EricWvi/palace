//! Plain-text previews of message Markdown for timeline cards.
use pulldown_cmark::{Event, Options, Parser, TagEnd};

/// Longest preview kept from one message, counted in Unicode scalar values.
pub const EXCERPT_CHARS: usize = 120;

/// Reduces Markdown to the words a reader would see, so every client shows the same preview.
///
/// The server does this once instead of sending whole messages: a card needs a few dozen
/// characters, and a long answer would otherwise make the timeline response unbounded.
pub fn excerpt(markdown: &str) -> String {
    let mut text = String::new();
    // The reading page renders GFM, so tables and strikethrough must parse the same way here.
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Text(words) | Event::Code(words) => text.push_str(&words),
            // Block boundaries become spaces so adjacent paragraphs and cells do not run together;
            // inline ends (emphasis, links) add nothing, or "**回答**很好" would gain a gap.
            Event::SoftBreak | Event::HardBreak | Event::Rule => text.push(' '),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::CodeBlock
                | TagEnd::TableCell
                | TagEnd::BlockQuote(_),
            ) => text.push(' '),
            // Raw HTML is never rendered by the reading page, so it never reaches the preview.
            _ => {}
        }
    }
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= EXCERPT_CHARS {
        return collapsed;
    }
    let mut cut: String = collapsed.chars().take(EXCERPT_CHARS).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Markup, raw HTML and layout whitespace disappear while the visible words stay in order.
    /// Core test case:
    /// - `specs/test-cases/server/moment/moment-timeline.md#conversation-cards-must-summarize-their-own-path`
    #[test]
    fn strips_markdown_to_visible_words() {
        assert_eq!(
            excerpt(
                "## 标题\n\n**回答**很好，见 [链接](https://x.test) 与 `code`。\n\n- 一\n- 二\n\n<script>alert(1)</script>\n\n| A | B |\n|---|---|\n| 1 | 2 |"
            ),
            "标题 回答很好，见 链接 与 code。 一 二 A B 1 2"
        );
        assert_eq!(excerpt("  多个   空白\r\n\t换行  "), "多个 空白 换行");
        assert_eq!(excerpt(""), "");
    }

    /// Truncation counts characters, not bytes, so Chinese and English cut at the same length.
    /// Core test case:
    /// - `specs/test-cases/server/moment/moment-timeline.md#conversation-cards-must-summarize-their-own-path`
    #[test]
    fn truncates_by_unicode_scalars() {
        let exact = "中".repeat(EXCERPT_CHARS);
        assert_eq!(excerpt(&exact), exact);
        assert_eq!(
            excerpt(&format!("{exact}a")),
            format!("{}…", "中".repeat(EXCERPT_CHARS))
        );
        let mixed = "ab 中文 ".repeat(30);
        let cut = excerpt(&mixed);
        assert_eq!(cut.chars().count(), EXCERPT_CHARS);
        assert!(cut.ends_with('…'));
        assert!(!cut.contains(" …"));
    }
}
