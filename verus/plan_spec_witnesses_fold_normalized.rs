//! The fold witness for the normalization route.
//!
//! [`super::fold`] pins three of the four routes a document can take through the
//! fold: a matched close, an unmatched tail, and an abandoned block. None of them
//! reaches a *matched* block that is released `Normalized`, because every one of
//! their blocks is either closed without an interior conflict or never closed at
//! all. That leaves the emission contract's blank-tag half with no document-level
//! instance, and a route with no instance is a route the document theorem could
//! be quietly mis-wiring.
//!
//! The witness here supplies it. It lives in its own file rather than beside the
//! other three because the block it needs is four lines long and each of them has
//! to be named twice — once in the hypothesis and once in the conclusion — so
//! adding it to `verus/plan_spec_witnesses_fold.rs` would put that file over the
//! 400-line ceiling `AGENTS.md` sets. The split is by route, which is also the
//! seam at which the evidence reads best.

use crate::fence_spec::{spec_closes, spec_fence_next, spec_state_seeded};
use crate::{
    spec_absorb, spec_absorb_all, spec_block_agenda, spec_block_agenda_seeded,
    spec_delimiter_agendum, spec_delimiters_eligible, spec_document_plan, spec_drive,
    spec_drive_step, spec_fenced, spec_finish, spec_is_outer_delimiter, spec_line_facts,
    spec_open_block, spec_plan_step, spec_planner_features, spec_released_agenda, spec_step_agenda,
    spec_step_pending, spec_step_release, spec_step_state,
};
use super::spec_witness_bare_backticks;
use crate::production_fence::LineFeatures;
use crate::production_plan::Agendum;
use vstd::prelude::*;

verus! {

/// A backtick delimiter at depth zero whose trailing-blank tag is stated.
///
/// [`spec_witness_bare_backticks`] is the tag-true case; the null opener below
/// needs the other one, because its marker run is followed by the `null`
/// specifier that normalization drops.
pub open spec fn spec_witness_backtick_tags(
    marker_len: usize,
    trailing_blank: bool,
) -> LineFeatures {
    LineFeatures { depth: 0, marker: Some('`'), marker_len, trailing_blank }
}

/// The document the fold drives to a matched close through the fallback route.
///
/// A four-backtick opener carrying a `null` specifier, a three-backtick `rust`
/// opener and its three-backtick closer, and a four-backtick closer. The interior
/// pair is issue #480's shape in its matched form: under the source opener neither
/// line is a delimiter relation the block honours, but under the three-backtick
/// opener the pass would write, the first is a new opening delimiter and the
/// second closes it. The planner therefore records a conflict and the release
/// writes both outer delimiters `Normalized` instead of `Compressed`.
///
/// The opener also differs from its source only in the tag, so the emitted run's
/// first line is the document-level instance of the positive half of
/// [`spec_delimiter_reads_back`](crate::spec_delimiter_reads_back).
proof fn lemma_witness_matched_normalized_plan_is_live()
    ensures
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_backtick_tags(4, false))
                .push(spec_witness_backtick_tags(3, false))
                .push(spec_witness_bare_backticks(3))
                .push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true).push(true).push(true).push(true),
        ).agenda == Seq::<Agendum>::empty()
            .push(Agendum::Normalized)
            .push(Agendum::Verbatim)
            .push(Agendum::Verbatim)
            .push(Agendum::Normalized),
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_backtick_tags(4, false))
                .push(spec_witness_backtick_tags(3, false))
                .push(spec_witness_bare_backticks(3))
                .push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true).push(true).push(true).push(true),
        ).unclosed.is_none(),
{
    let source = Seq::<LineFeatures>::empty()
        .push(spec_witness_backtick_tags(4, false))
        .push(spec_witness_backtick_tags(3, false))
        .push(spec_witness_bare_backticks(3))
        .push(spec_witness_bare_backticks(4));
    let rewritable = Seq::<bool>::empty().push(true).push(true).push(true).push(true);
    reveal_with_fuel(spec_drive, 8);
    reveal_with_fuel(spec_block_agenda_seeded, 8);
    reveal_with_fuel(spec_state_seeded, 8);
    reveal(spec_document_plan);
    reveal(spec_finish);
    reveal(spec_drive_step);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_released_agenda);
    reveal(spec_block_agenda);
    reveal(spec_delimiter_agendum);
    reveal(spec_is_outer_delimiter);
    reveal(spec_delimiters_eligible);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    reveal(spec_absorb_all);
    reveal(spec_open_block);
    reveal(spec_step_state);
    reveal(spec_step_pending);
    reveal(spec_line_facts);
    reveal(spec_fenced);
    reveal(spec_planner_features);
    reveal(spec_closes);
    reveal(spec_fence_next);
    // The fold's own step at the final index; see `fold`'s module comment for
    // why an explicit equality is needed across a module boundary.
    assert(spec_drive(source, rewritable, 4) == spec_drive_step(
        spec_drive(source, rewritable, 3),
        spec_line_facts(spec_state_seeded(source, 3, None), source[3], rewritable[3]),
        3,
    ));
    let d = spec_drive(source, rewritable, 4);
    assert(spec_finish(d, 4).agenda == Seq::<Agendum>::empty()
        .push(Agendum::Normalized)
        .push(Agendum::Verbatim)
        .push(Agendum::Verbatim)
        .push(Agendum::Normalized));
    assert(spec_finish(d, 4).unclosed.is_none());
}

} // verus!