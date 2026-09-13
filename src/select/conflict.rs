//! Pure conflict-marker policy for repository-selected rewrites.
//!
//! This module deliberately has no filesystem dependency. It classifies a
//! document's text, combines that classification with a repository state, and
//! decides whether a rewrite must be refused. [`super::repository_state`]
//! supplies that state at the write boundary.

/// Whether a document contains every marker form that makes a rewrite unsafe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictMarkerState {
    /// The document does not carry all three marker forms.
    Absent,
    /// The document carries all three marker forms.
    Present,
}

/// Whether Git has an operation paused that can leave a conflict unresolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryOperationState {
    /// No merge, rebase, revert, or cherry-pick is paused.
    Idle,
    /// A merge, rebase, revert, or cherry-pick is paused.
    InProgress,
}

/// The three marker forms a conflicted hunk carries, in Git's order.
const MARKERS: [char; 3] = ['<', '=', '>'];

/// The shortest run that begins a marker line.
///
/// Seven is Git's default, and not its only length: a repository that sets
/// `conflict-marker-size` in `.gitattributes` gets runs of exactly the
/// configured length. Guessing low can rewrite a resolution; guessing high
/// only refuses a rewrite that `--allow-conflicted` overrides.
const MARKER_LEN: usize = 7;

/// Classifies the conflict-marker forms in `content`.
#[must_use]
pub fn marker_state(content: &str) -> ConflictMarkerState {
    if has_conflict_markers(content) {
        ConflictMarkerState::Present
    } else {
        ConflictMarkerState::Absent
    }
}

/// Whether `content` carries all three conflict-marker forms, each at the
/// start of a line with a run of at least seven characters.
///
/// All three are required because any one alone is ordinary Markdown. A fenced
/// example carrying all three is indistinguishable from a conflict, which is
/// why this result is combined with [`RepositoryOperationState`] rather than
/// acting on the content alone.
#[must_use]
pub fn has_conflict_markers(content: &str) -> bool {
    MARKERS.iter().all(|marker| {
        content
            .lines()
            .any(|line| starts_with_marker(line, *marker))
    })
}

/// Decides whether the explicit document and repository states refuse a
/// rewrite.
#[must_use]
pub const fn refuses(markers: ConflictMarkerState, repository: RepositoryOperationState) -> bool {
    matches!(markers, ConflictMarkerState::Present)
        && matches!(repository, RepositoryOperationState::InProgress)
}

/// Whether `line` begins with a run of at least [`MARKER_LEN`] copies of
/// `marker`.
fn starts_with_marker(line: &str, marker: char) -> bool {
    let mut characters = line.chars();

    (0..MARKER_LEN).all(|_| characters.next() == Some(marker))
}
