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
- [ ] M1 Extract the pure fence transition kernel and route `FenceTracker`
      through it.
- [ ] M2 Make the `compress_fences` rewrite decision a pure function of
      `(opening state, line)`.
- [ ] M4 Add the corpus-wide `regions` equality test and cross-pass
      equivalence evidence.
- [ ] M3 Add the Verus proofs and ledger rows.
- [ ] M5 Documentation, gates, CodeRabbit, draft PR.

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

Not yet populated.

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

### M1 — Extract the pure fence transition kernel

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

### M2 — Make the rewrite decision pure

Factor `src/fences/compress.rs` so both flush paths compute their opening
`Strategy` from one pure function of `(opening state, line features)`, and both
emit delimiter lines through one shared rewrite helper. The decision function
must have no side effects, no output buffer, and no regex; where the rewrite
needs the marker run, it takes pre-parsed features.

Acceptance: `make test` passes, including every existing fence test; the two
flush paths demonstrably call the same helper.

### M3 — Verus proofs and ledger rows

Add `spec fn spec_step`, `spec fn spec_regions`, and the refinement
postcondition on the executable kernel, including the kernel with `#[path]` from
`verus/lib.rs`. Prove obligation 3 and, within the stated tolerance,
obligation 4. State the issue #480 reproduction as a specification test. Add
the documented mutation check that drops the marker-character check from
`spec_step` and confirms the tilde/backtick witness fails. Add ledger rows for
the kernel and the theorem, with "external contracts: none", and keep
`make verus-selftest` intact.

Acceptance: `make verus` verifies; `make verus-selftest` still rejects the
smoke proof; `make lint` accepts the new ledger rows; the mutation gate fails
for the intended reason.

### M4 — Corpus equality and cross-pass evidence

Add an integration test under `tests/` modelled on `tests/idempotence_drift.rs`
that walks `tests/data/`, formats each input through the compiled binary, and
asserts `regions(output) == regions(input)` on payload lines, with
self-guarding assertions. Confirm no private classifier remains in the skipping
passes; for residual direct predicate use, add an equivalence test and a ledger
row.

Acceptance: the new test passes and its guards fail if the corpus is empty.
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
