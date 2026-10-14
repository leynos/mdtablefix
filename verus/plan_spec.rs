//! Specification of the fence-compression planner and its document theorem.
//!
//! `verus/fence_spec.rs` proves a local result: respelling one block's
//! delimiters cannot move an interior line out of its region, for a block whose
//! interior satisfies a guard. That is a statement about a guarded block. This
//! module states the document-level result, whose subject is the whole pass: the
//! output has the same number of lines as the input, and every line keeps the
//! region it had.
//!
//! The document theorem is true for a stronger reason than a per-line transition
//! argument, and it is worth naming. A delimiter the planner respells
//! conservatively — [`spec_delimiter_agendum`] returning [`Agendum::Normalized`]
//! — is written in its *source* marker family and run, with a language specifier
//! dropped only when it was null or absent. Dropping a null specifier cannot
//! change the marker, the run, or the depth the transition reads, but it does
//! raise `trailing_blank`, which `spec_closes` consults. A normalized delimiter
//! therefore keeps marker, run, and depth, and is not plain agreement with its
//! source; [`spec_delimiter_reads_back`] is that relation. Region equality holds
//! for the conflict-bearing blocks that take this route because the lemma
//! guards on `spec_masked`, which excludes the one seed under which the raised
//! tag would be read. Every other line the planner leaves alone follows from a
//! *pointwise reading agreement*, stated once in
//! [`lemma_agreement_preserves_prefix`] and applied to those lines. That is why
//! the planner needs only an eligibility predicate, and why those cases need no
//! transition reasoning at all.
//!
//! Compression — [`Agendum::Compressed`], which writes three backticks whatever
//! the source used — does change the reading, so it does need transition
//! reasoning. [`lemma_compressed_prefix`] supplies it by observing that the
//! written opener is exactly `spec_compressed` of the source opener, which is
//! the second state `fence_spec.rs`'s guarded-interior lemma already compares
//! against. The two files therefore meet exactly: the guard that lemma assumes
//! is the guard [`lemma_no_conflict_all_clear`] extracts from the planner's own
//! conflict fold.
//!
//! The proofs are over *readings* of rendered lines rather than over rendered
//! text, because the pass's decisions are functions of both: the reading decides
//! recognition, and the bytes decide the next reading. A specification over
//! readings alone could not state that payload bytes are unchanged, so
//! [`SpecLine`] carries both and the raw-text obligation is the explicit
//! contract relating them.
//!
//! This file is a facade. It holds no specification of its own: the six child
//! modules declared at its foot carry the whole model, split where the argument
//! splits rather than where the file grew long. The dependencies between them
//! run one way — `compression` and `document` read `agreement`; `steps` and
//! `fold` read `planner`; `fold` reads `steps`; `document` reads `compression`
//! — so the children can be read in that order, and the parent exists to give
//! them one name and one published surface.
//!
//! The split was made at declaration seams, not at section banners: each
//! child owns whole items and the doc comments and proofs that belong to them,
//! so no item spans two files. Every item kept the visibility it had, and the
//! parent re-imports only the names its children and the composition need but
//! the crate root does not.

use vstd::prelude::*;

verus! {

#[path = "plan_spec_agreement.rs"]
mod agreement;

pub use agreement::{
    SpecLine,
    spec_same_reading,
    spec_line_agrees,
    spec_delimiter_reads_back,
    spec_blank_tag_raised,
    spec_masked,
    spec_emission_reads_back,
    lemma_same_reading_preserves_region,
    lemma_same_reading_preserves_next,
    lemma_agreement_preserves_prefix,
};

#[path = "plan_spec_planner.rs"]
mod planner;

pub use planner::{
    spec_open_block,
    spec_absorb,
    spec_absorb_all,
    spec_plan_step,
    lemma_inside_open_means_fence_shaped,
    spec_delimiters_eligible,
    spec_is_outer_delimiter,
    spec_delimiter_agendum,
    spec_block_agenda_seeded,
    spec_block_agenda,
    spec_fenced,
};

use planner::lemma_block_agenda_length;

#[path = "plan_spec_compression.rs"]
mod compression;

pub use compression::lemma_compressed_block_regions;

#[path = "plan_spec_steps.rs"]
mod steps;

pub use steps::{
    spec_planner_features,
    spec_line_facts,
    spec_released_agenda,
    spec_step_pending,
    spec_step_opener_rewritable,
    spec_step_start,
    spec_step_release,
    spec_step_writes_line,
    spec_pending_lines,
};

#[path = "plan_spec_fold.rs"]
mod fold_plan;

pub use fold_plan::{
    DriveState,
    Drive,
    spec_step_state,
    spec_step_agenda,
    spec_drive_step,
    spec_drive,
    Plan,
    spec_finish,
    spec_document_plan,
    lemma_document_plan_total,
};

#[path = "plan_spec_document.rs"]
mod document;

pub use document::lemma_released_block_reads_back;

/// The driver length theorem and the invariant it establishes.
///
/// Declared here rather than in `verus/lib.rs` so the induction sits beside
/// the fold it reasons about, and so the mutation mirror carries the file
/// without a second list to keep in step. The child needs the step
/// specifications above it, so it is declared after them.
///
/// The child is private and reached through a private glob, and its lemmas are
/// `pub(super)`, so the move changes no item's reach. Only [`spec_balanced`] was
/// public when the six lived in this file, and this file is its only consumer
/// besides the child itself; the crate-root route that `verus/lib.rs` publishes
/// for `plan_spec` must not gain five new names as a side effect of a file
/// split. The `pub(super)` on the lemmas is what the parent needs to name them
/// at all — a private glob does not project a child's private items into its
/// parent's scope — and it stops there.
#[path = "plan_spec_drive_induction.rs"]
mod drive_induction;
use drive_induction::*;

/// Composition of block results into the document theorem.
///
/// Declared here rather than in `verus/lib.rs` so the composition sits beside
/// the specification it lifts, and so the mutation mirror carries the file
/// without a second list to keep in step.
///
/// The module is private but the glob is public, so the thirteen pub names the
/// composition published at the crate root before the move still do: the facade
/// keeps `crate::spec_regions_from` and its siblings reachable exactly as they
/// were, rather than the split quietly withdrawing them.
///
/// Nothing in this file calls into the child yet. The composition lemmas are
/// stated and verified, but the document theorem above reaches its conclusion
/// through the driver invariant instead. That is the outstanding obligation
/// recorded under M6 item 5b in the ExecPlan: the composition is what would let
/// the document theorem be restated in terms of it, and until that restatement
/// lands the section is a proof island. The re-export is the honest way to keep
/// it in place — an unused private glob would be a warning, and withdrawing the
/// names would narrow the published surface as a side effect of a file split.
///
/// The four items the child needs from this file are `pub(super)`, so the move
/// widens no public surface.
#[path = "plan_spec_compose.rs"]
mod compose;
pub use compose::*;

/// Non-vacuity witnesses for the lemmas above.
///
/// Extracted to bring this specification back under the 400-line ceiling
/// `AGENTS.md` sets, and declared here rather than in `verus/lib.rs` so the
/// mirror the mutation gates build carries the file without a second list to
/// keep in step, and so the witnesses travel with the theorem they protect.
#[path = "plan_spec_witnesses.rs"]
mod witnesses;
pub use witnesses::*;

} // verus!
