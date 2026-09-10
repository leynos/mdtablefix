//! Orphan fence specifier attachment helpers.

use super::{FENCE_RE, is_null_lang};
use crate::wrap::FenceTracker;

/// Result of an orphan fence specifier attachment operation.
///
/// This outcome is visible across fence modules so callers can distinguish
/// successful attachment from preserving the original input.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum AttachmentOutcome {
    /// An item was attached to the following fence.
    Attached,
    /// Existing input was preserved instead of attaching it.
    Preserved,
}

#[derive(Debug, PartialEq, Eq)]
enum NextFence {
    Attachable { blank_count: usize },
    NotAttachable { blank_count: usize },
}

/// Combine an opening fence with a language specifier.
///
/// The fence's indentation is retained whenever present. If the specifier's
/// indentation extends the fence's, the deeper specifier indentation is used.
/// When the fence lacks indentation, the specifier's indentation becomes the fence's.
/// If the indentations differ without one extending the other (e.g., tabs vs spaces),
/// the fence's indentation wins.
///
/// # Examples
///
/// ```rust,ignore
/// use mdtablefix::fences::attach_specifier_to_fence;
/// assert_eq!(attach_specifier_to_fence("```", "rust", "  "), "  ```rust");
/// assert_eq!(attach_specifier_to_fence("  ```", "rust", "    "), "    ```rust");
/// ```
fn attach_specifier_to_fence(fence_line: &str, specifier: &str, spec_indent: &str) -> String {
    let Some(cap) = FENCE_RE.captures(fence_line) else {
        return fence_line.to_owned();
    };
    let fence_indent = cap.get(1).map_or("", |m| m.as_str());
    let fence_marker = cap.get(2).map_or("```", |m| m.as_str());
    let final_indent = if fence_indent.is_empty() || spec_indent.starts_with(fence_indent) {
        spec_indent
    } else {
        fence_indent
    };
    format!("{final_indent}{fence_marker}{specifier}")
}

fn next_attachable_fence<'a, I>(mut lines: std::iter::Peekable<I>) -> NextFence
where
    I: Iterator<Item = &'a String>,
{
    let mut blank_count = 0;
    while let Some(next_line) = lines.peek() {
        if next_line.trim().is_empty() {
            blank_count += 1;
            let _ = lines.next();
            continue;
        }

        let is_attachable = FENCE_RE
            .captures(next_line)
            .is_some_and(|captures| is_null_lang(captures.get(3).map_or("", |m| m.as_str())));
        return if is_attachable {
            NextFence::Attachable { blank_count }
        } else {
            NextFence::NotAttachable { blank_count }
        };
    }

    NextFence::NotAttachable { blank_count }
}

/// Determine whether a line is a thematic break rather than a language specifier.
///
/// `--breaks` normalises every thematic break to a run of
/// [`crate::breaks::THEMATIC_BREAK_LEN`] underscores, and an underscore run
/// matches [`super::ORPHAN_LANG_RE`]. Attaching one to the fence below deletes
/// the break, so a document whose break precedes a code block would never reach
/// a fixed point under `--breaks --fences`.
///
/// The check is the one [`crate::breaks::format_breaks`] applies, so a line this
/// module refuses to attach is exactly a line that pass rewrites. Lines indented
/// by four columns or more are indented code, not breaks, and keep their
/// specifier behaviour.
fn is_thematic_break(line: &str) -> bool {
    crate::breaks::THEMATIC_BREAK_RE.is_match(line.trim_end())
}

/// Emit `line` verbatim when it is a thematic break rather than a specifier.
///
/// Returns `true` when the line was emitted, so the caller skips specifier
/// attachment for it.
pub(super) fn preserve_thematic_break(line: &str, out: &mut Vec<String>) -> bool {
    if is_thematic_break(line) {
        out.push(line.to_owned());
        return true;
    }
    false
}

/// Attach an orphan specifier to the next attachable fence.
///
/// The lookahead step is pure: it clones the iterator and reports whether an
/// unlabelled fence follows after only blank lines. This command step then
/// consumes the original iterator, mutates the output buffer, and advances the
/// structural fence tracker only when it actually consumes a fence.
pub(super) fn attach_to_next_fence<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    specifier: &str,
    indent: &str,
    out: &mut Vec<String>,
    specifier_line: &str,
    tracker: &mut FenceTracker,
) -> AttachmentOutcome
where
    I: Iterator<Item = &'a String> + Clone,
{
    match next_attachable_fence(lines.clone()) {
        NextFence::Attachable { blank_count } => {
            for _ in 0..blank_count {
                let _ = lines.next();
            }
            if let Some(fence_line) = lines.next() {
                out.push(attach_specifier_to_fence(fence_line, specifier, indent));
                let _ = tracker.observe_source_line(fence_line);
            }
            AttachmentOutcome::Attached
        }
        NextFence::NotAttachable { blank_count } => {
            out.push(specifier_line.to_string());
            for _ in 0..blank_count {
                if let Some(blank_line) = lines.next() {
                    out.push(blank_line.clone());
                }
            }
            AttachmentOutcome::Preserved
        }
    }
}
