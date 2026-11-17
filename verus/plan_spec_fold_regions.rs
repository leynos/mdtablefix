//! The document theorem: the fold preserves every line's region.
//!
//! [`lemma_fold_invariant`] is the induction that establishes, at every prefix, the
//! invariant the released-span theorem consumes. [`lemma_document_regions_preserved`]
//! is the theorem itself: it composes the chain the fold accumulated — one span per
//! written chunk — and, when the document ended inside a block, appends that block's
//! preserved tail.
//!
//! The shape of the argument is worth naming, because it is what makes the theorem
//! about the pass rather than about an assumption. Nothing here assumes that a
//! released block preserves its interior. The induction proves the buffer half of
//! the invariant one route at a time, and each route discharges the released-span
//! theorem's guard from the planner's own conflict fold. The only inputs the theorem
//! takes are the emission contract — what the rendered spelling of each agenda entry
//! parses back to — and the two sequences themselves.
//!
//! Fence recognition and blockquote parsing are external contracts throughout: the
//! theorem relates two sequences of [`LineFeatures`], and the claim that rendered
//! text parses back to a feature sequence is the adapter's obligation, not this
//! file's.

use crate::fence_spec::{spec_regions_seeded, spec_rewrite_permitted, spec_state_seeded};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::{Agendum, Block as PlanBlock, Release};

use super::{
    lemma_block_chunk_composes, lemma_document_plan_total, lemma_extends_add_left,
    lemma_extends_transitive, lemma_fold_step, lemma_interior_permitted, lemma_interior_shape,
    lemma_prefix_splits_at_seeded_state, lemma_released_agenda_length, lemma_spans_compose_regions,
    spec_agendum_line,
    spec_balanced, spec_chain_of, spec_chain_preserved, spec_document_plan, spec_drive,
    spec_drive_step,
    spec_emission_reads_back, spec_extends, spec_finish, spec_fold_buffered, spec_fold_chunk,
    spec_fold_facts, spec_fold_invariant, spec_fold_written, spec_pending_lines,
    spec_released_agenda,
};

use vstd::prelude::*;

verus! {

/// `INV-DRIVER-FOLD` holds at every prefix, from the plan alone.
///
/// The hypotheses are what the caller can know before any span is released: the
/// plan's agenda, which is fixed for the whole document, covers exactly the source,
/// and each of its entries reads back to the emitted line of the same index. The
/// induction is over the prefix, and each step is [`lemma_fold_step`], so the
/// invariant is established by the same case split the planner made rather than by
/// an argument about the fold's shape.
pub proof fn lemma_fold_invariant(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
)
    requires
        0 <= i <= source.len(),
        i <= rewritable.len(),
        emitted.len() == source.len(),
        plan_agenda.len() == source.len(),
        spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        forall|j: int|
            #![trigger source[j]]
            0 <= j < source.len() ==> spec_emission_reads_back(
                plan_agenda[j],
                spec_agendum_line(source[j], #[trigger] emitted[j], plan_agenda[j]),
            ),
    ensures spec_fold_invariant(source, emitted, rewritable, plan_agenda, i),
    decreases i,
{
    reveal(spec_fold_invariant);
    if i == 0 {
        // The empty prefix writes nothing, buffers nothing, and its chain is the
        // single cut at the document's start.
        reveal(spec_fold_written);
        reveal(spec_fold_buffered);
        reveal(spec_balanced);
        reveal(spec_pending_lines);
        reveal(spec_chain_of);
        reveal(spec_chain_preserved);
        reveal(spec_extends);
        assert(spec_drive(source, rewritable, 0).out == Seq::<Agendum>::empty()) by {
            reveal_with_fuel(spec_drive, 2);
        };
        assert(spec_state_seeded(source, 0, None) == None::<FenceState>) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_state_seeded(emitted, 0, None) == None::<FenceState>) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_drive(source, rewritable, 0).state.pending.is_none()) by {
            reveal_with_fuel(spec_drive, 2);
        };
    } else {
        let prev = spec_drive(source, rewritable, i - 1);
        let next = spec_drive(source, rewritable, i);
        let facts = spec_fold_facts(source, rewritable, i);
        let chunk = spec_fold_chunk(source, rewritable, i);
        // The step reads its one line and appends its chunk.
        reveal(spec_drive);
        assert(next == spec_drive_step(prev, facts, i - 1)) by {
            reveal_with_fuel(spec_drive, 2);
        };
        reveal(super::spec_drive_step);
        assert(next.out == prev.out.add(chunk));
        // The shorter agenda is therefore an initial segment of the plan too, which
        // is the recursive call's hypothesis. Deriving it before the call is what
        // keeps the induction's own obligation in the caller rather than in a
        // postcondition the callee would have to weaken.
        lemma_extends_add_left(prev.out, chunk);
        lemma_extends_transitive(prev.out, next.out, plan_agenda);
        lemma_fold_invariant(source, emitted, rewritable, plan_agenda, i - 1);
        lemma_fold_step(source, emitted, rewritable, plan_agenda, i);
    }
}

/// The plan's agenda is an initial segment of the fold's final agenda.
///
/// The tail of a document that ends between blocks is the fold's own agenda, and
/// the tail of one that ends inside a block is that agenda plus the released block.
/// Both are initial-segment extensions, which is what lets the induction above be
/// stated over the plan's fixed agenda rather than over the fold's growing one.
pub proof fn lemma_plan_extends_fold_out(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
)
    ensures spec_extends(
        spec_drive(source, rewritable, source.len() as int).out,
        spec_document_plan(source, rewritable).agenda,
    ),
{
    let n = source.len() as int;
    let drive = spec_drive(source, rewritable, n);
    reveal(spec_document_plan);
    reveal(spec_finish);
    reveal(spec_extends);
    match drive.state.pending {
        Some(block) => {
            lemma_extends_add_left(drive.out, spec_released_agenda(
                block,
                n - drive.state.start,
                drive.state.opener_rewritable,
                false,
                Release::Unmatched,
            ));
        },
        None => {
            assert(spec_document_plan(source, rewritable).agenda == drive.out);
            assert(drive.out.subrange(0, drive.out.len() as int) =~= drive.out) by {
                assert forall|k: int| 0 <= k < drive.out.len() implies drive.out.subrange(
                    0,
                    drive.out.len() as int,
                )[k] == drive.out[k] by {
                    vstd::seq::axiom_seq_subrange_index(drive.out, 0, drive.out.len() as int, k);
                }
            };
        },
    }
}

/// The block a document ends inside preserves the document's tail.
///
/// The tail is the fourth flush path: the document stopped with a delimiter open, so
/// the block is released as unmatched and its lines are written exactly as a close
/// would have written them, without a closing delimiter to respell. Its span comes
/// from the released-chunk theorem, and the block's guard comes from the invariant's
/// buffer half — the planner's own conflict fold over the lines it absorbed.
pub proof fn lemma_document_tail_composes(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    block: PlanBlock,
)
    requires
        source.len() == rewritable.len(),
        emitted.len() == source.len(),
        spec_fold_invariant(
            source,
            emitted,
            rewritable,
            spec_document_plan(source, rewritable).agenda,
            source.len() as int,
        ),
        spec_drive(source, rewritable, source.len() as int).state.pending == Some(block),
        forall|j: int|
            #![trigger source[j]]
            0 <= j < source.len() ==> spec_emission_reads_back(
                spec_document_plan(source, rewritable).agenda[j],
                spec_agendum_line(
                    source[j],
                    #[trigger] emitted[j],
                    spec_document_plan(source, rewritable).agenda[j],
                ),
            ),
        spec_regions_seeded(
            source,
            spec_drive(source, rewritable, source.len() as int).out.len() as int,
            None,
        ) == spec_regions_seeded(
            emitted,
            spec_drive(source, rewritable, source.len() as int).out.len() as int,
            None,
        ),
    ensures spec_regions_seeded(source, source.len() as int, None) == spec_regions_seeded(
        emitted,
        source.len() as int,
        None,
    ),
{
    let n = source.len() as int;
    let drive = spec_drive(source, rewritable, n);
    let start = drive.state.start;
    let opener_rw = drive.state.opener_rewritable;
    let chunk = spec_released_agenda(block, n - start, opener_rw, false, Release::Unmatched);
    reveal(spec_document_plan);
    reveal(spec_finish);
    assert(spec_document_plan(source, rewritable).agenda == drive.out.add(chunk));
    reveal(spec_fold_invariant);
    reveal(spec_fold_written);
    reveal(spec_fold_buffered);
    reveal(spec_balanced);
    assert(start < n);
    assert(drive.out.len() == start);
    assert(0 <= start);
    // The interior, exactly as the abandoned routes establish it.
    lemma_interior_shape(source, block.opening, start, n);
    assert(!block.conflicts ==> forall|j: int|
        #![trigger source[j]]
        start < j < n ==> source[j].depth >= block.opening.open_depth
            && !crate::fence_spec::spec_closes(block.opening, source[j])
            && spec_rewrite_permitted(block.opening, source[j])) by {
        if !block.conflicts {
            lemma_interior_permitted(source, block, start, n);
            assert forall|j: int|
                #![trigger source[j]]
                start < j < n implies source[j].depth >= block.opening.open_depth
                    && !crate::fence_spec::spec_closes(block.opening, source[j])
                    && spec_rewrite_permitted(block.opening, source[j]) by {
            }
        }
    };
    // The plan's own entries at the tail's positions are the chunk's. Stated in
    // the shape the released-chunk lemma asks for — indexed by document position,
    // which is also its trigger — rather than by chunk position.
    lemma_released_agenda_length(block, n - start, opener_rw, false, Release::Unmatched);
    assert(chunk.len() == n - start);
    assert forall|j: int|
        #![trigger source[start + j]]
        0 <= j < n - start implies spec_emission_reads_back(
            chunk[j],
            spec_agendum_line(source[start + j], emitted[start + j], chunk[j]),
        ) by {
        vstd::seq::axiom_seq_add_index2(drive.out, chunk, start + j);
        assert(spec_document_plan(source, rewritable).agenda[start + j] == chunk[j]);
        assert(0 <= start + j < n);
    }
    lemma_block_chunk_composes(
        source,
        emitted,
        chunk,
        start,
        n,
        block,
        Release::Unmatched,
        opener_rw,
        false,
    );
    // The span's own region result is a segment; the split law appends it to the
    // prefix the composition already closed.
    lemma_prefix_splits_at_seeded_state(source, start, n, None);
    lemma_prefix_splits_at_seeded_state(emitted, start, n, None);
}

/// `LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS` — the document theorem.
///
/// For every source, every plan the pass derives from it, and every emitted sequence
/// whose rendering parses back to the plan's agenda, the two runs classify every line
/// identically. The only hypothesis about the emitted run is the emission contract:
/// each rendered spelling parses to the features the plan's agendum names, which is
/// the adapter's obligation and is where fence recognition and blockquote parsing
/// enter as external contracts.
///
/// The proof composes the spans the fold wrote. While a block is buffered nothing is
/// written, so the spans are exactly the chunks: one per direct write, one per
/// released block. When the document ends outside a block the last cut is the end of
/// the document; when it ends inside one, the tail is appended as a fourth span.
pub proof fn lemma_document_regions_preserved(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
)
    requires
        source.len() == rewritable.len(),
        emitted.len() == source.len(),
        forall|j: int|
            #![trigger source[j]]
            0 <= j < source.len() ==> spec_emission_reads_back(
                spec_document_plan(source, rewritable).agenda[j],
                spec_agendum_line(
                    source[j],
                    #[trigger] emitted[j],
                    spec_document_plan(source, rewritable).agenda[j],
                ),
            ),
    ensures spec_regions_seeded(source, source.len() as int, None) == spec_regions_seeded(
        emitted,
        source.len() as int,
        None,
    ),
{
    let n = source.len() as int;
    let drive = spec_drive(source, rewritable, n);
    let plan = spec_document_plan(source, rewritable);
    let chain = spec_chain_of(source, rewritable, n);
    let last = chain.cuts.len() as int - 1;
    lemma_document_plan_total(source, rewritable);
    lemma_plan_extends_fold_out(source, rewritable);
    lemma_fold_invariant(source, emitted, rewritable, plan.agenda, n);
    reveal(spec_fold_invariant);
    reveal(spec_fold_written);
    assert(spec_chain_preserved(source, emitted, chain.cuts, chain.states, last));
    assert(chain.cuts[last] == drive.out.len() as int);
    assert(chain.cuts[0] == 0);
    assert(chain.states[0] == None::<FenceState>);
    assert(chain.cuts.len() > 0);
    lemma_spans_compose_regions(source, emitted, chain.cuts, chain.states, last);
    assert(spec_regions_seeded(source, drive.out.len() as int, None) == spec_regions_seeded(
        emitted,
        drive.out.len() as int,
        None,
    ));
    assert(spec_state_seeded(source, drive.out.len() as int, None) == spec_state_seeded(
        emitted,
        drive.out.len() as int,
        None,
    ));
    reveal(spec_balanced);
    match drive.state.pending {
        Some(block) => {
            assert(drive.out.len() == drive.state.start);
            lemma_document_tail_composes(source, emitted, rewritable, block);
        },
        None => {
            reveal(spec_pending_lines);
            assert(drive.out.len() == n);
        },
    }
}

} // verus!