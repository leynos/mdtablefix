//! Footnote normalization utilities.
//!
//! Converts bare numeric references in text to GitHub-flavoured Markdown
//! footnote links and normalizes footnote numbering and ordering by
//! orchestrating specialised submodules.

mod inline;
mod lists;
mod parsing;
mod renumber;

use inline::{convert_inline, is_atx_heading_prefix};
use lists::convert_block;
use renumber::{renumber_labels, reorder_footnotes};
use tracing::debug;

use crate::textproc::{Token, push_original_token, tokenize_markdown};

/// Rewrite bare numeric references as Markdown footnote references.
///
/// This is the length-changing half of [`convert_footnotes`]: a reference such
/// as `docs.1` grows into `docs.[^1]`, so any pass that measures text — the table
/// reflow and the paragraph wrap — has to run after it rather than before. It is
/// separate from the rest so the caller can place the two halves either side of
/// those passes; see `process_stream_inner`.
#[must_use]
pub fn convert_inline_footnotes(lines: &[String]) -> Vec<String> {
    convert_inline_footnotes_inner(lines, None)
}

/// Rewrites bare numeric references while preserving Setext heading text.
///
/// `process_stream_inner` invokes this when it will subsequently convert Setext
/// headings, so a line the heading pass will read as heading text keeps its bare
/// references: `docs.1` above an underline is heading text, not a reference.
/// Standalone callers keep the historical behaviour of
/// [`convert_inline_footnotes`], which has no heading-conversion context.
#[must_use]
pub(crate) fn convert_inline_footnotes_with_setext(
    lines: &[String],
    headings_enabled: bool,
) -> Vec<String> {
    let setext_text_lines = headings_enabled.then(|| crate::headings::setext_text_lines(lines));
    convert_inline_footnotes_inner(lines, setext_text_lines.as_deref())
}

/// Applies token-aware inline conversion while preserving protected heading
/// text.
///
/// `setext_text_lines` identifies lines whose numeric text will later become a
/// Setext heading; those lines must pass through unchanged in this stage.
fn convert_inline_footnotes_inner(
    lines: &[String],
    setext_text_lines: Option<&[bool]>,
) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());

    for (index, line) in lines.iter().enumerate() {
        if setext_text_lines.is_some_and(|setext_lines| setext_lines.get(index) == Some(&true))
            || is_atx_heading_prefix(line)
        {
            out.push(line.clone());
        } else {
            let mut converted = String::with_capacity(line.len());
            for token in tokenize_markdown(line) {
                match token {
                    Token::Text(t) => converted.push_str(&convert_inline(t)),
                    other => push_original_token(&other, &mut converted),
                }
            }
            out.push(converted);
        }
    }

    debug!(
        phase = "inline",
        lines_in = lines.len(),
        lines_out = out.len(),
        "converting inline footnote references"
    );
    out
}

/// Rewrite footnote labels to their sequential numbers.
///
/// This is the second length-changing half of [`convert_footnotes`]: a reference
/// such as `[^10]` becomes `[^1]` once the distinct references are numbered by
/// first encounter, and a definition header is rewritten from the same mapping,
/// so both narrow the line they sit on and any pass that measures text has to
/// run after them. The same scan promotes a trailing ordered-list item that a
/// reference shares its number with into a definition header, which widens the
/// line instead, and for the same reason belongs on this side of those passes.
/// It is separate from [`convert_footnote_definitions`], which settles the
/// block structure — converting a trailing ordered list that no reference
/// reaches, and reordering the definitions — so the caller can place the two
/// halves either side of those passes; see `process_stream_inner`.
///
/// Lines inside fenced code blocks are left alone.
#[must_use]
pub fn renumber_footnote_labels(lines: &[String]) -> Vec<String> {
    let mut out = lines.to_vec();
    renumber_labels(&mut out);
    debug!(
        phase = "labels",
        lines_in = lines.len(),
        lines_out = out.len(),
        "renumbering footnote labels"
    );
    out
}

/// Fold a trailing ordered list into definitions and reorder the block.
///
/// This is the structural half of [`convert_footnotes`]: it settles the block
/// and keeps the number each header already carries, because those are
/// [`renumber_footnote_labels`]'s work. Numbering them here as well would take
/// fresh numbers from the pool for the definitions no reference points at, in
/// line order, which moves a definition the label stage had placed earlier to
/// the end of the block.
///
/// The one list it does number is the one it folds itself, and it numbers that
/// from one: `convert_block` converts a heading-led trailing list only when no
/// reference and no definition has claimed a number, so the items are the only
/// definitions in the document and the list's own marker numbers say nothing
/// about where they belong. It reads the block structure around that list, so
/// it runs after the heading pass has settled the structure, and it rewrites
/// list items as definition headers, so it runs after the passes that lay lines
/// out.
#[must_use]
pub fn convert_footnote_definitions(lines: &[String]) -> Vec<String> {
    let mut out = lines.to_vec();
    convert_block(&mut out);
    reorder_footnotes(&mut out);
    debug!(
        phase = "definitions",
        lines_in = lines.len(),
        lines_out = out.len(),
        "converting footnote definitions"
    );
    out
}

/// Convert bare numeric footnote references to Markdown footnote syntax.
///
/// Equivalent to running [`convert_inline_footnotes`],
/// [`renumber_footnote_labels`], and [`convert_footnote_definitions`] in that
/// order; the CLI splits the three around its layout passes, and this
/// whole-document form stays for callers that need them in one step.
#[must_use]
pub fn convert_footnotes(lines: &[String]) -> Vec<String> {
    convert_footnote_definitions(&renumber_footnote_labels(&convert_inline_footnotes(lines)))
}

#[cfg(test)]
mod tests;
