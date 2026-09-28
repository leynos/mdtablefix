//! List-item containers for the break pass (#572).
//!
//! A thematic break indented to a list item's content column belongs to that
//! item, however many child blocks, blank lines or nested lists come between
//! the item's marker and the break. The stack keeps every open item's content
//! column, so the pass can tell which item holds a line and classify the line
//! relative to it: a break four columns deep inside a `10.` item is a break,
//! not indented code.

use crate::classify::{ClassifyCtx, LineClass, classify_line_with_body, list_content_indent};

/// Content columns of the list items that are open at the current line.
#[derive(Default)]
pub(super) struct ItemStack {
    /// Content columns from the outermost open item to the innermost.
    columns: Vec<usize>,
    /// Whether the previous line was paragraph text a lazy line may continue.
    prev_text: bool,
}

/// Where a line sits: the content column of the item holding it, and the
/// line rebuilt relative to that column.
pub(super) struct Placement {
    /// Content column of the innermost open item containing the line, or 0.
    pub(super) container: usize,
    /// The line with the container's columns of indentation removed.
    pub(super) relative: String,
}

impl ItemStack {
    /// Forgets every open item at a fenced-code boundary.
    pub(super) fn reset(&mut self) {
        self.columns.clear();
        self.prev_text = false;
    }

    /// Records a blank line, which ends any lazy paragraph continuation.
    pub(super) fn blank(&mut self) { self.prev_text = false; }

    /// Places a non-blank, unquoted line and updates the open items.
    ///
    /// Items whose content column is right of the line's indentation close,
    /// unless the line lazily continues the paragraph above it. A list item
    /// line opens an item at its own content column.
    pub(super) fn place(&mut self, line: &str) -> Placement {
        let indent = leading_width(line);
        let text = line.trim_start();
        let lazy = self.prev_text && is_paragraph_text(text);
        if !lazy {
            while self.columns.last().is_some_and(|&column| column > indent) {
                self.columns.pop();
            }
        }
        let container = self.columns.last().copied().unwrap_or(0);
        let relative = format!("{}{text}", " ".repeat(indent.saturating_sub(container)));
        let class = classify_line_with_body(&relative, &ClassifyCtx::default()).class;
        if class == LineClass::ListItem {
            self.columns.push(list_content_indent(text, indent));
        }
        self.prev_text = matches!(class, LineClass::ParagraphText | LineClass::ListItem);
        Placement {
            container,
            relative,
        }
    }
}

/// Reports whether text, taken without its indentation, is paragraph text.
fn is_paragraph_text(text: &str) -> bool {
    classify_line_with_body(text, &ClassifyCtx::default()).class == LineClass::ParagraphText
}

/// Measures a line's leading whitespace in parser columns, tabs to the next stop of four.
fn leading_width(line: &str) -> usize {
    line.chars()
        .take_while(|ch| ch.is_whitespace())
        .fold(0, |column, ch| {
            if ch == '\t' {
                column + 4 - column % 4
            } else {
                column + 1
            }
        })
}
