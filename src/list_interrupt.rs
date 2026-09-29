//! Recognises the block starts that interrupt a paragraph without a blank line.
//!
//! An ordered list ends where a block that can interrupt a paragraph starts
//! outside its item, blank line or not. `CommonMark` restricts these starts:
//! a bullet item must not be empty, and an HTML block interrupts only for
//! its first six start conditions. Everything else that starts a line after
//! item text is a lazy paragraph continuation and leaves the list open.

/// HTML block names whose opening or closing tag starts a type 6 block.
const BLOCK_TAGS: [&str; 62] = [
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

/// Names that start a type 1 HTML block, which runs to its closing tag.
const RAW_TAGS: [&str; 4] = ["script", "pre", "style", "textarea"];

/// Reports whether a line starts a block that can interrupt a paragraph.
///
/// The line is judged without its indentation, because the caller compares
/// the indentation with the item's content column itself.
pub(crate) fn interrupts_paragraph(line: &str) -> bool {
    let text = line.trim_start();
    is_bullet_with_content(text) || text.starts_with('>') || starts_html_block(text)
}

/// Reports whether text opens a bullet item that has content after its marker.
///
/// An empty item cannot interrupt a paragraph, and only an ASCII space or
/// tab separates the marker from the content.
fn is_bullet_with_content(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('-' | '*' | '+'))
        && matches!(chars.next(), Some(' ' | '\t'))
        && chars.any(|ch| !matches!(ch, ' ' | '\t'))
}

/// Reports whether text opens an HTML block of type 1 to 6.
fn starts_html_block(text: &str) -> bool {
    text.strip_prefix('<')
        .is_some_and(|rest| starts_declaration(rest) || starts_tag_block(rest))
}

/// Reports whether text after `<` opens a comment, processing instruction,
/// declaration or CDATA section (types 2 to 5).
fn starts_declaration(rest: &str) -> bool {
    let is_marked = ["!--", "?", "![CDATA["]
        .iter()
        .any(|marker| rest.starts_with(marker));
    let is_declaration = rest
        .strip_prefix('!')
        .is_some_and(|name| name.starts_with(|ch: char| ch.is_ascii_alphabetic()));
    is_marked || is_declaration
}

/// Reports whether text after `<` opens a raw-text or block-level tag (types 1 and 6).
///
/// A closing tag starts a block only for the block-level names; the raw-text
/// names start one only when they open.
fn starts_tag_block(rest: &str) -> bool {
    let (is_closing, tag) = match rest.strip_prefix('/') {
        Some(tag) => (true, tag),
        None => (false, rest),
    };
    let name_end = tag
        .find(|ch: char| !ch.is_ascii_alphanumeric())
        .unwrap_or(tag.len());
    let name = tag[..name_end].to_ascii_lowercase();
    ends_tag_name(&tag[name_end..]) && is_block_name(&name, is_closing)
}

/// Reports whether the text after a tag name ends the name.
fn ends_tag_name(after: &str) -> bool {
    after.is_empty() || after.starts_with([' ', '\t', '>']) || after.starts_with("/>")
}

/// Reports whether a tag name starts an HTML block, given whether the tag closes.
fn is_block_name(name: &str, is_closing: bool) -> bool {
    BLOCK_TAGS.contains(&name) || (!is_closing && RAW_TAGS.contains(&name))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the paragraph-interruption rules.

    use rstest::rstest;

    use super::interrupts_paragraph;

    #[rstest]
    #[case::bullet("- item")]
    #[case::star("* item")]
    #[case::plus_with_tab("+\titem")]
    #[case::quote("> quote")]
    #[case::comment("<!-- note -->")]
    #[case::processing_instruction("<?php")]
    #[case::declaration("<!DOCTYPE html>")]
    #[case::cdata("<![CDATA[x]]>")]
    #[case::raw_tag("<script>")]
    #[case::raw_tag_upper("<PRE class=a>")]
    #[case::block_tag("<div>")]
    #[case::closing_block_tag("</table>")]
    #[case::self_closing_block_tag("<hr/>")]
    fn a_block_start_interrupts_a_paragraph(#[case] line: &str) {
        assert!(interrupts_paragraph(line));
    }

    #[rstest]
    #[case::bare_bullet("-")]
    #[case::bullet_without_content("-   ")]
    #[case::non_breaking_space("-\u{a0}text")]
    #[case::no_separator("-text")]
    #[case::plain_text("text")]
    #[case::inline_tag("<span>x</span>")]
    #[case::closing_inline_tag("</em>")]
    #[case::longer_name("<divider>")]
    #[case::declaration_without_letter("<!1")]
    #[case::bare_angle("<")]
    fn a_lazy_continuation_does_not_interrupt(#[case] line: &str) {
        assert!(!interrupts_paragraph(line));
    }
}
