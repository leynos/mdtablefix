//! Pure fence transition kernel behind [`FenceTracker`](super::FenceTracker).
//!
//! The kernel answers one question for every line of a document: is this line
//! a fence delimiter, literal fenced content, or ordinary prose? It is the
//! single classifier shared by the wrapping pipeline, the fence-compression
//! pass, and every pass that skips fenced content, so they cannot disagree
//! about which lines are literal code.
//!
//! Everything here is a pure function of pre-parsed line features and the
//! previous state. There is no regex, no borrowing, and no I/O, so
//! `verus/lib.rs` can include this production body directly and prove that
//! [`fence_step`] realises the specification it states.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

include!("../../verified_kernel_macros.rs");

/// The region a source line belongs to once fences have been resolved.
///
/// Region is the classification the whole pipeline depends on. A payload line
/// may be rewritten only while it stays [`Region::Prose`].
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum Region {
    /// A line that opens or accepts a fence: the two transitions that change
    /// which lines are literal.
    Delim,
    /// Content inside a fenced region, which no pass may rewrite.
    Literal,
    /// Ordinary Markdown outside any fence, which passes may rewrite.
    Prose,
}

/// Opening-fence identity that a later line must match before it can close.
///
/// The three fields are exactly the conditions CommonMark places on a closing
/// fence: the same marker family, a run at least as long, and the same
/// blockquote depth.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct FenceState {
    /// Marker family used by the opener, either a backtick or a tilde.
    pub marker: char,
    /// Marker run length the closer must reach or exceed.
    pub marker_len: usize,
    /// Blockquote nesting depth at which the fence was opened.
    pub open_depth: usize,
}

/// The pre-parsed features of one source line.
///
/// Parsing is deliberately outside the kernel: extracting these features needs
/// the blockquote and fence regexes, which Verus cannot compile. Passing them
/// in keeps the transition relation small enough to prove.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct LineFeatures {
    /// Blockquote nesting depth at which the line appears.
    pub depth: usize,
    /// Marker character of the line's fence run, when the line is fence-shaped.
    pub marker: Option<char>,
    /// Length of that marker run.
    pub marker_len: usize,
    /// Whether only ASCII spaces and tabs follow the marker run.
    ///
    /// CommonMark forbids an info string on a closing fence, so a same-marker
    /// line carrying trailing text is literal content, not a close.
    pub trailing_blank: bool,
}

impl LineFeatures {
    /// Describe a line that is not fence-shaped at any depth.
    #[must_use]
    pub const fn prose(depth: usize) -> Self {
        Self {
            depth,
            marker: None,
            marker_len: 0,
            trailing_blank: false,
        }
    }

    /// Describe a fence-shaped line from its parsed marker run and info string.
    #[must_use]
    pub const fn fence(
        depth: usize,
        marker: char,
        marker_len: usize,
        trailing_blank: bool,
    ) -> Self {
        Self {
            depth,
            marker: Some(marker),
            marker_len,
            trailing_blank,
        }
    }

}

verified_kernel_function! {
/// Whether the line's marker closes the fence described by `state`.
///
/// This is the single closing rule. The transition kernel applies it to the
/// active opener, and the compression pass applies it to the opener it intends
/// to write, so the two cannot drift apart.
#[must_use]
pub fn closes_fence(state: FenceState, line: LineFeatures) -> bool;
ensures(result => result == crate::spec_closes(state, line));
{
    matches!(
        line.marker,
        Some(marker)
            if line.depth == state.open_depth
                && marker == state.marker
                && line.marker_len >= state.marker_len
                && line.trailing_blank
    )
}
}

verified_kernel_function! {
/// Whether the line would close `state` were it not carrying an info string.
///
/// A line that satisfies this but fails [`closes_fence`] is a would-be closer
/// rejected only for its trailing text.
#[must_use]
pub fn agrees_with_opener(state: FenceState, line: LineFeatures) -> bool;
ensures(result => result == crate::spec_agrees_with_opener(state, line));
{
    closes_fence(
        state,
        LineFeatures {
            trailing_blank: true,
            ..line
        },
    )
}
}

/// The marker character every compressed delimiter is written with.
pub const COMPRESSED_MARKER: char = '`';

/// The marker run length every compressed delimiter is written with.
pub const COMPRESSED_MARKER_LEN: usize = 3;

verified_kernel_function! {
/// The delimiter compression writes in place of a state's opening marker.
///
/// Normalization is deliberately a fixed target rather than a shortening:
/// every compressed delimiter is three backticks, whatever the source used.
/// The written delimiter covers exactly the run that was rewritten, so its
/// string length is [`COMPRESSED_MARKER_LEN`] in every case.
#[must_use]
pub fn compressed(state: FenceState) -> FenceState;
ensures(result => result == crate::spec_compressed(state));
{
    FenceState {
        marker: COMPRESSED_MARKER,
        marker_len: COMPRESSED_MARKER_LEN,
        open_depth: state.open_depth,
    }
}
}

/// Reduce one complete source line, blockquote prefix included, to kernel
/// features.
///
/// The streaming [`FenceTracker`](super::FenceTracker) and the batch
/// classification both use this, so the two cannot hold different notions of
/// what a fence line is.
#[cfg(not(verus_keep_ghost))]
#[must_use]
pub fn features_of_line(line: &str) -> LineFeatures { super::features_of(line) }

/// The opening-fence state a source line establishes once it opens a block.
#[must_use]
pub const fn opener(depth: usize, marker: char, marker_len: usize) -> FenceState {
    FenceState {
        marker,
        marker_len,
        open_depth: depth,
    }
}

verified_kernel_function! {
/// Whether `line` is fence-shaped content the block opened by `state` holds
/// open as literal interior text.
///
/// Such a line closes nothing: either its run is too short, its marker is the
/// wrong family, or it carries an info string. Reading it as a delimiter is the
/// defect behind issue #480.
#[must_use]
pub fn interior_delimiter(state: FenceState, line: LineFeatures) -> bool;
ensures(result => result == crate::spec_interior_delimiter(state, line));
{
    line.marker.is_some() && line.depth >= state.open_depth && !closes_fence(state, line)
}
}

verified_kernel_function! {
/// Whether compressing `state`'s delimiter could change the region of `line`.
///
/// This is the whole safety condition for delimiter compression, and it is a
/// pure function of the opening state and one line's features: no buffer, no
/// regex, and no accumulated flag. A block may have its delimiter rewritten to
/// [`compressed`] exactly when this returns `false` for every line in it.
///
/// An interior line that newly closes the rewritten opener ends the block
/// early, moving every later line out of the literal region; that is issue
/// #480, so the pass keeps the source delimiter whenever this returns `true`.
///
/// The marker-family comparison is what makes the predicate discriminating,
/// and it rests on [`closes_fence`] testing the marker character. That
/// dependency is what the mutation gate exercises: dropping the character check
/// leaves this predicate unchanged but falsifies the lemma that justifies it,
/// because a line of an unrelated family would then appear to close the
/// rewritten opener.
#[must_use]
pub fn compression_changes_region(state: FenceState, line: LineFeatures) -> bool;
ensures(result => result == crate::spec_compression_changes_region(state, line));
{
    interior_delimiter(state, line)
        && matches!(
            line.marker,
            Some(marker) if marker == state.marker || marker == compressed(state).marker
        )
}
}

verified_kernel_function! {
/// Advances the fence state by one line and reports that line's region.
///
/// The rules are, in order of precedence:
///
/// 1. A line appearing at a blockquote depth below the opener ends the fence
///    implicitly, before the line itself is read. The line is still processed
///    against the now-empty state, so a delimiter on it opens a fresh fence.
/// 2. A line that is not fence-shaped is literal content while a fence is
///    open, and prose otherwise.
/// 3. A fence-shaped line matching the open delimiter in marker family, run
///    length, and depth, with only trailing whitespace after it, closes the
///    fence.
/// 4. Any other fence-shaped line seen while a fence is open leaves the state
///    unchanged and is literal content: it is either a closing marker carrying
///    an info string, or a marker incompatible with the active opener. Such a
///    line closes nothing, and reading it as a delimiter is exactly the defect
///    that let a later pass rewrite payload text.
/// 5. A fence-shaped line seen with no fence open opens one.
#[must_use]
pub fn fence_step(state: Option<FenceState>, line: LineFeatures) -> (Option<FenceState>, Region);
ensures(result => result.0@ == crate::spec_fence_next(state@, line@),
    result.1 == crate::spec_fence_region(state@, line@));
{
    // An implicit close is applied first, so no later rule can skip it.
    let opened = match state {
        Some(open) if line.depth < open.open_depth => None,
        other => other,
    };

    let Some(marker) = line.marker else {
        return (
            opened,
            if opened.is_some() {
                Region::Literal
            } else {
                Region::Prose
            },
        );
    };

    match opened {
        Some(open) if closes_fence(open, line) => (None, Region::Delim),
        Some(_) => (opened, Region::Literal),
        None => (
            Some(FenceState {
                marker,
                marker_len: line.marker_len,
                open_depth: line.depth,
            }),
            Region::Delim,
        ),
    }
}
}

verified_loop_function! {
/// Classifies every line of a document, in order, by region.
///
/// This is the classification every pass consumes: a line is [`Region::Prose`]
/// exactly when the pipeline may rewrite it.
#[must_use]
pub fn regions(features: &[LineFeatures]) -> Vec<Region>;
ensures(result => result@ == crate::spec_regions(features@));
before {
    let mut state: Option<FenceState> = None;
    let mut out: Vec<Region> = Vec::new();
    let mut index = 0;
}
while (index < features.len()) invariant(
    index <= features@.len(),
    out@ == crate::spec_regions_seeded(features@, index as int, None),
    state@ == crate::spec_state_seeded(features@, index as int, None),
) {
    let (next, region) = fence_step(state, features[index]);
    state = next;
    out.push(region);
    index += 1;
}
after { out }
}

/// Classifies each source line, including any blockquote prefix, by region.
///
/// This is the public face of the kernel: callers that hold whole documents
/// rather than a streaming pass ask here, so they receive the same answer the
/// passes derive from [`FenceTracker`](super::FenceTracker).
///
/// # Examples
///
/// ```
/// use mdtablefix::wrap::{Region, classify_regions};
///
/// let lines = ["````", "```", "literal", "````"];
/// assert_eq!(
///     classify_regions(&lines),
///     vec![
///         Region::Delim,
///         Region::Literal,
///         Region::Literal,
///         Region::Delim
///     ],
/// );
/// ```
#[cfg(not(verus_keep_ghost))]
#[must_use]
pub fn classify_regions<I, S>(lines: I) -> Vec<Region>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let features: Vec<LineFeatures> = lines
        .into_iter()
        .map(|line| super::features_of(line.as_ref()))
        .collect();
    regions(&features)
}
