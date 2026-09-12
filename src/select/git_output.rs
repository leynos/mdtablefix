//! What comes back from `git`: the listing parsed, and the diagnostic made safe.
//!
//! Everything here is a pure function of bytes another program wrote, which is
//! why it sits apart from [`super::git_ls_files`]: the adapter decides what to
//! run and what a failure means, and this module turns what came out of the
//! pipe into types this repository owns. Nothing here spawns anything, and
//! nothing here depends on the ambient filesystem, so both halves are testable
//! without a repository.

use camino::Utf8PathBuf;

/// Candidate paths, with a count of paths that were not valid UTF-8.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CandidateListing {
    pub paths: Vec<Utf8PathBuf>,
    pub skipped_non_utf8: usize,
}

/// The longest run of Git's own diagnostic that is relayed before it is cut.
///
/// A cap rather than a promise: a repository name is not this tool's to trust,
/// and a diagnostic that arrives in kilobytes would bury the message it is
/// supposed to support.
const RELAYED_LIMIT: usize = 1024;

/// Renders `bytes` as one line that is safe to write to a terminal.
///
/// Git's text is the one part of this tool's output that this repository does
/// not author, so it is scrubbed on the way through: a newline would forge a
/// second line of stderr, and a control character — an escape, say — would let
/// a path in the repository drive the terminal reading the diagnostic. Each run
/// of them becomes a single space, so a message written as several lines stays
/// readable as one, and a run at either end disappears rather than becoming a
/// gap. Bytes that are not UTF-8 become the replacement character rather than
/// being dropped, because the text is a diagnostic, not a path: nothing acts
/// on it.
///
/// What is scrubbed is [`is_display_control`] rather than
/// [`char::is_control`] alone: the characters that lay a line out are not all
/// in the control category.
pub(crate) fn relayable(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut scrubbed = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        if is_display_control(character) {
            // Not `scrubbed.is_empty()`: a control character before any text
            // marks no space, or the message would begin with one.
            pending_space = !scrubbed.is_empty();
        } else {
            if pending_space {
                scrubbed.push(' ');
                pending_space = false;
            }
            scrubbed.push(character);
        }
    }

    if scrubbed.chars().count() > RELAYED_LIMIT {
        let cut = scrubbed
            .char_indices()
            .nth(RELAYED_LIMIT)
            .map_or(scrubbed.len(), |(index, _)| index);
        scrubbed.truncate(cut);
        scrubbed.push('…');
    }

    scrubbed
}

/// Whether `character` is one that must not reach a terminal as text.
///
/// [`char::is_control`] covers the Unicode `Cc` category, which is where an
/// escape lives. The line separator, the paragraph separator, and the
/// bidirectional formatting controls are not control characters, and they do
/// the same work to a relayed diagnostic: a path in the repository that carries
/// one can give the reader a line break this tool did not write, or reverse the
/// direction of the text beside it. Scrubbing them is what makes "one line, and
/// safe to show" true of every byte Git can hand back, rather than of the
/// characters that happen to be in `Cc`.
fn is_display_control(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
}

/// Splits a NUL-terminated byte stream, counting entries that are not UTF-8.
///
/// An empty segment is skipped rather than reported as an empty path, so empty
/// input yields an empty listing. Git terminates even the last path, so the
/// only empty segments are the one after the final NUL and those in input this
/// tool was handed rather than input Git wrote; neither names a file.
///
/// A path that is not UTF-8 is counted, not dropped silently: it cannot be
/// reported as a [`Utf8PathBuf`], and a caller that acts on the listing must be
/// able to say how many files it could not consider.
pub(crate) fn split_nul_delimited(bytes: &[u8]) -> CandidateListing {
    let mut listing = CandidateListing::default();
    for segment in bytes.split(|byte| *byte == 0) {
        if segment.is_empty() {
            continue;
        }
        match std::str::from_utf8(segment) {
            Ok(path) => listing.paths.push(Utf8PathBuf::from(path)),
            Err(_) => listing.skipped_non_utf8 += 1,
        }
    }

    listing
}

#[cfg(test)]
#[path = "git_output_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "git_output_relay_tests.rs"]
mod relay_tests;
