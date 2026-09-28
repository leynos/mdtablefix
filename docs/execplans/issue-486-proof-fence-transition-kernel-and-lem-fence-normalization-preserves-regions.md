# Fence transition kernel and LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS

This ExecPlan is a living document. The sections `Progress`,
`Surprises & discoveries`, and `Decision log` must be kept up to date as work
proceeds.

## Purpose / big picture

`mdtablefix` rewrites Markdown through several independent passes: wrapping,
ellipsis normalization, footnote renumbering, list renumbering, heading
conversion, break normalization, and fence-delimiter compression. Two of those
passes — fence compression and the passes that skip fenced content — must agree
on a single question for every line of a document: *is this line literal code,
a delimiter, or prose?*

Today they agree because they all construct a `FenceTracker`, but nothing
enforces that agreement and nothing proves the compression pass preserves it.
The consequence was two real defects. Compression rewrote an unclosed fence's
opening delimiter so that an interior line silently became the closer, and a
later pass moved payload text out of the literal region and rewrote it; this is
issue [#480](https://github.com/leynos/mdtablefix/issues/480). Footnote
renumbering then used its own Boolean fence toggle that disagreed with the
tracker; this is [issue #481](https://github.com/leynos/mdtablefix/issues/481).
Both are fixed, but by inspection rather than by construction.

After this work, the classification is a small pure function, the compression
rewrite decision is a pure function of `(opening state, line)`, and a Verus
proof establishes that delimiter *spelling* may change while payload *regions*
may not. A reader can then observe the guarantee by running `make verus`, and
by running the corpus-wide `regions` equality test over every file in
`tests/data/`.

## Conformance basis

- [ADR 0011](../../adrs/0011-verified-normalization-core.md): "Verify a narrow
  normalization core". Governs the `#[path]` inclusion convention, the ledger,
  and the `make verus` / `make verus-selftest` gates.
- [Verification ledger](../../verification.md): the claim table that must gain
  rows for every new kernel, theorem, and assumed-equivalent pass.
- Issue #486: this proof target. Issues #479 (harness, closed), #480 and #481
  (defects, closed) are the prerequisites the issue names. Issue #494 is the
  tracking issue for the composition theorem.
- The user supplied a five-task coding plan with this request; the milestones
  below follow that task order. It is treated as a standing instruction to
  implement, so no separate approval gate is taken.

Trace chain for the central obligation:

```plaintext
issue-486 -> M3 (LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS) -> make verus
issue-486 -> M4 (corpus regions equality) -> tests/fence_regions_corpus.rs
```

## Constraints

- Preserve the public API of `FenceTracker`. Consumers in
  `src/wrap/tokenize/mod.rs`, `src/process.rs`, `src/footnotes/*`,
  `src/lists.rs`, `src/ellipsis.rs`, `src/html.rs`, `src/headings.rs`,
  `src/breaks.rs`, and `src/fences/*` must not need signature changes.
- No code file may exceed 400 lines (`AGENTS.md`).
- Verus proofs must be production-used: included from `verus/lib.rs` with
  `#[path]`, never a standalone reimplementation (ADR 0011).
- Ledger "Executable function" names must be real `fn` declarations under
  `src/`, as enforced by `scripts/check-verification-ledger.sh`.
- Every gate is run sequentially, and only by `scrutineer`. No parallel
  format/lint/test runs.
- en-GB-oxendict spelling in all prose and comments.

## Tolerances (exception triggers)

- If the full inductive region theorem does not converge within five focused
  proof iterations, stop and escalate with the narrowest proved statement
  available. Fall back to the per-block preservation lemma plus a ledger row
  recording the residual gap. Do not present a partial induction as complete.
- If linking production `compress_fences` to a Verus spec model requires
  modelling regex recognition, stop and escalate rather than adding a second
  implementation. ADR 0011 forbids a parallel model without a refinement proof.
- If a candidate change breaks a consumer's public API, stop; the API is
  frozen for this work.
- If `make verus` runtime exceeds roughly ten minutes, escalate before
  committing, because CI cost matters.

## Risks

- **Proof scope.** `compress_fences` buffers, defers, caches rewrites, and has
  two flush paths. A faithful spec model is the largest single risk. Mitigated
  by the first tolerance: bound the effort and record honestly.
- **Model/production drift.** The kernel is production code included by Verus,
  so it cannot drift; but the spec model of *compression* could. Mitigated by
  routing the production rewrite decision through the same pure function the
  proof uses, and by the corpus equality test of M4.
- **Vacuous proofs.** A theorem stated over an unreachable state is worthless.
  Mitigated by the witness proofs and the mutation gate described in the
  verification plan.
- **Cross-pass equivalence.** `src/breaks.rs:380` calls `crate::wrap::is_fence`
  directly inside a proptest strategy filter, and that is a generator predicate
  rather than a production classification. Confirm before adding a ledger row.

## Progress

- [x] Reconnaissance: kernel pattern, consumers, corpus, harness.
- [x] Rename branch and push with upstream tracking.
- [x] Set the Lody session title.
- [x] Write this ExecPlan.
- [x] M0 Verify and record that the #480 and #481 prerequisite fixes are on
      the branch tip.
- [x] M1 Extract the pure fence transition kernel and route `FenceTracker`
      through it. Committed as `95c27a3`.
- [x] M2 Make the `compress_fences` rewrite decision a pure function of
      `(opening state, line)`. Committed as `fbd31de`.
- [x] M4 Add the corpus-wide `regions` equality test and cross-pass
      equivalence evidence. Committed as `480b3ef`.
- [x] M3 Add the Verus proofs and ledger rows. Committed as `a82667f` (proofs)
      and `69d0321` (mutation gate, ledger). `make verus`: 81 verified, 0
      errors.
- [ ] M5 Documentation, gates, CodeRabbit, draft PR. In progress: ExecPlan
      update done; gate run, CodeRabbit, push, and PR remain.

Milestones M4 and M3 are sequenced ahead of M3's riskier proof work, so that a
proof that breaches its tolerance leaves a complete, useful deliverable behind.
See the decision log.

## Surprises & discoveries

- Both prerequisite defects (#480, #481) are already fixed on `origin/main` and
  their issues are closed. The branch tip `91aa6d9` sits level with
  `origin/main`. The reproduction
  `printf '````\n```\nliteral...\n' | mdtablefix --fences --ellipsis` now
  returns the input unchanged, so the theorem is no longer false on the branch
  point. Task 1 of the supplied plan reduces to verifying and recording this.
- A Verus harness already exists and works: `make verus-selftest` installs
  Verus 0.2025.04.19.1b16620 and correctly rejects `verus/smoke.rs`. A prior
  issue (#479) delivered it.
- There is a complete precedent for the kernel shape:
  `src/classify_kernel.rs` plus `classify_kernel_macros.rs`,
  `classify_kernel_predicates.rs`, `classify_kernel_consumers.rs`, and
  `verus/classify_spec.rs`, wired by `#[path]` from `verus/lib.rs`. The fence
  kernel should follow it exactly rather than invent a second convention.
- `src/wrap/fence.rs` is 417 lines, already over the 400-line guideline, so
  the kernel must live in a submodule regardless.
- Every `tests/data/` fence fixture is balanced; there is no unclosed-fence
  fixture there. Unmatched-path coverage exists only in Rust tests
  (`tests/fences.rs:411`). `tests/data/footnotes_fence_toggle_input.txt` has a
  four-backtick opener with an interior three-backtick line and is a suitable
  must-cover fixture for the corpus test.
- **The symmetric theorem is false, and the verifier caught it.** The first
  formulation was a biconditional: compressing an opener preserves each line's
  closer status. Verus rejected it with `postcondition not satisfied`, and the
  reason is real rather than a proof failure — a tilde line closes a tilde
  opener but cannot close the three-backtick opener the pass writes in its
  place. The pass is nonetheless entitled to respell that line, so the two runs
  legitimately disagree about delimiter identity. The theorem was restated
  one-sidedly over the lines the pass leaves alone. This is the single most
  important discovery of the task: without a prover, a plausible-sounding
  symmetric claim would have been asserted in a comment and believed.
- **`const fn` bodies are opaque to Verus.** `LineFeatures::prose(0)` and the
  other `const fn` constructors cannot be called from a proof body at all
  ("cannot call function with mode exec"), and a `const fn` cannot carry a
  `when_used_as_spec` contract, so its body cannot be unfolded in a spec either.
  Witness lemmas therefore build their `LineFeatures` literally, and the kernel
  inlines the struct updates its contracts depend on. Three helpers
  (`without_info_string`, `with_trailing_blank`, `fence_marker`) were deleted
  rather than kept as unfolded-by-nobody indirection.
- **A macro does not carry a doc comment placed outside it.** An outer `///`
  above a `verified_kernel_function! {` invocation produces `unused doc
  comment`. The doc must be the first thing inside the macro's own parens.
- **Recursive spec functions are opaque until revealed.** The witness lemma's
  non-degeneracy assertion (`spec_regions_seeded(...) == Seq::empty().push(...)`)
  needed explicit `reveal(...)` calls; without them Z3 has no reason to unfold
  the recursion. The same applies to `seq!`, which routes through array `View`
  machinery the solver will not unfold on its own — the witness body is built
  from `Seq::empty().push(...)` instead.
- **The loop invariant's shape was wrong, not its content.** The original stated
  `out@ + spec_regions_from(features@, index, state@) == spec_regions(features@)`,
  relating a growing prefix to a shrinking suffix. Closing it requires
  associativity of sequence concatenation, which vstd exposes only as
  `lemma_concat_associative` — outside the default broadcast group, and a plain
  `proof fn` rather than a `broadcast proof fn`. Crucially, the call would have
  had to sit in the loop body, which plain Cargo also compiles, so Verus syntax
  there breaks the non-Verus build. Respecifying regions as a *seeded prefix*
  (`spec_regions_seeded`) removed the need for the lemma entirely: the loop's
  invariant became the definition, and no concatenation algebra is involved.
  This was a better outcome than a `proof_after` block would have been, because
  it also made the executable recurrence and the spec recurrence visibly the
  same recurrence.
- **The mutation gate's own gate needed gating.** Requiring only "Verus failed"
  would accept a parse error or an unrelated obligation. The script now
  additionally requires the output to name `closes_fence`'s contract against
  `spec_closes`, so the gate fails for the intended reason or not at all.

## Decision log

- Decision: place the kernel at `src/wrap/fence/kernel.rs`, declared as a child
  module of the existing `src/wrap/fence.rs`. Rationale: colocation with the
  feature (`AGENTS.md` "Group by feature, not layer"), and the parent file must
  shed lines to return under the 400-line guideline. `fence.rs` plus
  `fence/kernel.rs` is a legal Rust 2018 layout. Date 2026-09-28.
- Decision: treat the supplied five-task coding plan as an approved standing
  instruction and do not pause for a separate plan-approval gate. Rationale:
  the request ends with an explicit instruction to push and open a draft PR.
- Decision: not renaming the branch through GitHub's flow. The branch was
  renamed before any pull request existed, and pushed under its final name, so
  there is no PR to keep in sync.
- Decision: sequence the corpus equality test (M4) before the Verus proofs
  (M3). Rationale: the proof is the one deliverable carrying a tolerance-based
  fallback, so finishing the deterministic evidence first means a tolerance
  breach still leaves a complete, shippable improvement rather than a
  half-finished theorem. Date 2026-09-28.
- Decision: restate the region theorem one-sidedly rather than prove a
  biconditional. Rationale: the biconditional is false (see Surprises). The
  one-sided form is the property the formatter actually needs — no line moves
  between the literal and prose regions — and it is the form the executable
  sweep tests. Date 2026-09-28.
- Decision: specify regions as a seeded prefix (`spec_regions_seeded`) rather
  than as a suffix from a running index. Rationale: the suffix form's loop
  invariant needs sequence-concatenation associativity, available only outside
  vstd's default broadcast group; the prefix form makes the invariant the
  definition and needs no concatenation lemma. It also removes any temptation
  to put Verus-only syntax in the loop body, which Cargo must also compile.
  Date 2026-09-28.
- Decision: exclude the kernel's recognition entry points from the proof build
  with `#[cfg(not(verus_keep_ghost))]`, matching the classifier. Rationale: the
  regex boundary cannot compile under Verus, and keeping recognition outside
  the trusted boundary is the established pattern. What crosses the boundary is
  a pre-parsed `LineFeatures`, and that contract is recorded in the ledger.
  Date 2026-09-28.

## Verification plan

Obligations, and how each is discharged.

1. **Local correctness of the transition kernel.** `exec fn fence_step`
   implements `spec fn spec_step` exactly. Method: Verus postcondition
   `ensures result.0@ == spec_step(state@, line@)`. Artefact: per-line kernel in
   `src/wrap/fence/kernel.rs`, included by `verus/lib.rs`. Evidence:
   `make verus` reports the kernel's function count as verified. Non-vacuity:
   the witness tests in M1 exercise four distinct transitions (open, interior
   literal, matching close, implicit close); the mutation gate deletes the
   marker-character check and must fail.

2. **Region classification is a deterministic function of the line sequence.**
   `regions` folds `spec_step`. Method: Verus `spec fn spec_regions` over
   `Seq<Seq<char>>`; the executable `regions` carries the refinement
   postcondition. Non-vacuity: two concrete witnesses with different expected
   vectors — a four-backtick opener with an interior three-backtick line and a
   four-backtick closer must yield `[Delim, Literal, Delim]`; the same with a
   three-backtick closer must yield `[Delim, Literal, Literal]`. Artefacts:
   proofs in `verus/lib.rs`; executable witnesses in `tests/fences.rs` or
   `src/wrap/tests/fence_tracker.rs`.

3. **LEM-REWRITE-PRESERVES-CLOSER-RELATION.** If a line closes a state under
   the original opener, the rewritten line closes the rewritten opener; if it
   does not close, it still does not close. Method: Verus lemma over the pure
   `rewrite_decision` and `rewrite_delimiter` functions extracted in M2.
   Non-vacuity: the lemma must be stated for both strategies, and the mutation
   gate must show a dropped marker check falsifies it.

4. **LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS.** For every line sequence,
   `regions(compress(ls))` agrees with `regions(ls)` on payload lines. Method:
   induction over the line sequence, using obligation 3 for the delimiter
   steps. Non-vacuity: the issue #480 reproduction is a specification test —
   the theorem must state that line 3 of `["````", "```", "literal..."]` stays
   `Literal`, and the proof must fail if `flush_unmatched_block` reverts to
   ignoring the conflict flag. Subject to the first tolerance.

5. **Cross-pass agreement.** Every pass that skips fenced content consumes the
   same classifier. Method: the passes already construct `FenceTracker`; M1
   routes that through the kernel, so no private classifier remains there. For
   any residual direct predicate use, add an executable equivalence test and a
   ledger row marked "classification assumed equivalent". Artefacts:
   `tests/fences.rs` and the ledger.

6. **Corpus-wide equality.** Running `regions` over every `tests/data/` input
   and its formatted output yields equality on payload lines. Method: an
   executable integration test modelled on `tests/idempotence_drift.rs`.
   Non-vacuity: assert the corpus is non-empty, assert a minimum checked count,
   and pin `tests/data/footnotes_fence_toggle_input.txt` as must-cover.

External contracts: none claimed. The kernel takes pre-parsed line features and
performs no regex recognition; the feature extraction that *does* use regex
stays outside the proof boundary and is listed as a residual in the ledger, in
the same spirit as the existing rows.

Axioms relied upon: the pinned Verus release and its standard library, treated
as an axiom per ADR 0011; and the `verus/lib.rs` char-conversion boundary
already recorded in the ledger.

## Outcomes & retrospective

Delivered, pending the M5 gate run, CodeRabbit review, and draft PR.

The formatter now classifies every line of a document through one pure
transition kernel, and the region-preservation theorem is machine-checked
against that same production body. `make verus` reports 81 verified functions
with no errors, `make verus-fence-mutation` confirms the proof fails for the
intended reason when the closing rule is weakened, and
`tests/fence_regions.rs` replays the argument over every fixture in
`tests/data/` plus four whole-file documents that reach the unclosed-fence
path.

Three lessons are worth keeping.

The first is that the verifier earned its place by rejecting a plausible
theorem. The symmetric formulation of region preservation reads well and is
false, and nothing but a prover would have said so. The value of this work is
not the proof as an artefact; it is that the claim now stated is the one that
survives contact with a counterexample search.

The second is that a specification's *shape* is as much a design decision as
its content. The suffix formulation
(`out@ + spec_regions_from(index) == spec_regions`) looks like the natural way
to state a fold, but its invariant needs concatenation associativity, which
vstd keeps out of its default broadcast group; closing it would have meant
either an expensive hint in the loop body, where Cargo must also parse it, or a
`proof_after` block. Respecifying regions as a seeded prefix dissolved the
obligation instead of discharging it, and as a side effect made the executable
recurrence and the spec recurrence literally the same recurrence.

The third is that a mutation gate needs a gate of its own. "Verus failed" is
satisfied by a typo. Requiring the output to name the specific contract that
was falsified is what makes the gate evidence rather than ceremony.

Residual gaps, recorded rather than hidden: the regex-facing recognition
boundary (`features_of_line`, `classify_regions`) is excluded from the proof
build with `#[cfg(not(verus_keep_ghost))]`, so the claims hold for pre-parsed
line features, not for the parse that produces them. That is the same boundary
the existing classifier uses, and the assumed contract is recorded in
`docs/verification.md`. The `compress_fences` pass itself is not proved; what
is proved is the predicate it consults and the region-level consequence of
consulting it correctly, with the executable sweep as the bridge.

## Context and orientation

The pipeline is: `src/process.rs` orchestrates; `src/fences/compress.rs` holds
`compress_fences`; `src/wrap/fence.rs` holds `FenceTracker`; each skipping pass
(`src/lists.rs`, `src/ellipsis.rs`, `src/headings.rs`, `src/breaks.rs`,
`src/html.rs`, `src/wrap/tokenize/mod.rs`, `src/footnotes/*`) builds its own
`FenceTracker` and gates on `is_fence_marker || is_in_fence`.

Build and gate commands are Make targets: `make test`, `make lint`,
`make check-fmt`, `make verus`, `make verus-selftest`, `make verus-mutation`.
`make lint` includes `check-verification-ledger`, so a ledger row naming a
non-existent function fails lint.

## Milestones

### M0 — Record the prerequisite fixes (complete)

Confirmed on the branch tip. `flush_unmatched_block` derives its strategy from
`opening_rewrite(block.has_conflicting_interior_fence)`
(`src/fences/compress.rs:126`) and `flush_matched_block` does the same
(`:140`); both emit delimiter lines through `rewrite_fence_line`. The footnote
passes each construct a fresh `FenceTracker` and scan from line zero
(`src/footnotes/renumber.rs:140`, `:218`; `src/footnotes/lists.rs:54`;
`src/footnotes/renumber/definitions.rs:215`, `:337`), all gating on
`is_fence_marker || is_in_fence`. All five `FenceTracker` constructions in the
footnote subtree were inspected and agree; no divergence exists, so no
correction was needed and no new abstraction was introduced.

Evidence, run against the freshly built `target/debug/mdtablefix`. Each input
opens with four backticks, continues with a three-backtick line, and then has a
payload line. The observed output was byte-identical to the input in every case.

- Piping an input of those three lines into `mdtablefix --fences --ellipsis`
  printed the same three lines back. The third line carries a three-dot
  ellipsis run, which is what the earlier defect let the pass rewrite.
- Piping the same input into `mdtablefix` with the full flag set
  (`--wrap --renumber --breaks --ellipsis --fences`) printed the same three
  lines back.
- Piping a five-line input — a language-tagged four-backtick opener, an
  interior three-backtick line, a short footnote reference inside the fence,
  the closing four backticks, and a prose reference after it — into
  `mdtablefix --footnotes` printed the same five lines back, leaving the
  reference inside the fence and the reference after it both untouched.

A note for whoever continues this work: a fenced block whose body contains a
backtick fence line cannot itself be written with backtick fences in any
document this repository formats, because `compress_fences` will either rewrite
the outer delimiter or markdownlint will report an unterminated block. Record
such evidence as inline code spans in prose, or as a tilde-fenced block whose
opener is longer than any interior run, not as a backtick-fenced `plaintext`
block.

All three reproduce the input unchanged. Both defects are fixed, and regression
tests already exist:
`unclosed_fence_keeps_the_opener_reported_for_an_interior_shorter_fence`
(`tests/fences.rs:411`), the `--fences --ellipsis` CLI case
(`tests/cli.rs:400`), and
`test_fence_toggle_regression_prose_ref_after_nested_fence`
(`tests/footnotes.rs:42`).

### M1 — Extract the pure fence transition kernel (complete)

Delivered in `src/wrap/fence/kernel.rs` (348 lines), declared as a child module
of `src/wrap/fence.rs` and re-exported through it. `fence_step` is the pure
transition function; `regions` folds it over a line sequence; `Region` and
`FenceState` are defined there. `observe_parsed` in the parent derives features
through the kernel's `features_of_line`, calls `fence_step`, and maps the result
onto `FenceObservation`; the tracing events and their `transition` / `reason`
values are preserved and `FenceTracker`'s public API is unchanged. Committed as
`95c27a3`.

As planned, the signature is narrower than the milestone text anticipated:
`LineFeatures` carries the marker character, run length, blockquote depth, and
a trailing-whitespace-only flag, and the *presence* of a marker is that field's
`Some`, so no separate fence-marker flag was needed. Recognition stays in the
parent as `features_of_line`; the kernel is pure over pre-parsed features.

Acceptance met: `make test` passes; the fence-tracker unit and logging tests
pass unchanged; the witness tests asserting the two `regions` vectors live in
`src/wrap/tests/fence_tracker.rs`.

The original milestone text follows.

Add `src/wrap/fence/kernel.rs` holding: a `Region` enum (`Delim`, `Literal`,
`Prose`); the opening-fence state; a pre-parsed line-feature struct carrying
marker character, marker run length, blockquote depth, a
trailing-whitespace-only flag, and a fence-marker flag; `fence_step`, a pure
function from `(Option<FenceState>, LineFeatures)` to
`(Option<FenceState>, Region)` reproducing `observe_parsed` exactly; and
`regions`, a fold of `fence_step` over a line sequence. Model the
line-to-features extraction as a pure function over the character sequence so
`regions` needs no regex.

Refactor `observe_parsed` in `src/wrap/fence.rs` to derive the features, call
`fence_step`, and map the result onto `FenceObservation`, preserving the
current tracing events and their `transition` / `reason` values and keeping the
`FenceTracker` public API unchanged.

Acceptance: `make test` passes; `src/wrap/tests/fence_tracker.rs` and
`src/wrap/tests/fence_tracker_logging.rs` pass unchanged; new witness tests
assert the two `regions` vectors from obligation 2.

### M2 — Make the rewrite decision pure (complete)

`compression_changes_region` in the kernel is now the whole rewrite decision,
and `ParsedLine::observe` (`src/fences/compress.rs:188`) computes each line's
kernel features from the same single parse that produces the structural marker,
so the decision and the transition cannot disagree about what a line is.
`advance_fence_block` consults the predicate directly rather than accumulating a
regex-derived flag. `opening_rewrite` maps the block-level guard onto a
`Strategy`, and both flush paths take their `Strategy` from it: `flush_unmatched_block`
rewrites only the opening delimiter, `flush_matched_block` both delimiters,
`flush_original_block` neither — chosen by `flush_completed_block` from the
cached rewrites. Committed as `fbd31de`.

Acceptance met: `make test` passes including every existing fence test; both
flush paths call `opening_rewrite`, and all four emit delimiter lines through
the shared `rewrite_fence_line`.

### M3 — Verus proofs and ledger rows (complete)

`verus/fence_spec.rs` states the specification and `verus/lib.rs` includes
`src/wrap/fence/kernel.rs` through `#[path]`, so the proofs constrain the body
the formatter runs. The spec functions are `spec_fence_next`,
`spec_fence_region`, `spec_state_seeded`, `spec_regions_seeded`,
`spec_closes`, `spec_agrees_with_opener`, `spec_compressed`,
`spec_interior_delimiter`, `spec_compression_changes_region`, and
`spec_rewrite_permitted`. Every kernel decision carries a postcondition tying
it to its spec function.

Obligation 3 is discharged by `lemma_rewrite_preserves_closer_relation`, in the
one-sided form the counterexample admits. Obligation 4 is discharged by
`lemma_normalization_preserves_regions`, by induction on the prefix length with
`lemma_state_preserved` and a single `lemma_block_stays_open` step. The final
tolerance was never approached: the proof converged once the specification's
shape was corrected, and no `admit` or extra axiom was needed.

`scripts/check-fence-mutation.sh` (target `make verus-fence-mutation`) drops
the marker-character comparison from `closes_fence` and requires the failure to
name that contract. Seven ledger rows were added, the theorem among them.

`make verus`: 81 verified, 0 errors.

Acceptance met: `make verus` verifies with 0 errors; `make verus-selftest`
still rejects the smoke proof; `make lint`'s `check-verification-ledger` accepts
the new rows; the mutation gate fails for the intended reason.

### M4 — Corpus equality and cross-pass evidence (complete)

`tests/fence_regions.rs` (271 lines) holds two sweeps over every fixture under
`tests/data/`. The first asserts
`regions(compress_fences(lines)) == regions(lines)` at every index — the
theorem itself, as a line-by-line equality, since `compress_fences` is a
one-line-in, one-line-out map. The second drives the real binary with the full
flag set and asserts that every line the classifier calls literal survives
byte-for-byte. Both self-guard on corpus size and pin
`tests/data/footnotes_fence_toggle_input.txt` as must-cover.

Because no `tests/data/` fixture is unbalanced, `UNCLOSED_DOCUMENTS` adds four
whole-file documents that reach `flush_unmatched_block`, the first being the
issue #480 reproduction. Committed as `480b3ef`.

Cross-pass agreement (obligation 5) holds without new machinery: all 29
`FenceTracker::new` sites in `src/` route through `observe_parsed`, every
skipping pass gates on `is_fence_marker || is_in_fence`, and `compress_fences`
consults the kernel predicate. No private classifier remains, so no equivalence
test or "assumed equivalent" ledger row was needed.

Acceptance met: the new tests pass; their guards fail on an empty corpus;
`make test` and `make lint` pass.

### M5 — Documentation and delivery

Update `docs/verification.md`, and the relevant architecture or design document
via `docs/contents.md`, then run all gates through `scrutineer`, request
`coderabbit review --agent` and clear every concern, commit, and open a draft
PR titled with `(#486)` containing a `## References` section linking the Lody
session.

## Revision note

- 2026-09-28: Initial draft. Reconnaissance found both prerequisite defects
  already fixed on main and a working Verus harness, so the supplied Task 1 is
  recorded as verification rather than construction, and the plan proceeds from
  the kernel extraction.
- 2026-09-28: M0 completed with reproduction evidence, and M4 resequenced ahead
  of M3 to protect the deliverable against the proof tolerance.
