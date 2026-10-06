//! Plain-text previews of message Markdown for timeline cards and the reading page's table of contents.
use pulldown_cmark::{Event, Options, Parser, TagEnd};

/// Longest preview kept from one message, counted in Unicode scalar values.
pub const EXCERPT_CHARS: usize = 120;

/// How much of a message the timeline reads to build its preview, in Unicode scalar values.
///
/// The database hands over only this head, so a long answer costs neither transfer nor parsing;
/// four times the preview leaves room for the markup around its words.
pub const EXCERPT_SOURCE_CHARS: usize = 512;

/// Longest table-of-contents line kept from one message, counted in Unicode scalar values.
///
/// The table of contents shows one line per message in a narrow panel that cuts with an ellipsis anyway,
/// so this only needs to outlast the widest line it can show.
pub const TOC_LINE_CHARS: usize = 48;

/// How much Markdown a table-of-contents line parses from the start of each message, in bytes.
///
/// The lines are built for every message of a conversation on each read, and an answer can
/// run to tens of kilobytes; 48 characters need about 150 bytes of Chinese, so this leaves
/// room for the markup around them.
const TOC_SOURCE_BYTES: usize = 256;

/// Reduces Markdown to the words a reader would see, so every client shows the same preview.
///
/// The server does this once instead of sending whole messages: a card needs a few dozen
/// characters, and a long answer would otherwise make the timeline response unbounded. Callers
/// pass the first [`EXCERPT_SOURCE_CHARS`] of a message; syntax left open at that cut shows up
/// literally, which only reaches the preview when almost all of the head is invisible markup.
pub fn excerpt(markdown: &str) -> String {
    cut(&plain_text(markdown), EXCERPT_CHARS)
}

/// The opening words of a message as one short line, for the reading page's table of contents.
pub fn toc_line(markdown: &str) -> String {
    // The cut ignores Markdown: syntax it leaves open, such as "[文字](https://…", shows up
    // literally only at the end of the line, which the panel's ellipsis almost always hides.
    // Backing off to an earlier line break would avoid that, but would shrink the common
    // "好的！" + long paragraph opening to its first two words.
    let head = &markdown[..markdown.floor_char_boundary(TOC_SOURCE_BYTES)];
    cut(&plain_text(head), TOC_LINE_CHARS)
}

/// The visible words of Markdown, with all layout whitespace collapsed to single spaces.
fn plain_text(markdown: &str) -> String {
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
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Keeps at most `limit` characters, marking a cut with an ellipsis that never follows a space.
fn cut(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(limit).collect();
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

    /// A table-of-contents line is the same plain text as the excerpt, only shorter.
    /// Core test case:
    /// - `specs/test-cases/server/conversation/reading-page.md#the-message-toc-must-list-every-message-of-the-current-path-by-its-opening`
    #[test]
    fn lines_up_a_message_by_its_opening_words() {
        assert_eq!(
            toc_line("## 标题\n\n**回答**很好，见 [链接](https://x.test)。"),
            "标题 回答很好，见 链接。"
        );
        let long = "字".repeat(TOC_LINE_CHARS * 2);
        assert_eq!(toc_line(&long), format!("{}…", "字".repeat(TOC_LINE_CHARS)));
        assert_eq!(toc_line(""), "");
    }

    /// Only the head of a long message is parsed, and a short opening line keeps the paragraph
    /// that follows it.
    /// Core test case:
    /// - `specs/test-cases/server/conversation/reading-page.md#the-message-toc-must-list-every-message-of-the-current-path-by-its-opening`
    #[test]
    fn lines_up_only_the_head_of_a_long_message() {
        let paragraph = "从旧金山出发，沿海有三个不错的选择。".repeat(20);
        assert!(paragraph.len() > TOC_SOURCE_BYTES);
        let opening: String = format!("好的！ {paragraph}")
            .chars()
            .take(TOC_LINE_CHARS)
            .collect();
        assert_eq!(
            toc_line(&format!("好的！\n\n{paragraph}")),
            format!("{opening}…")
        );
        // Words past the budget never reach the line, however little came before them.
        let hidden = format!(
            "[开头](https://x.test/{}) 后面",
            "a".repeat(TOC_SOURCE_BYTES)
        );
        assert_eq!(
            toc_line(&hidden),
            format!("[开头](https://x.test/{}…", "a".repeat(TOC_LINE_CHARS - 20))
        );
        // A multi-byte character straddling the budget is dropped, never split.
        let straddling = format!("{}中", "字".repeat(TOC_SOURCE_BYTES / 3));
        assert_eq!(
            toc_line(&straddling),
            format!("{}…", "字".repeat(TOC_LINE_CHARS))
        );
    }
}
