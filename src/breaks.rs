//! Thematic break formatting utilities.

use std::borrow::Cow;

use crate::{
    classify::{
        ClassifiedLine,
        ClassifyCtx,
        LineClass,
        ListContinuationState,
        classify_line_with_body,
        is_canonical_break_line,
        quote_depth,
        structural_content_indent,
    },
    wrap::{BlockKind, FenceTracker, LinkReferenceMatcher, classify_residual_block},
};

mod item_stack;
use item_stack::{ItemStack, Placement};

pub const THEMATIC_BREAK_LEN: usize = 70;

/// Shared replacement line so every thematic break can be returned without allocation.
static THEMATIC_BREAK_LINE: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| "_".repeat(THEMATIC_BREAK_LEN));

/// Whether a tracked prior line holds paragraph text a later line can continue.
/// A list item marker line counts: it starts its item's paragraph.
fn holds_paragraph_text(class: LineClass) -> bool {
    matches!(class, LineClass::ParagraphText | LineClass::ListItem)
}

/// Context retained between adjacent lines in the break pass.
struct BreakLineState {
    /// Prior class, quote depth, and list content column, when available.
    previous: Option<(LineClass, usize, Option<usize>)>,
    /// Active list indentation tracked for Setext decisions.
    lists: ListContinuationState,
    /// Link-reference matcher shared by every line in the pass.
    links: LinkReferenceMatcher,
}

impl BreakLineState {
    /// Creates state with no prior line, ready to classify the first line.
    fn new(links: LinkReferenceMatcher) -> Self {
        Self {
            previous: None,
            lists: ListContinuationState::default(),
            links,
        }
    }

    /// Forgets context at a fenced-code boundary.
    ///
    /// The matcher is pass configuration, so it survives the reset.
    fn reset(&mut self) {
        self.previous = None;
        self.lists.reset();
    }

    /// Selects the classifier context for the next source line.
    fn context(&self, line: &str, first_pass: &ClassifiedLine<'_>, depth: usize) -> ClassifyCtx {
        if self.continues_paragraph(line, first_pass, depth) {
            ClassifyCtx::following(LineClass::ParagraphText, true)
        } else {
            ClassifyCtx::default()
        }
    }

    /// Checks whether the preceding text can supply a Setext prefix.
    fn continues_paragraph(
        &self,
        line: &str,
        classified: &ClassifiedLine<'_>,
        depth: usize,
    ) -> bool {
        self.previous
            .is_some_and(|(class, old_depth, continuation_indent)| {
                (class == LineClass::ParagraphText
                    || (class == LineClass::ListItem && continuation_indent.is_some()))
                    && old_depth == depth
                    && continuation_indent.is_none_or(|indent| {
                        structural_content_indent(line, classified.body) >= indent
                    })
            })
    }

    /// Records a structural line for the next classifier decision.
    fn observe(&mut self, line: &str, classified: &ClassifiedLine<'_>, depth: usize) {
        let residual = if classified.class == LineClass::ParagraphText {
            classify_residual_block(classified.body.trim(), self.links)
        } else {
            None
        };
        let is_paragraph_link = residual == Some(BlockKind::LinkReferenceDefinition)
            && self.previous.is_some_and(|(class, old_depth, _)| {
                holds_paragraph_text(class) && old_depth == depth
            });
        let is_residual_block = residual.is_some() && !is_paragraph_link;
        let continuation_indent = if is_residual_block {
            self.lists.reset();
            None
        } else {
            self.lists.observe(line, classified)
        };
        self.previous = if classified.class == LineClass::Blank || is_residual_block {
            None
        } else {
            Some((classified.class, depth, continuation_indent))
        };
    }
}

/// Returns the canonical thematic break emitted by [`format_breaks`].
#[must_use]
pub(crate) fn canonical_break() -> &'static str { THEMATIC_BREAK_LINE.as_str() }

/// Normalize thematic breaks outside fenced code blocks.
///
/// Consecutive hyphens, asterisks or underscores are replaced with a
/// standardized line of underscores. Fenced code blocks are ignored so
/// that breaks within them remain untouched.
///
/// # Examples
///
/// ```
/// use std::borrow::Cow;
///
/// use mdtablefix::{THEMATIC_BREAK_LEN, format_breaks};
///
/// let lines = vec!["foo".to_string(), "***".to_string(), "bar".to_string()];
/// let out = format_breaks(&lines);
/// let break_line = "_".repeat(THEMATIC_BREAK_LEN);
/// assert_eq!(
///     out,
///     vec![
///         Cow::Borrowed("foo"),
///         Cow::Borrowed(break_line.as_str()),
///         Cow::Borrowed("bar"),
///     ]
/// );
/// ```
#[must_use]
pub fn format_breaks(lines: &[String]) -> Vec<Cow<'_, str>> {
    let mut out = Vec::with_capacity(lines.len());
    // Track fenced code blocks consistently while formatting breaks.
    let mut fences = FenceTracker::default();
    let mut state = BreakLineState::new(LinkReferenceMatcher::production());
    let mut items = ItemStack::default();

    for line in lines {
        let fence = fences.observe_source_line(line);
        if fence.is_fence_marker || fence.is_in_fence {
            state.reset();
            items.close_outdented(line);
            out.push(Cow::Borrowed(line.as_str()));
            continue;
        }

        let first_pass = classify_line_with_body(line, &ClassifyCtx::default());
        let prefix_len = line.len() - first_pass.body.len();
        let prefix = &line[..prefix_len];
        let depth = quote_depth(prefix);
        let context = state.context(line, &first_pass, depth);
        let classified = if context == ClassifyCtx::default() {
            first_pass
        } else {
            classify_line_with_body(line, &context)
        };
        state.observe(line, &classified, depth);
        let placement = place_in_item(&mut items, line, prefix);

        if let Some(indentation) = placement
            .as_ref()
            .and_then(|p| item_break(p, line, &context))
        {
            out.push(canonicalized_break(indentation, true));
        } else if placement.is_none() && is_canonical_break_line(line, &context) {
            out.push(canonicalized_break(prefix, false));
        } else {
            out.push(Cow::Borrowed(line.as_str()));
        }
    }

    out
}

/// Places an unquoted, non-blank line in the open list items, or records a blank.
///
/// Returns the placement only when an item contains the line; everything
/// else keeps the pass's context-free decision.
fn place_in_item(items: &mut ItemStack, line: &str, prefix: &str) -> Option<Placement> {
    if line.trim().is_empty() {
        items.blank();
        return None;
    }
    if prefix.contains('>') {
        items.close_outdented(line);
        return None;
    }
    Some(items.place(line)).filter(|placement| placement.container > 0)
}

/// Returns a break's indentation when the line is a break inside its item.
///
/// The line is judged relative to the item's content column, so a break four
/// columns deep inside a `10.` item is recognised rather than read as code.
fn item_break<'line>(
    placement: &Placement,
    line: &'line str,
    context: &ClassifyCtx,
) -> Option<&'line str> {
    is_canonical_break_line(&placement.relative, context)
        .then(|| &line[..line.len() - line.trim_start().len()])
}

/// Reports whether a break's prefix is structural and must precede the canonical line.
///
/// A quote prefix always is. Indentation is when the break sits inside a list
/// item, because it places the break in that item.
fn keeps_prefix(prefix: &str, is_in_item: bool) -> bool {
    let is_indented_in_item = is_in_item && !prefix.is_empty();
    prefix.contains('>') || is_indented_in_item
}

/// Retains a structural prefix when emitting the shared canonical break line.
///
/// A quote prefix is always kept. Indentation is kept when the break sits
/// inside a list item, because moving it to column 0 would take the break,
/// and everything after it, out of the item (#572). Elsewhere indentation of
/// up to three spaces changes nothing structural, so the break is emitted at
/// column 0.
fn canonicalized_break(prefix: &str, is_in_item: bool) -> Cow<'static, str> {
    if keeps_prefix(prefix, is_in_item) {
        Cow::Owned(format!("{prefix}{}", canonical_break()))
    } else {
        Cow::Borrowed(canonical_break())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod prop_tests;
