//! Proves local chunk conservation for the `ProcessBuffer` ledger.
//!
//! This model tracks chunks only. It deliberately abstracts table reflow and
//! ellipsis replacement, so it makes no byte-level or cross-pass claim.

use vstd::prelude::*;
use vstd::seq::group_seq_lemmas;

fn main() {}

verus! {

pub type Chunk = Seq<char>;

/// Models the `out`, `buf`, and source-cursor state of `ProcessBuffer`.
pub struct BufferLedger {
    pub emitted: Seq<Chunk>,
    pub pending: Seq<Chunk>,
    pub source: Seq<Chunk>,
    pub cursor: nat,
}

impl BufferLedger {
    /// Returns the emitted and pending chunk sequences.
    pub open spec fn view(&self) -> (Seq<Chunk>, Seq<Chunk>) {
        (self.emitted, self.pending)
    }

    /// Abstracts per-chunk formatting while retaining chunk order and count.
    pub open spec fn transform(source: Seq<Chunk>) -> Seq<Chunk> { source }

    /// States the chunk-conservation invariant for the processed prefix.
    pub open spec fn preserves_processed_prefix(&self) -> bool {
        self.cursor <= self.source.len()
            && self.emitted.add(self.pending) == self.source.take(self.cursor as int)
    }

    /// Buffers the next source chunk without changing emitted output.
    pub proof fn push_line(self) -> (next: Self)
        requires
            self.preserves_processed_prefix(),
            self.cursor < self.source.len(),
        ensures
            next.source == self.source,
            next.cursor == self.cursor + 1,
            next.view().0 == self.view().0,
            next.view().1 == self.view().1.push(self.source[self.cursor as int]),
            next.preserves_processed_prefix(),
    {
        broadcast use group_seq_lemmas;

        let next = Self {
            emitted: self.emitted,
            pending: self.pending.push(self.source[self.cursor as int]),
            source: self.source,
            cursor: self.cursor + 1,
        };
        assert(next.source.take(next.cursor as int) == self.source.take(self.cursor as int).push(
            self.source[self.cursor as int],
        ));
        assert(next.emitted.add(next.pending) == next.source.take(next.cursor as int));
        next
    }

    /// Moves every pending chunk into emitted output in its existing order.
    pub proof fn flush(self) -> (next: Self)
        requires
            self.preserves_processed_prefix(),
        ensures
            next.source == self.source,
            next.cursor == self.cursor,
            next.view().0 == self.view().0.add(self.view().1),
            next.view().1.len() == 0,
            next.preserves_processed_prefix(),
    {
        broadcast use group_seq_lemmas;

        let next = Self {
            emitted: self.emitted.add(self.pending),
            pending: Seq::<Chunk>::empty(),
            source: self.source,
            cursor: self.cursor,
        };
        assert(next.emitted.add(next.pending) == self.emitted.add(self.pending));
        assert(next.emitted.add(next.pending) == next.source.take(next.cursor as int));
        next
    }

    /// Finishes the ledger by draining pending chunks into emitted output.
    pub proof fn finish(self) -> (next: Self)
        requires
            self.preserves_processed_prefix(),
            self.cursor == self.source.len(),
        ensures
            next.source == self.source,
            next.cursor == self.cursor,
            next.pending.len() == 0,
            next.emitted == Self::transform(next.source),
            next.preserves_processed_prefix(),
    {
        let next = self.flush();
        assert(next.source.take(next.cursor as int) == next.source);
        assert(next.emitted == Self::transform(next.source));
        next
    }
}

/// Shows the buffer preconditions admit a run with observable output.
pub proof fn lemma_buffer_reaches_nonempty_output()
    ensures
        exists|emitted: Seq<Chunk>| emitted.len() > 0,
{
    broadcast use group_seq_lemmas;

    let first = Seq::<char>::empty().push('a');
    let second = Seq::<char>::empty().push('b');
    let source = Seq::<Chunk>::empty().push(first).push(second);
    let empty = BufferLedger {
        emitted: Seq::<Chunk>::empty(),
        pending: Seq::<Chunk>::empty(),
        source,
        cursor: 0,
    };
    assert(empty.source.take(0) == Seq::<Chunk>::empty());
    assert(empty.emitted.add(empty.pending) == empty.source.take(0));
    assert(empty.preserves_processed_prefix());
    let one = empty.push_line();
    let two = one.push_line();
    let finished = two.finish();
    assert(finished.source.len() == 2);
    assert(finished.emitted == finished.source);
    assert(finished.emitted.len() == 2);
    assert(exists|emitted: Seq<Chunk>| emitted.len() > 0);
}

} // verus!
