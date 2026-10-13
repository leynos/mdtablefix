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
issue-486 -> M4 (corpus regions equality) -> tests/fence_regions.rs
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
- The Cargo package-cache lock (`~/.cargo/.package-cache-mutate`) is
  **permanently deadlocked** for the remainder of this work by another agent's
  process in the `podbot` worktree (`cargo test` holds the sole write lock and
  waits on a `trybuild` child that waits on the parent's lock). It will not
  clear unaided, 85 cargo processes are queued behind it, and it is not this
  branch's to kill. Consequently `lint`, `typecheck`, and `test` **cannot run
  locally at all**; CI is their authority. `cargo fmt` and the Verus gates are
  lock-independent and do still run.

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

- [x] Rebase onto `origin/main` (`bf6c46b`) from the recorded boundary
      `425cd2f`. Seven conflicts resolved, 61 commits replayed, `range-diff`
      clean, all ten gates green on the rebased head. Committed as the replay
      itself plus `a0864e1` (formatter), `1b2d3fb` (mutation-gate evidence),
      and the ExecPlan record. Date 2026-10-13.
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
- [x] M5 Push and draft PR. Draft PR
      [#588](https://github.com/leynos/mdtablefix/pull/588) opened, titled with
      `(#486)`, body carrying `Closes #486` and the Lody session link.
- [x] M5 Gates. `check-fmt`, `verus` (81 verified, 0 errors), `verus-selftest`,
      `verus-mutation`, and `verus-fence-mutation` pass locally. `lint`,
      `format`, `markdownlint`, the Windows `atomic write contract` job, and the
      test suite (2544 run, 2544 passed, 0 skipped) pass in CI at `e810a01`,
      which is where the Cargo gates had to run because the local Cargo
      package-cache lock is deadlocked.
- [x] M5 CodeRabbit review. `coderabbit review --agent --base main` reviewed 17
      files and returned 16 finding entries resolving to 11 distinct issues:
      two major (API docs for the newly public `Region` and `classify_regions`;
      four `compression_predicate_*` tests to consolidate into one `#[rstest]`
      table) and nine minor or trivial (a doc comment that contradicted its own
      assertion, a production `.expect()`, substring rather than whole-line
      comparison in the corpus sweep, `verus-fence-mutation` missing from
      `.PHONY`, two Verus doc errors, and two ExecPlan corrections). All 11 were
      verified against the code and remediated; none were correctness defects in
      the proof. The whole-line finding was substantive rather than cosmetic: it
      caught the terminator mismatch recorded in Surprises, which the weaker
      substring assertion had been hiding.
- [x] M5 CodeRabbit round 4. Zero findings. All seven non-Cargo gates green at
      `b53a7e9` (`check-fmt`, `markdownlint`, `check-static-regexes`,
      `check-verification-ledger`, `verus` 81 verified/0 errors,
      `verus-fence-mutation`, `verus-selftest`), and CI green at the same commit
      (CI 36382514058, Verus 36382513964). The review was run with `--base
      origin/main`; see the decision log.
- [x] M5 Mark the PR ready for review. PR
      [#588](https://github.com/leynos/mdtablefix/pull/588) is `OPEN`,
      `isDraft=false`, base `main`, mergeable, with `Closes #486` and the Lody
      session link in the body.
- [x] M5 Hosted review remediation. Taking the PR out of draft started the
      GitHub App review, which returned four inline findings, one error-level
      pre-merge check, two warnings, a CodeScene quality-gate failure and a
      codex P2. Four were fixed in `4c3b4c5` (the missing `replaced` transition
      arm; corpus-sweep read failures promoted from silent skips to errors; the
      literal-line sweep changed from substring to an in-step walk of both
      literal subsequences; ExecPlan obligations 3 and 4 restated to match what
      the proofs actually establish). The remaining assertion-block finding was
      fixed in `9288d42` by splitting the kernel tests into
      `src/wrap/tests/fence_kernel_tests.rs`. The two documentation warnings
      were fixed later in `b9fe398`; see the Documentation entry below.
- [x] M5 Line-model fix. The stricter literal-line walk failed in CI on
      `tests/data/document/mixed_in_fence.dat`. The cause was the sweep's own
      line model, not a fence defect: the sweeps split on the line feed and kept
      the carriage return, while the product parses a `SourceDocument` (which
      strips a leading byte-order mark) and splits with `str::lines`, so a line
      never carries its terminator. `FENCE_RE` excludes carriage returns from
      its info capture, so a delimiter carrying a stray `\r` is not a fence at
      all. Fixed in `30a5867` by splitting both sides through one helper that
      mirrors the product. Verified against the whole corpus through the real
      binary: 153 fixtures, zero mismatches, and the walk still detects the
      issue #480 defect.
- [x] M5 Documentation. The two hosted-review documentation warnings are fixed.
      `docs/developers-guide.md` drops the stale `src/classify_kernel_macros.rs`
      reference and describes the fence kernel; `docs/users-guide.md` gains a
      Batch fence classification section. Fixed in `b9fe398`. The Format gate
      then failed on the new prose, because it was wrapped by hand rather than
      by `mdtablefix --check`'s own rule set; fixed in `9d20e58`.
- [x] M5 CodeScene regression. The hosted CodeScene check failed on
      `src/wrap/tests/fence_tracker.rs` with `8.03 → 7.79` and the Large
      Assertion Blocks biomarker, while `main` passes the same check at 8.03.
      The decline was mine: three `assert_eq!` on `.features`, added mid-run to
      `observe_source_fence_exposes_structural_marker_with_prefix_indent`,
      pushed three consecutive-assert runs from 3 to 4 and took the count of
      flagged test cases from 7 (main) to 8 (branch). CodeScene's own wording
      for the biomarker is "Consecutive assert statements indicate missing
      abstractions", so the prescribed remedy is extraction, not suppression.
      Every run of four or more is now collapsed to at most three via helpers
      named for the invariant under test (`assert_fence_state`,
      `assert_fence_step`, `assert_transition`, `assert_fenced_line`), and the
      property block moved verbatim to `src/wrap/tests/fence_tracker_props.rs`
      so both files sit under the 400-line limit. Measured result: max
      consecutive-assert run 8 → 3 across every file. **Verified**: CI green at
      `a55e030` (`build-test` success — clippy, the full 2544-test suite, and the
      Windows contract job), and CodeScene now passes at `8.03 → 10.00` with
      *both* the Large Assertion Blocks and Duplicated Assertion Blocks
      biomarkers cleared. The first push of this fix failed the Lint gate with
      `clippy::trivially_copy_pass_by_ref`, because the new `assert_transition`
      took a three-boolean `Copy` type by reference; fixed in `a55e030` by
      taking it by value. `cargo fmt` had reported the file clean, which is the
      point: formatting parses a file, and this defect was only visible to a
      lint that needs type information.
- [x] M5 Close the hosted-review threads. All four review threads are resolved.
      Each finding was verified against the revision it was raised on
      (`6f9ece9` for the CodeRabbit review, `63d4672` for the codex one) rather
      than against the branch tip, because a finding that is stale at the tip
      may still have been valid when raised. All four were valid on their
      reviewed revision and are fixed at the tip: the codex
      `replaced`-transition finding by `4c3b4c5` (submitted 05:49:11Z on
      `63d4672`, fixed 06:13:44Z), and the three CodeRabbit findings —
      obligations 3 and 4, the silent fixture skips, and the membership-based
      literal check — by the same commit, which cites all four in its message.
      The codex thread was answered with that provenance and resolved.
- [x] M5 Clear the MD012 docs-gate failure. CI at `73d0981` failed `build-test`
      on a single `MD012/no-multiple-blanks` at ExecPlan line 312; Format, the
      full 3618-test suite and every other job passed in the same run. A
      documentation-only commit had narrowed which gates applied but not removed
      the obligation to run the one that did. Fixed and recorded in `044abca`;
      `make markdownlint` reports 0 errors across 41 files.
- [x] M5 Rebase onto the PR's target branch. The branch was rebased onto
      `origin/main` (`60eb907d`) from `f9e332bb`, replaying 34 commits. Two
      conflicts were resolved from the Git stages rather than by driver label:
      `src/wrap.rs`, where the commit's two edits were applied onto the upstream
      state, and `Makefile`, which keeps both this branch's
      `verus-fence-mutation` target and main's `test-workflow-contracts` target
      with its `CV005_CONTRACTS_REF` pin. `range-diff` reported only the
      `Makefile` `.PHONY` line as differing, `git diff --check` was clean, and
      the force-push was leased to the previously recorded remote head. Result:
      `NEW_HEAD` `820adf49`.
- [x] M5 Post-rebase gates. Seven gates were run against the rebased tree.
      `check-fmt` (41 files unchanged), `lint` (clippy clean under `-D
      warnings`), `typecheck`, `markdownlint` (0 issues in 41 files), `verus`
      (81 verified, 0 errors), and `verus-fence-mutation` (mutation applied and
      rejected, naming `postcondition not satisfied` against
      `result == crate::spec_closes(state, line)`) all pass. `make test` failed
      in the trybuild `compile` target only, with a moving victim and an
      invariant cause — see Surprises. Its two recipes were then run directly:
      the `compile` target passes 10/10 with `--test-threads=1`, and the doctest
      recipe (`--doc`) passes 41 tests with 22 ignored. The default `make test`
      remains red for an environment reason, not a code one.
- [x] M5 Rebase onto current `origin/main`. The branch was rebased again, onto
      `origin/main` at `425cd2f`, from `f561055`, replaying 36 commits over the
      exclusive boundary `60eb907` — the parent of the branch's first own
      commit, confirmed as an ancestor of `origin/main` with zero merges in the
      range. One conflict arose, in `docs/users-guide.md`: main had appended a
      new section where the branch appended one, so the resolution keeps both,
      the branch's `Batch fence classification` first and main's `Build
      standard` second. Both sides' content is retained; nothing was dropped
      from either. `git range-diff` reports 35 of 36 commits identical, with
      only that doc commit different, matching the union resolution exactly,
      and `git diff --check` against the target is clean. `NEW_HEAD` `972eb7d`.
- [x] M5 Post-rebase gates, second run. All four requested gates pass on
      `972eb7d`, run sequentially by `scrutineer` with each gate's output
      captured to a log under `/tmp`: `check-fmt` (exit 0, 5 s, 42 files
      unchanged), `typecheck` (exit 0, 785 s), `lint` (exit 0, 161 s, with the
      `check-ripgrep`, `check-static-regexes`, and `check-verification-ledger`
      prerequisites passing), and `make test` (exit 0, 3577 s). The test gate
      ran 69 suites green — 2819 passed, 0 failed, 22 ignored — and the
      trybuild `compile` target passed all 10 cases. **The descriptor
      exhaustion recorded in the previous entry did not recur**, so the
      serialised fallback was not needed and `make test` is green by its
      default recipe. No compiler, Clippy, or test warnings were emitted. The
      run took 59.6 minutes because build admission is shared and was saturated
      while this worktree compiled cold, not because of any gate defect.
- [x] M6 Advisory triage. The four advisory checks were traced to their source:
      they are the pre-merge check table in CodeRabbit comment `5863179912`
      (2026-09-28), re-summarised in the 2026-10-13 reply. Items 1
      (`Unit Architecture`) and 3 (`User-Facing Documentation`) were re-checked
      against the published tip and are already satisfied — `data_files` and
      `read_fixture` both return `Result` with both sweeps propagating, and
      `docs/users-guide.md:1126` documents `classify_regions` and all three
      `Region` variants. Item 4 (`Developer Documentation`) was completed by
      rewriting the `### Strategy enum` section; the two
      `docs/adrs/0006-single-pass-idempotence.md` references were verified
      accurate and left alone. Item 2 (`Linked Issues`) is the outstanding one
      and is the subject of M6 item 5.
- [x] M6 Item 5a — extract the compression planner and prove the document
      theorem. Committed as `3846774`. `src/fences/plan.rs` is new: a pure,
      regex-free planner owning `Block`, `Release`, `Agendum`, `LineFacts`,
      `Step`, and six `verified_kernel_function!` kernels plus one
      `verified_loop_function!`. `src/fences/compress.rs` was rewritten to
      consume it, keeping only parsing, the two delimiter respellings, and
      buffering. `verus/plan_spec.rs` states the document theorem. `make verus`
      reports 108 verified and 0 errors (up from 81); `cargo test --workspace`
      reports 2819 passed, 0 failed; `cargo clippy --workspace --all-targets
      --all-features -- -D warnings` and `cargo fmt --all --check` are clean.
      The parked WIP ref `wip/486-plan-kernel` is superseded and can be deleted.
- [x] M6 Item 6 — the compression mutation gate, wired into the Makefile as
      `verus-compression-mutation` and added to `.PHONY`. It confirms the gate
      fails for the intended reason.
- [x] M6 Item 6 (second half) — the two older mutation gates were repaired and
      all three are now wired into CI. `check-classifier-mutation.sh` and
      `check-fence-mutation.sh` each hand-listed which production files to copy
      into their temporary proof tree, and adding `production_plan` to
      `verus/lib.rs` left both copying a stale subset, so each died with
      `couldn't read .../src/fences/plan.rs` before checking any obligation.
      Both gates refused that as mutation detection, which is the behaviour the
      specification demands, but it meant neither had been exercising its proof
      since the planner landed. `scripts/verus-mutation-mirror.sh` now builds
      the tree for all three by mirroring `src/` whole, so a new production
      module in `verus/lib.rs` needs no gate change. Verified each gate still
      detects its intended mutation and nothing else: fence 142 verified with 1
      error naming `result == crate::spec_closes(state, line)`, classifier 142
      with 1 error, compression 142 with 1 error naming
      `result == crate::spec_delimiter_agendum(is_delimiter, compressible)`.
      Committed as `11935a3`, with `.github/workflows/verus.yml` running all
      three as `c6b315f`.
- [x] M6 Item 2 (witness half) — three further non-vacuity witnesses pin rules
      the proof leans on and that had no witness behind them: the marker-family
      comparison the fence mutation drops, the depth check that ends a block
      before its line is read, and the eligibility rule refusing a rewrite when
      only one end may be respelled. Checked falsifiable rather than merely
      true: removing the transition's depth check makes the run fail. `make
      verus` reports 146 verified, 0 errors. Committed as `478befd`, recorded
      in the ledger prose as `2242fd9`.
- [ ] M6 Item 5b — the driver composition. `plan_step`'s routing relation
      already models every release the pass can name, but the emitted readings
      are related to the source readings by the external emission contract
      rather than by a composition lemma over the buffered-bucket fold. This is
      the last gap between the block-level proofs and the pass, and the full
      specification is recorded in the M6 section below and in CodeRabbit's
      AI-agent prompt of 2026-10-13. **Do not mark issue #486 fully addressed
      while this remains open.**
- [ ] M6 Item 2 — the document-level fence-compression preservation proof as
      a whole. Partially delivered: the block-interior and compressed-block
      lemmas, the non-vacuity witnesses, and the mutation gate are in; the
      driver composition above is not. The full specification is recorded in the
      M6 section below and in CodeRabbit's AI-agent prompt of 2026-10-13.

Milestones M4 and M3 are sequenced ahead of M3's riskier proof work, so that a
proof that breaches its tolerance leaves a complete, useful deliverable behind.
See the decision log.

## Surprises & discoveries

- **The rebase onto `origin/main` replayed 61 commits and completed cleanly.**
  The recorded boundary was `OLD_BASE=425cd2f`, `OLD_HEAD=2df4c4a`, target
  `bf6c46b`; `bf6c46b` turned out to be a single squash commit directly on
  `425cd2f`, so the replay range was linear with no merge commits and no
  squash-boundary recovery was needed. Seven conflicts arose, in `verus/lib.rs`,
  `src/verified_kernel_macros.rs`, `scripts/check-classifier-mutation.sh`,
  `tests/verus_harness.rs`, `.github/workflows/verus.yml`,
  `docs/verification.md`, and `docs/developers-guide.md`; each was a pair of
  descriptions of one artefact, and `git range-diff` over `425cd2f..2df4c4a`
  against `bf6c46b..NEW_HEAD` shows twelve changed commits and no dropped work.
  The one apparent deletion, `src/classify_kernel_macros.rs`, is the branch's
  rename to `src/verified_kernel_macros.rs`, and the renamed file's body is
  `main`'s byte for byte. Date 2026-10-13.
- **`make check-fmt` resolves its formatter through `PATH`, not through the
  branch's build.** `$(MDTABLEFIX) ?= mdtablefix` resolves to
  `/home/leynos/.cargo/bin/mdtablefix` 0.6.1, which predates every `src/`
  commit on this branch, so the gate validates the branch's documents with a
  formatter the branch does not contain. `main` sets the same default, and both
  binaries agree exactly on the current tree, so this is an environment hazard
  to record rather than a defect in the branch. Date 2026-10-13.
- **The host has not been reconciled to the Weave estate baseline, so the
  rebase onto `origin/main` runs with the driver bypassed.** The skill
  governing this operation expects global registration *not* to select Weave
  for an ordinary path: "No global attributes rule selects Weave. The legacy
  managed block in `~/.config/git/attributes` is removed. Global registration
  alone must not select Weave for an ordinary path… A host that still selects
  Weave from the global attributes file, or that reports `0.3.6`, has not been
  reconciled; record that as an environment finding and apply the unattended
  bypass below rather than assuming the baseline."

  This host still selects it.
  `git config --show-origin --get merge.weave.driver` resolves to an
  `ANSIBLE MANAGED BLOCK` in `~/.gitconfig` running `weave-driver` 0.5.1, and
  `~/.config/git/attributes` carries a second managed block, "weave user-wide
  enrolment", containing `*.rs merge=weave` (plus `*.py`, `*.ts`, `*.tsx`). The
  repository's tracked `.gitattributes` holds only `* text=auto eol=lf` — no
  merge rules at all — so `git check-attr merge -- src/lib.rs` reporting
  `merge: weave` is ambient configuration rather than repository consent. The
  skill's condition for bypassing is therefore met twice over: selection is
  global-only, **and** this is a multi-commit replay across substantial target
  history.

  The driver is bypassed for the replay with a command-scoped override, which
  replaces the named driver for those commands only and edits no attribute file:

  ```bash
  git -c merge.conflictStyle=zdiff3 \
      -c merge.weave.driver='git merge-file --zdiff3 --marker-size=%L %A %O %B' \
      -c merge.weave.recursive=text \
      rebase …
  ```

  The same `-c` overrides must be repeated on every `git rebase --continue`,
  because Git reads driver configuration again for each replayed commit. Two
  independent reasons make this the safer choice rather than merely the
  compliant one: the 0.5.1 baseline is explicitly "a containment change, not
  evidence that those defects are fixed", and the semantic corruption it
  contains parses, compiles, and passes tests — so a clean driver exit is not
  by itself evidence of a correct merge.

- **Two mutation gates had stopped exercising their proofs, and reported the
  fact as an infrastructure error rather than a pass.** `make verus-mutation`
  and `make verus-fence-mutation` both failed with
  `error: couldn't read .../verus/.fence-mutation-XXXXXX/../src/fences/plan.rs`.
  The cause was not the proofs: each gate hand-listed which production files
  to copy into its temporary proof tree, and M6 item 5a added `production_plan`
  to `verus/lib.rs` without either gate's list being updated. The copied entry
  point therefore declared a module whose source was never placed beside it,
  and Cargo aborted before any obligation was checked. The compression gate,
  written after the planner landed, listed all three modules and kept working —
  which is exactly why the drift went unnoticed: one green gate implied the
  family was healthy.

  The gates *refused* the infrastructure failure rather than counting it. Each
  demands a named postcondition in the output, so a parse error cannot satisfy
  it; `check-fence-mutation.sh` printed "Fence mutation did not falsify the
  closing rule's contract" and exited 1, and `check-classifier-mutation.sh`
  printed "Setext mutation did not reach a failed proof assertion". That guard
  is what turned a silent hole into a visible failure, and it is the reason the
  specification's instruction to reject infrastructure failures is worth having
  in the harness rather than only in the review policy. The durable fix is
  `scripts/verus-mutation-mirror.sh`, which mirrors `src/` whole so no gate
  holds a list that can go stale; all three gates now build from it and all
  three are in CI.

- **The prefix-carried fold invariant is not inductive, and this is now the
  precise residue of M6 item 5b.** The composition law is proved
  (`lemma_spans_compose_regions`), every block-level obligation is proved, and
  the translation layers that make block results usable at any offset are proved
  (`lemma_walk_at_offset`, `lemma_segment_equals_its_subrange`,
  `lemma_released_span_reads_back`, `lemma_released_span_composes`). What is
  *not* proved is an induction over `spec_drive` carrying the emitted-reading
  correspondence in its own hypothesis. The attempt was made and defeated by a
  real obstruction rather than by effort: **the emitted sequence is not a
  one-line-per-step extension of the fold.** The natural `decreases i`
  recursion appends one line per step, but on a `Close` step the agenda grew by
  a *release* — several agendum at once — so the fold's step has no one-line
  emitted analogue and the recursion has no well-founded measure in the agenda
  index. A well-founded version needs the fold to recurse with the emitted
  sequence in an accumulator, so that one step's several agendum are consumed
  against the one line the step accounts for; that is a restructuring of the
  specification rather than a proof-only change.

  What was added instead is the honest decomposition below the induction: the
  released-span translation, the route contract that names which conclusion
  each release discharges, the walk-at-offset translation, and
  `LEM-INSIDE-OPEN-MEANS-FENCE-SHAPED`, which is the side condition the
  induction's `Extend` case needs. Every one of those is a genuine obligation
  discharged rather than a restatement, and each is a prerequisite the
  induction would consume. **Recorded as a reduction, not as a completion:** M6
  items 2 and 5b remain open, and issue #486 must not be closed while they are.

- The CodeRabbit finding asking the literal-line sweep to compare whole output
  lines rather than substrings was not cosmetic: the stricter assertion failed
  in CI on `tests/data/document/mixed_in_fence.dat`, in both `build-test` and
  the Windows `atomic write contract` job. **The first diagnosis of that
  failure was wrong, and the second was right.** The first put it down to ADR
  0007 re-applying one line ending to the whole document, so that a
  CRLF-majority fixture is emitted entirely as CRLF; on that reading the fix
  was to strip the terminator from both sides before comparing. That patch made
  the assertion pass locally but the same failure came straight back in CI,
  because the real defect was upstream of the comparison: the sweep's *line
  model* disagreed with the product's. The sweeps split on the line feed and
  kept the carriage return, while the product parses a `SourceDocument` and
  splits with `str::lines`, so a line never carries its terminator. That is not
  a cosmetic difference: `FENCE_RE` excludes carriage returns from its info
  capture, so `"```\r"` is not recognised as a fence at all. On that fixture
  the old model therefore misfiled the closing delimiter as *literal* on the
  input side and classified the whole CRLF output as unbroken prose, leaving
  the output-side literal subsequence empty — which is exactly the reported
  `left: None, right: Some("echo hi")`. Fixed in `30a5867` by splitting both
  sides through one helper that mirrors the product. Two lessons: a fix that
  makes a failure go away locally is not a diagnosis, and a test that models
  its own input differently from the product can be confidently, silently
  wrong. A third consequence is that sweep #1 was **vacuous on all eight
  carriage-return fixtures** under the old model, since with no fence ever
  recognised it compared all-prose against all-prose; it is real there now.
  Separately, the weaker substring assertion was measurably worse in a second
  way: it would have accepted a literal line that a pass had *merged into* a
  longer line.

- **Hand-wrapped guide prose fails the Format gate, for the third time in this
  work.** `check-fmt` runs
  `mdtablefix --check --git --include-untracked
  --wrap --renumber --breaks --ellipsis --fences`,
  so the same tool that rewraps the repository is also the authority on what
  "wrapped" means. Prose wrapped by hand to look right passed
  `make markdownlint` and still failed, because the gate compares against
  mdtablefix's own reflow and not against a line-length rule. The fix is to run
  the file through the tool rather than to match your own idea of the width.
  Two standing traps make this easy to hit: `--in-place` without the rule flags
  does nothing, so a command that looks like it formatted the file may have
  left it untouched; and `docs/execplans/check-option.md` and
  `docs/execplans/git-option.md` are already unformatted at `origin/main`, so
  the gate reports pre-existing failures that are not this branch's to fix and
  must not be confused with regressions.

- **A green advisory check on `main` says nothing about the same check on this
  branch, and the failing one was mine.** The hosted CodeScene check was read
  as a standing, pre-existing condition and left alone through several rounds.
  It is not: `main` passes it at 8.03, and this branch fails it at 7.79. The
  decline traces to three assertions I added to one test function, which pushed
  its consecutive-assert runs from 3 to 4 and its file over the threshold. The
  general lesson is to compare a failing advisory check against the base branch
  before deciding whose it is: "this check has always been red" is a claim that
  has to be checked, and `gh api .../commits/<ref>/check-runs` answers it in
  one call. Two specifics were worth learning too. The biomarker counts
  *consecutive* assertions per test case, not assertions per file, so splitting
  a file without touching the offending functions changes nothing — my earlier
  `fence_kernel_tests.rs` split was inert for exactly that reason. And the
  count of flagged cases is 7 on `main` and 8 on this branch, which is the
  number that actually moved; the score itself is only its shadow.

- **A refactor that passes `cargo fmt` has been parsed, not verified.** The
  helpers here replaced 24 assertion sites, and formatting proves only that the
  file is syntactically valid. With the Cargo package-cache lock held by
  another agent for the whole session, the focused test run could not happen
  locally at all. The extraction was therefore checked two other ways that do
  not need the lock: the moved property block and both helpers were diffed
  against the original and confirmed byte-for-byte identical, and every
  rewritten assertion was re-read against the source of `main` to confirm it
  tests the same thing. One conversion was in fact *weaker* than what it
  replaced — it substituted `in_fence_for_line` for a direct
  `observation.is_in_fence`, two predicates that agree today but need not — and
  was corrected. Verification that a refactor is faithful is not the same as
  verification that it compiles, and neither is the same as verification that
  it passes.

  The prediction held, and the failure it missed was instructive. CI found
  `clippy::trivially_copy_pass_by_ref` in the new helper: `FenceObservation` is
  three `bool`s and a `Copy`, so an `&FenceObservation` parameter is more
  expensive than passing it by value, and `-D warnings` makes that fatal. None
  of the three structural checks could have caught it, and neither could
  `cargo fmt`, because the defect was not in the *logic* the checks compared —
  the assertions were faithful line for line — but in a *signature* choice that
  only a lint with type information can see. Being faithful and being
  well-typed are independent properties, and a test-only change is easy to
  mistake for one that cannot break the build.

- **CI is the real gate when the local lock is deadlocked, and it works.** With
  `lint`, `typecheck`, and `test` unrunnable locally, every Cargo gate was
  obtained by pushing and watching the run: `gh run watch <id> --exit-status`
  foregrounded in the background, with `--log-failed` to extract the failure and
  `--log` plus `grep 'test result:'` to confirm the suite actually ran rather
  than trusting a green job badge. The last step matters. "Job passed" and "my
  tests passed" are different claims, and only the second is the one worth
  making: the individual `wrap::tests::fence_tracker_props::*` lines were read
  out of the log to confirm the moved module compiled and its four property
  cases executed.

- Acting on the CodeRabbit finding about `ObservedFence` removed a parse rather
  than adding a field. `ParsedLine::observe` was calling
  `features_of_line(line)` — a second full regex pass over a line the tracker
  had already parsed — because `ObservedFence` carried only the structural
  `(indent, marker, info)` capture. Exposing the features the tracker already
  computed let `compress.rs` consume them directly. That in turn made
  `features_of_line` dead in production, so it was deleted along with its three
  re-exports; `line_features` in the parent remains the single producer.
  Removing production code is the better outcome here than adding a field would
  have been, and it is what the finding pointed at even though it only asked to
  "avoid a second parse".

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
  `src/classify_kernel.rs` plus `src/verified_kernel_macros.rs`,
  `src/classify_kernel_predicates.rs`, `src/classify_kernel_consumers.rs`, and
  `verus/classify_spec.rs`, wired by `#[path]` from `verus/lib.rs`. The fence
  kernel should follow it exactly rather than invent a second convention. (The
  macros file was named `classify_kernel_macros.rs` when this reconnaissance
  was written; it has since been renamed, and the reference here is corrected
  to match the file that exists.)
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
  `when_used_as_spec` contract, so its body cannot be unfolded in a spec
  either. Witness lemmas therefore build their `LineFeatures` literally, and
  the kernel inlines the struct updates its contracts depend on. Three helpers
  (`without_info_string`, `with_trailing_blank`, `fence_marker`) were deleted
  rather than kept as unfolded-by-nobody indirection.
- **A macro does not carry a doc comment placed outside it.** An outer `///`
  above a `verified_kernel_function! {` invocation produces
  `unused doc comment`. The doc must be the first thing inside the macro's own
  parens.
- **Recursive spec functions are opaque until revealed.** The witness lemma's
  non-degeneracy assertion
  (`spec_regions_seeded(...) == Seq::empty().push(...)`) needed explicit
  `reveal(...)` calls; without them Z3 has no reason to unfold the recursion.
  The same applies to `seq!`, which routes through array `View` machinery the
  solver will not unfold on its own — the witness body is built from
  `Seq::empty().push(...)` instead.
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
- **`spec_closes` was cross-checked against the CommonMark specification.** The
  normative sentences of §4.5 are "The closing code fence must use the same
  character as the opening fence", "The closing code fence must be at least as
  long as the opening fence", and "A closing fence … may be followed only by
  spaces or tabs". Those are exactly the three tests `closes_fence` performs,
  and the trailing-whitespace clause is why `trailing_blank` admits only ASCII
  space and tab rather than trimming Unicode-aware. The one rule outside the
  kernel is container matching: CommonMark requires a closing fence to be in
  the same container as its opener, which `spec_closes` states as depth
  equality and the kernel inherits.
- **The commit-gate sweep is contending with a shared Cargo package-cache
  lock.** All four Verus gates and `check-fmt` pass, but `make lint`,
  `make typecheck`, and `make test` sat in
  `Blocking waiting for file lock on shared package cache` for over an hour
  behind other agents' cargo jobs. This is infrastructure contention, not a
  code failure: no gate has been observed to fail on this commit. Per the
  standing instruction the lock is waited out rather than worked around with a
  private `CARGO_HOME`.
- **The lock is deadlocked, not merely busy.** Kernel evidence: PID 1832225
  (`cargo test` in `/podbot/worktrees/9863d7f9…`) holds the exclusive
  `FLOCK WRITE` on `.package-cache-mutate` and is parked in `do_wait`; its
  child 1855438 is parked in `futex_wait_queue`; *its* child 1855450 wants the
  lock and is parked in `locks_lock_inode_wait`. That is a closed cycle formed
  entirely by one agent's process tree. Forty-six processes are queued on the
  same lock (`/proc/locks`), including a `cargo check` of this very worktree.
  CPU counter deltas across a 20-second and again across a 100-minute sample
  are flat, and no `rustc` process has existed for the whole period. An
  `--offline` invocation still wants the lock, so there is no supported route
  around it. This is the escalation the Tolerances section exists to catch;
  clearing it means killing another agent's job, which is outside this work's
  authority.
- **CI ran the three blocked gates, and found a real defect in one of them.**
  The draft PR's `build-test` job failed twice on `make lint` with
  `clippy::unnecessary_wraps` at `tests/fence_regions.rs`: first on
  `assert_regions_preserved`, whose reads are handled with
  `let Ok(..) else { continue }` so it never propagates a failure; then, once
  that `Result` was gone, on its caller. Both are now plain `fn`. Worth
  recording as a lesson about waiting out a contended lock: waiting was right —
  working around the lock with a private `CARGO_HOME` would not have been the
  fix, and would have hidden this defect. What recovered the gate was routing
  it somewhere it could run, not bypassing it.
- **The Verus gates are lock-independent.** They invoke `rust_verify` through
  `uvx` and never touch the Cargo package cache, which is why all four
  completed in about a second each while the Cargo gates sat blocked. Useful
  for triage: a green Verus run says nothing about whether a Cargo gate has
  been attempted.
- **A passing mutation gate cannot show its own evidence.**
  `make verus-fence-mutation` asserts three substrings of the verifier output
  (`verification results::`, `postcondition not satisfied`, and the `ensures`
  clause `result == crate::spec_closes(state, line)`) and then deletes its
  temporary directory in a `trap`, so on the pass path nothing is printed. The
  contract the gate requires the output to name is the spec-call expression,
  not the literal token `closes_fence`. Exit 0 therefore means the three
  assertions held, which is the designed evidence, but the log cannot be read
  afterwards to confirm which contract failed if a future mutation is
  mis-targeted. Recorded rather than "fixed": making the pass path print the
  verifier output is a change to the existing gate's noise level, outside this
  issue's proof scope.
- **A clean CLI CodeRabbit pass does not mean the PR is clean.** Round 4 of
  `coderabbit review --agent` returned zero findings at `b53a7e9`, and every
  gate was green, so the PR was marked ready. Taking it out of draft then
  started the *hosted* GitHub App review, which reviewed the same commit and
  returned four inline findings, an error-level pre-merge check, two warning
  pre-merge checks, a CodeScene quality-gate failure and a codex P2 — a
  materially different and stricter result. The two are separate reviewers: the
  App only reviews non-draft PRs, and the CLI pass cannot see its configured
  rule set. The operational consequence is that "all concerns cleared" cannot
  be established from the CLI alone, and the point at which the App first runs
  is a state change worth anticipating rather than treating as a formality.
- **A docs-only commit still has to pass a gate, and the ExecPlan failed its
  own.** The commit that recorded the CodeScene fix was documentation only, so
  no Rust gate applied to it — but `make markdownlint` did, and it failed:
  MD012/no-multiple-blanks at a single stray double blank left behind when an
  edit re-inserted a bullet into the middle of the Surprises list. Format, the
  full 3618-test suite, and every other job passed in the same run; this one
  lint was the entire failure. The lesson is narrow but worth stating: "docs
  only" reduces which gates apply, it does not remove the obligation to run the
  ones that do. It is also the second time in this task that a *structural*
  check caught what reasoning had missed, the first being the
  `clippy::trivially_copy_pass_by_ref` error in a `cargo fmt`-clean file.
- **A stale local binary reports defects that no longer exist.** The vendored
  `target/debug/mdtablefix` was built at 02:41, before the fence work landed,
  and `--check` from it flagged `docs/execplans/check-option.md` and
  `docs/execplans/git-option.md` — files this branch did not touch. The freshly
  built CI binary reports all 41 files unchanged. Both readings are of *this
  branch's* formatting rules, but only one of them runs the current classifier.
  When a local self-format check disagrees with CI's Format step, check the
  binary's mtime against the commit before believing the local result.
- **The post-rebase `make test` failure is file-descriptor exhaustion, not a
  code defect.** The gate failed in the trybuild `compile` target on both runs,
  but on a *different* case each time — `footnote_stages_public_api_compiles`
  in 199s, then `line_ending_const_api_compiles` in 80s — and every other
  target passed. The invariant across both is the cause printed by the compiler
  wrapper:
  `build-limits: cannot start /home/leynos/.cargo/bin/sccache: [Errno
  24] Too many open files`,
  which aborted the nested cargo build and cascaded into `could not compile`
  for the dependency crates. No trybuild `wip/` directory or `MISMATCH` block
  was ever produced, so no case's *correctness* was falsified. File descriptors
  are per-process, and trybuild runs all ten of its cases concurrently, each
  spawning its own nested cargo tree — the most fd-hungry gate on a machine
  already holding 12 compiles at the queued ceiling. Running the same target
  with `--test-threads=1` passes 10/10 in 19.69s. Recorded as an environment
  limitation of the shared host; the deterministic evidence for the target is
  the serialised run, and CI (a quieter, per-job machine) remains the authority
  for the default invocation. Do not read a moving trybuild victim as a flaky
  test in this repository.

- **The descriptor exhaustion did not recur, so it was environmental after
  all.** The second post-rebase run of `make test`, on a quieter machine and
  with admission at capacity 4 rather than at the queued ceiling, passed the
  trybuild `compile` target 10/10 on the *default* invocation — no
  `--test-threads=1`, no `Too many open files`, and no moving victim. That
  confirms the diagnosis recorded above rather than overturning it: the failure
  tracks the machine's descriptor pressure, not the repository. The practical
  consequence is that the serialised fallback is a workaround for a contended
  host, not the target's deterministic evidence, and a future red trybuild run
  on a busy host should be re-run on a quieter one before it is believed.
- **A stale remote-tracking ref looked exactly like a diverged branch, and
  nearly caused a false alarm about lost work.** The local
  `refs/remotes/origin/<branch>` said `f9e332b` while `gh pr view` reported the
  PR head as `820adf4`, and the two appeared to share no ancestry below the old
  merge base — which would have meant a competing push had landed work this
  worktree never saw. Re-fetching with an explicit refspec
  (`+refs/heads/<branch>:refs/remotes/origin/<branch>`) reported a forced
  update and resolved it: `820adf4` was an ancestor of the local tip all along,
  and the tracking ref was merely stale. The lesson is that the tracking ref is
  not the remote — `gh pr view --json headRefOid` and an explicit-refspec fetch
  are the authorities — and that a scare of this shape should be settled by
  fetching, never by resetting to the ref that raised it.
- **The rebase's only conflict was an append-append, and the union is the
  correct resolution.** Main and this branch both added a section to
  `docs/users-guide.md` at the same position, so a resolution that took either
  side alone would have silently deleted the other's documentation. The seam is
  also a useful reminder that an append-append conflict is the one case where
  reading the *diff* rather than the conflict markers gives the whole picture:
  git presents it as a disagreement about one hunk, not as two independent
  additions to preserve.
- **Clippy's preferred spelling of an `Option` chain is one Verus cannot
  compile, so the kernel bodies must be written in the form the prover accepts
  and the lint is asked to accept it there.** `open_block` reduces a
  `LineFacts` to its opening state through two `Option` projections, which
  `clippy::manual_map` reports as a nested `match` and would rewrite as
  `.and_then(..)`. `and_then` is a call to an `impl` block's method on a
  reference, and the pinned Verus release rejects it outright:
  `error: core::option::impl&%0::and_then is not supported`. The fix is not to
  silence the lint — the file stays clean under `-D warnings` — but to write
  the projection with two `let Some(..) = .. else { return None; };`
  statements, which is idiomatic Rust, needs no `match`, and is the form the
  kernel already used elsewhere. The general lesson for this codebase is that
  Clippy's suggestion is not automatically applicable inside an `include!`d
  kernel body: a lint-clean rewrite of a kernel is only safe once Verus has
  verified it.
- **A base case that reaches the bottom of a recursive specification needs
  more than `reveal`.** `reveal(f)` supplies one unfolding, which is exactly
  enough for an inductive step that the induction hypothesis already carries
  the rest of. A base case is different: it asserts a *closed* statement about
  a one- or two-element sequence, and the solver must unfold the recursion all
  the way to the `upto <= 0` arm before the assertion is even meaningful. The
  three base-case failures in this session (`lemma_compressed_prefix` at a
  prefix of one line, and `lemma_witness_compressed_block_is_live` at three),
  were all fixed by `reveal_with_fuel(f, n)` with `n` set slightly above the
  number of nested unfoldings the assertion needs — 2 for a two-deep chain, 4
  for a three-element one. The heuristic that worked is to count the sequence
  elements the assertion mentions and add one, rather than to add fuel until
  the solver stops complaining.
- **Region equality for the whole document does not need a per-line transition
  proof, because a non-rewriting write reads back identically.** The obvious
  route — induct over the driver, carrying a region for every emitted line —
  needs the emitted reading of each line to be related to its source reading by
  a transition argument, which is a large composition problem. The route that
  worked is to note that only `Agendum::Compressed` changes what the classifier
  reads: a `Normalized` write preserves marker family, run length, depth and
  the blank-tag decision, so dropping a null language specifier is invisible to
  `fence_step`. Every other position is written verbatim. So the document
  theorem follows from a *pointwise reading agreement* — for each emitted line,
  its region under the emitted prefix equals its region under the source prefix
  — and the pointwise claim is one case split, with the single interesting case
  discharged by the guarded-interior lemma that was already proved for the
  block interior. This is the design insight that made the document theorem
  tractable, and it is worth restating before any future attempt to strengthen
  the result: the hard part is not the driver, it is finding the reading that
  is preserved by the writes the driver performs.
- **The fence-skipping consumers are all adapters over the shared kernel; the
  one independent reimplementation is currently unreachable.** A sweep of every
  decision site that skips fenced content found seventeen `FenceTracker`
  constructions in `src/`, one per pass that needs to know whether a line is
  inside a fence, and every one of them routes through `fence_step` — the
  streaming passes through `observe_source_line` or `observe_inner`, and
  `compress_fences` additionally through the kernel predicate
  `compression_changes_region`. Two qualifications belong on the record rather
  than being smoothed over. First, `src/classify_kernel_predicates.rs` does
  contain a genuinely independent fence recognizer — `is_fence_marker` and
  `is_closing_fence`, the latter without any blockquote-depth rule, and with an
  `OpenFence` that carries only a marker and a run length. It cannot currently
  disagree with the kernel in production because its `is_in_fence` field is
  only ever set true under `#[cfg(test)]`, and `LineClass::FenceMarker` has no
  production consumer; it is a drift hazard rather than an active
  inconsistency. Second, there are two distinct fence regexes —
  `src/wrap/fence.rs` and `src/fences.rs` — whose info-string and
  trailing-whitespace grammars differ; the second never decides literal-ness,
  only delimiter spelling and attachment eligibility, and its results are gated
  through the planner and the kernel predicate. Both qualifications are
  recorded as remaining external contracts in `docs/verification.md` rather
  than as discharged obligations.

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
- Decision: review against `origin/main`, not the local `main` ref. Rationale:
  local `main` was 29 commits behind `origin/main`, which is what the PR
  actually targets and is a direct ancestor of this branch. Reviewing against
  the stale ref reported a 255-file, 34,245-insertion diff; against
  `origin/main` it is the 17 files this work touched, which is the diff a
  reviewer needs. Date 2026-09-28.
- Decision: extract the planner into `src/fences/plan.rs` as a set of
  `verified_kernel_function!` / `verified_loop_function!` kernels, included by
  `verus/lib.rs` through a `#[path]` attribute, rather than proving a separate
  model of the pass. Rationale: ADR 0011's pattern is that the proof build
  compiles the production body, so the postconditions constrain the decisions
  the formatter actually makes. A parallel model would be free to drift from
  the pass it claims to describe, which is precisely the failure this issue
  exists to prevent. Date 2026-10-05.
- Decision: state the planner's postconditions against `crate::spec_*`
  functions resolved through a `pub use plan_spec::*;` glob in `verus/lib.rs`.
  Rationale: `src/fences/plan.rs` is compiled twice — once as an ordinary
  production module, once as a module included by the proof entry point — and
  the second form has only the entry point's exports to reach the
  specifications through, so a `crate::` path that resolves in neither build is
  the only spelling that works in both. The glob is load-bearing; removing it
  produces eight `E0425` errors pointing into the production file. Date
  2026-10-05.
- Decision: prove full Region-sequence equality, not merely region preservation
  on payload lines. Rationale: the pointwise reading-agreement argument (see
  Surprises) is no harder than the weaker statement once the reading is chosen
  correctly, and the full equality is the property whose executable counterpart
  `tests/fence_regions.rs` already asserts corpus-wide — so the theorem and the
  sweep now agree on a statement rather than on a weakened version of one. Date
  2026-10-05.
- Decision: keep the driver composition (M6 item 5b) open rather than closing
  issue #486 on the strength of the block-level result. Rationale: the user's
  specification requires that block buffering, conflict accumulation and
  emission not be connected to the specification only by tests, and the
  emission step is currently related to the specification by the external
  emission contract plus the corpus sweep. The block-level theorem is complete
  and is the larger half of the work, but marking the issue addressed while a
  required obligation is open would misreport the state of the proof. Date
  2026-10-05.
- Decision: audit the fence-skipping consumers by inspection and record the
  result as evidence rather than attempting to prove each adapter. Rationale:
  the obligation is that every pass which skips fenced content consumes the
  same classifier, which is a property of the call graph rather than of any one
  pass's arithmetic; the audit found seventeen tracker constructions all
  routing through `fence_step`, and the two qualifications it raised are
  recorded as external contracts precisely because they are not provable from
  what is currently extracted. Date 2026-10-13.
- Decision: resolve the rebase conflicts toward `main` wherever the two sides
  described the same fact, and toward the branch only where the branch's
  subject was absent from `main`. Concretely: `main`'s new braced macro grammar
  replaced the branch's unbraced loop-body form in the renamed
  `src/verified_kernel_macros.rs`; `main`'s ATX-separator mutation and its
  `awk`-based output matcher replaced the branch's classifier copies, with the
  branch's shared mirror builder routed underneath; and `main`'s descriptive CI
  step name for the classifier gate replaced the branch's shorter one. The
  branch contributed the fence kernel, the compression planner, the two new
  mutation gates, and the mirror builder, none of which `main` has. Rationale:
  every conflict was two descriptions of one artefact, so the newer description
  won and the branch's additions were kept alongside. Date 2026-10-13.
- Decision: bypass the Weave driver for the replay with a command-scoped
  override rather than reconciling the host configuration. Rationale: the skill
  names a host that still selects Weave from the global attributes file as
  unreconciled; this host has an Ansible-managed `*.rs merge=weave` entry and
  the repository's own `.gitattributes` contains only `* text=auto eol=lf`, so
  Weave is selected by ambient configuration and not by repository consent. A
  multi-commit replay across substantial target history is the case the skill
  says to bypass. Date 2026-10-13.
- Decision: record the `mdtablefix` PATH shadowing as an environment finding
  rather than changing the Makefile in this branch. Rationale: `make check-fmt`
  resolves `mdtablefix` through `$(MDTABLEFIX) ?= mdtablefix`, which lands on
  `/home/leynos/.cargo/bin/mdtablefix` (0.6.1) built before this branch's
  `src/` commits, so the gate's formatter is not the binary the branch builds.
  The two binaries agree exactly on the current tree (both report
  `42 files left unchanged`, exit 0), and the default is unchanged from `main`,
  so retargeting it is a repository-wide build-hygiene change rather than part
  of this proof's scope. Date 2026-10-13.

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

3. **LEM-REWRITE-PRESERVES-CLOSER-RELATION**, the guarded-interior lemma,
   stated one-sidedly over pre-parsed `LineFeatures`. For a line inside a
   block's interior — one at or below the opener's depth that does not close
   the original opener and that `spec_rewrite_permitted` admits — the line does
   not close the rewritten opener either, and *both* runs classify it as
   `Literal`. The conclusion is a non-closing relation over parsed features,
   not a statement about delimiter identity, because the two runs may
   legitimately disagree there: a tilde line closes a tilde opener but cannot
   close the three-backtick opener the pass writes in its place. Method:
   `proof fn lemma_rewrite_preserves_closer_relation` over the pure
   `spec_compressed` / `spec_rewrite_permitted` predicates. Non-vacuity: the
   mutation gate drops the marker-character comparison from `closes_fence` and
   must report a failed obligation naming that function's contract against
   `spec_closes`; the doc comment on the lemma states why that check is what
   makes it true, and `lemma_witness_conflict_is_rejected` pins the issue #480
   shape the guard exists for.

4. **LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS**, scoped to one pre-parsed
   block interior whose lines each satisfy the compression guard. For such a
   body the original and compressed seeded states produce equal region
   sequences at every prefix — proved by
   `lemma_normalization_preserves_regions` (the induction, via
   `lemma_state_preserved`) over `LineFeatures`, not over `compress_fences`.
   The guard is load-bearing rather than decorative: without it a rewritten
   opener is closed by an interior line that did not close the original, every
   later line leaves the literal region, and the conclusion fails. Non-vacuity:
   `lemma_witness_interior_is_literal` exhibits a body satisfying every
   hypothesis and pins the region both runs agree on, so the conclusion is not
   an empty sequence; `lemma_witness_conflict_is_rejected` proves the guard
   fires on the issue #480 shape.

   The document-level result is now a Verus theorem, not only corpus evidence.
   `verus/plan_spec.rs` states it over readings: a delimiter the planner
   respells conservatively is written in its source marker family and run with
   only a null language specifier dropped, and dropping a null specifier
   changes no field the transition reads, so the line reads back as itself and
   region equality follows from a pointwise reading agreement
   (`lemma_agreement_preserves_prefix`) with no block-level reasoning at all.
   That covers every abandoned block, every matched block whose endpoints are
   not both rewrite candidates, and every conflict-bearing block. Only a
   compressed write changes the reading, and there the written opener is exactly
   `spec_compressed` of the source opener — the second state
   `lemma_normalization_preserves_regions` already compares against. The two
   files meet exactly, and `lemma_no_conflict_all_clear` is where the planner's
   own conflict fold supplies that lemma's guard. `lemma_compressed_prefix`
   extends the block result over a compressed block's interior and
   `lemma_compressed_block_regions` over its closing delimiter, and
   `lemma_block_agenda_length` supplies the line-count half. Non-vacuity:
   `lemma_witness_compressed_block_is_live` pins the three regions of a four-
   backtick/three-backtick block to the concrete sequence
   `[Delim, Literal, Delim]`, and `lemma_witness_conflict_moves_region` states
   the issue #480 defect as a pair of region values, showing the hypothesis is
   not vacuous. `make verus-compression-mutation` rewrites `delimiter_agendum`
   to compress every delimiter it is handed, discarding the argument that
   carries the block's compression answer, and must fail.

   The remaining boundary is the pass's *driver*: the loop that calls
   `plan_step` and dispatches the flush paths. `plan_step`'s routing relation
   already models every release the pass can name, but the emitted readings are
   related to the source readings by the external emission contract rather than
   by a composition lemma. The sweep in `tests/fence_regions.rs` asserts
   `regions(compress_fences(lines)) == regions(lines)` for every fixture under
   `tests/data/` plus the four `UNCLOSED_DOCUMENTS` reproducers, with
   `tests/data/footnotes_fence_toggle_input.txt` pinned as a must-cover case
   and each sweep asserting a minimum checked count so it cannot pass over an
   empty corpus. That sweep is executable evidence for the driver, and the
   composition lemma that would replace it is recorded as remaining work below.

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

External contracts: the one assumed contract is the reduction of a source line
to `LineFeatures` by the regex-based fence recognition and blockquote parsing
that feed the kernel. Every obligation below is stated over already-reduced
features, so none of them depends on that reduction being correct, and the
kernel body itself runs no regex. Recognition is excluded from the proof build
by `#[cfg(not(verus_keep_ghost))]` and is recorded as the assumed contract in
the assumptions column of each fence row in the verification ledger, alongside
the classifier's existing matcher contract rows.

Axioms relied upon: the pinned Verus release and its standard library, treated
as an axiom per ADR 0011; and the `verus/lib.rs` char-conversion boundary
already recorded in the ledger.

## Outcomes & retrospective

Delivered, pending CodeRabbit review and marking the PR ready. Draft PR
[#588](https://github.com/leynos/mdtablefix/pull/588) is open. The
implementation, proofs, mutation gates, and formatting gate are green locally;
the Cargo gates that the shared package-cache deadlock blocks locally — Format,
Markdown lint, Lint, and the test suite (2544 run, 2544 passed, 0 skipped) —
pass in CI on `e810a01`, and the Windows `atomic write contract` job passes
there too. The deadlock is recorded in Surprises and remains an escalation for
the user rather than a defect in this work.

The formatter now classifies every line of a document through one pure
transition kernel, and the region-preservation theorem is machine-checked
against that same production body. `make verus` reports 81 verified functions
with no errors, `make verus-fence-mutation` confirms the proof fails for the
intended reason when the closing rule is weakened, and `tests/fence_regions.rs`
replays the argument over every fixture in `tests/data/` plus four whole-file
documents that reach the unclosed-fence path.

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

Residual gaps, recorded rather than hidden: the claims hold for pre-parsed line
features, not for the parse that produces them, as the external contract above
sets out. The `compress_fences` pass itself is not proved either; what is
proved is the predicate it consults and the region-level consequence of
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
`FenceState` are defined there. The parent's `line_features` derives the
kernel's features from the same regex parse the tracker already performs,
`observe_step` calls `fence_step`, and the result is mapped onto
`FenceObservation`; the tracing events and their `transition` / `reason` values
are preserved and `FenceTracker`'s public API is unchanged. Committed as
`95c27a3`.

As planned, the signature is narrower than the milestone text anticipated:
`LineFeatures` carries the marker character, run length, blockquote depth, and
a trailing-whitespace-only flag, and the *presence* of a marker is that field's
`Some`, so no separate fence-marker flag was needed. Recognition stays in the
parent's `line_features`; the kernel is pure over pre-parsed features.

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
`advance_fence_block` consults the predicate directly rather than accumulating
a regex-derived flag. `opening_rewrite` maps the block-level guard onto a
`Strategy`, and both flush paths take their `Strategy` from it:
`flush_unmatched_block` rewrites only the opening delimiter,
`flush_matched_block` both delimiters, `flush_original_block` neither — chosen
by `flush_completed_block` from the cached rewrites. Committed as `fbd31de`.

Acceptance met: `make test` passes including every existing fence test; both
flush paths call `opening_rewrite`, and all four emit delimiter lines through
the shared `rewrite_fence_line`.

### M3 — Verus proofs and ledger rows (complete)

`verus/fence_spec.rs` states the specification and `verus/lib.rs` includes
`src/wrap/fence/kernel.rs` through `#[path]`, so the proofs constrain the body
the formatter runs. The spec functions are `spec_fence_next`,
`spec_fence_region`, `spec_state_seeded`, `spec_regions_seeded`, `spec_closes`,
`spec_agrees_with_opener`, `spec_compressed`, `spec_interior_delimiter`,
`spec_compression_changes_region`, and `spec_rewrite_permitted`. Every kernel
decision carries a postcondition tying it to its spec function.

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
still rejects the smoke proof; `make lint`'s `check-verification-ledger`
accepts the new rows; the mutation gate fails for the intended reason.

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

Completed in full. `docs/verification.md` gained the fence-kernel section and
six ledger rows; `docs/developers-guide.md` gained the fence-kernel paragraph
and the corrected `verified_kernel_macros.rs` path; `docs/users-guide.md`
gained the batch-classification section. The gates listed in Progress ran
locally or in CI as recorded there, `coderabbit review --agent` was requested
and every concern cleared across four rounds, and draft PR
[#588](https://github.com/leynos/mdtablefix/pull/588) was opened and then
marked ready for review.

Rebased twice since (see Progress), most recently onto `origin/main` at
`425cd2f`, and published as `NEW_HEAD` `860a166` with the force-push leased to
the previously recorded remote head `820adf4`. The remote-tracking ref was
refreshed with an explicit refspec and its head confirmed equal to local
`HEAD`; the branch upstream now points at `origin` and no commits are unpushed.
The branch is 36 commits ahead of `origin/main`.

### M6 — Outstanding work from the review round (in progress)

The request that carried the second rebase also listed work that is **not yet
done**, and none of it is recorded as complete. It is tracked here so the gap
is not lost.

Delivered since that request: the rebase itself, the four gates, and this
record.

Each item below, in the order the request gave it, with its current state.

1. A failed hosted-advisory check table names four checks. The
   `Unit Architecture` error was reported against `tests/fence_regions.rs`'s
   `data_files` returning an empty or partial corpus on failure; that function
   already returns `Result<Vec<PathBuf>, Box<dyn Error>>` at the tip, so the
   finding is either stale or about a different revision, and it needs
   re-checking against the check's own evidence rather than assumed fixed.
   **Not re-checked.** The advisory runs on the hosted service, so this needs
   the check's own output rather than a local re-run.
2. `Linked Issues` (warning): extend the formal result to the
   `compress_fences` boundary, or record an explicit accepted proof boundary in
   issue #486 and establish cross-pass agreement for every fence-skipping
   consumer. **Partly done.** The formal result now reaches every emission
   decision `compress_fences` makes, because the planner it calls is the module
   `verus/lib.rs` proves and the emitter writes what the planner's agenda
   names. The cross-pass agreement was audited by inspection: seventeen
   `FenceTracker` constructions in `src/`, one per pass that needs to know
   whether a line is inside a fence, every one routing through `fence_step`.
   The audit raised two qualifications, recorded in `docs/verification.md` as
   remaining external contracts rather than as discharged obligations: the
   classifier kernel's own `is_fence_marker` / `is_closing_fence` pair is a
   genuinely independent fence recognizer that cannot currently disagree in
   production only because its `is_in_fence` field is set true solely under
   `#[cfg(test)]`; and there are two fence regexes whose info-string and
   trailing-whitespace grammars differ, with the second never deciding
   literal-ness. **The composition law itself now exists**
   (`lemma_spans_compose_regions`), together with the translations that make
   block-level results usable at any offset, so the per-span obligation is
   proved and each release route's is named by `lemma_released_span_composes`.
   The remaining gap is narrower than it was and precisely stated: the
   induction over `spec_drive` that would instantiate the law at the fold is
   not proved, because the emitted sequence is not a one-line-per-step
   extension of the fold — see the new entry under *Surprises & discoveries*.
   Emitted readings are therefore still related to source readings by the
   external emission contract plus the corpus sweep, and **this item stays
   open**.
3. `User-Facing Documentation` (warning): document `classify_regions` and
   `Region` in `docs/users-guide.md`. The section landed in the rebase and
   auto-merged, but the advisory has not been re-run against it.
4. `Developer Documentation` (warning): update `docs/developers-guide.md` for
   the fence kernel, the `LineFeatures` boundary, `FenceTracker` routing,
   compression decisions, and `make verus-fence-mutation`, and correct stale
   references. **Done.** The `### Strategy enum` section was rewritten to state
   that `rewrite_marker` returns an `Option` rather than falling back
   internally, that `opening_rewrite` derives the block strategy from
   `compression_changes_region`, that `flush_completed_block` chooses between
   the matched and original flushes and `flush_unmatched_block` takes an
   unclosed block directly, and that `ParsedLine::observe` is the single parse
   every flush path consumes. The `src/classify_kernel_macros.rs` path was
   already corrected to `src/verified_kernel_macros.rs`, and
   `docs/adrs/0006-single-pass-idempotence.md:322` and `:397` were checked:
   `fences::flush_unmatched_block` still exists and still rewrites the opener
   while emitting the interior verbatim, so those references are accurate and
   were left alone. The planner extraction superseded that description in part —
   `opening_rewrite` and `flush_completed_block` no longer exist, because
   `plan_step` names the release and `flush_block` writes the agenda — so the
   section needs a second pass against the current tree.
5. The document-level fence-compression preservation proof: extract a
   Verus-compatible compression planner and emitter that `compress_fences`
   actually calls, so block buffering, conflict accumulation, and emission are
   connected to the proof by refinement rather than only by tests. **Done for
   the block level; the driver composition remains.** Commit `3846774` landed
   `src/fences/plan.rs` (the six planner kernels), `verus/plan_spec.rs` (the
   document theorem, its conflict bridge, the compressed-block induction, and
   the non-vacuity witnesses), the `verus/lib.rs` wiring, and the
   `compress_fences` refactor onto the planner. `make verus` reports 108
   verified, 0 errors. The parked WIP ref `wip/486-plan-kernel` (`8c022dc`) is
   superseded by that commit and can be deleted. **M6 item 5b, the driver
   composition, is open**: the emitted readings are related to the source
   readings by the external emission contract and the corpus sweep rather than
   by a composition lemma over the pass's buffered-block fold, and the user's
   specification requires that block buffering, conflict accumulation and
   emission not be connected to the specification only by tests.
6. The compression mutation gate, wired into the Makefile and CI. **Done.**
   `make verus-compression-mutation` runs
   `scripts/check-compression-mutation.sh`, which mutates the `compressible`
   guard to `true` and requires the run to fail with the `delimiter_agendum`
   postcondition named. Commit `3d176e2` added all three mutation gates to
   `.github/workflows/verus.yml`, so the classifier, fence, and compression
   negative controls all run on every pull request, and
   `tests/verus_harness.rs` asserts each target is named there so a gate cannot
   be dropped from CI unnoticed. The gate reports its evidence: the mutated
   body, the failed postcondition, and a confirmation line naming the
   rejection. Commit `1b2d3fb` added the same echo to the fence and compression
   gates, which had checked the same three conditions and then fallen silent on
   success.
7. A new `/comenq-coderabbit` review. **Requested** against the rebased head;
   see the revision note.

Issue #486 stays open until its original acceptance criteria are met. The
current state proves the block-interior lemma, the document theorem up to the
driver, and provides corpus-wide executable evidence; the driver composition in
item 5b is the part the request specifies that remains open. Items 1, 3, and 4
are confirmed satisfied against the published tip. Item 2's cross-pass
obligation is discharged by inspection plus the recorded external contracts
rather than by proof.

## Revision note

- 2026-09-28: Initial draft. Reconnaissance found both prerequisite defects
  already fixed on main and a working Verus harness, so the supplied Task 1 is
  recorded as verification rather than construction, and the plan proceeds from
  the kernel extraction.
- 2026-09-28: M0 completed with reproduction evidence, and M4 resequenced ahead
  of M3 to protect the deliverable against the proof tolerance.
- 2026-10-04: Rebased onto `main` at `60eb907` (34 commits replayed, one
  conflict each in `Makefile` and `src/wrap.rs`, both resolved by keeping the
  branch's change and `main`'s `test-workflow-contracts` addition). Audited with
  `range-diff` (only the `Makefile` hunk differs from the pre-rebase commit,
  and only by `main`'s own addition) and `git diff --check` (clean). The
  hosted-review remediation the ExecPlan recorded as in-flight is confirmed
  present in the rebased tree: `data_files` already returns `Result`, and the
  literal-line sweep already walks the two literal subsequences in step.
- 2026-10-04: M5 section brought into agreement with Progress, and the
  verification plan's obligations 3 and 4 restated to match the proofs actually
  delivered: obligation 3 is the one-sided guarded-interior lemma, and
  obligation 4 is scoped to pre-parsed block bodies meeting the compression
  guard, with whole-pass equality recorded as corpus evidence rather than a
  Verus theorem.
- 2026-10-13: Rebased a second time, onto `origin/main` at `425cd2f` from
  `f561055`, replaying 36 commits over the exclusive boundary `60eb907` (the
  parent of the branch's first own commit, confirmed an ancestor of
  `origin/main`, with zero merges in range). One conflict, in
  `docs/users-guide.md`, from main and this branch both appending a section at
  the same point; resolved as a union, branch section first. `range-diff` shows
  35 of 36 commits byte-identical with only that doc commit differing, and
  `git diff --check` against the target is clean. `NEW_HEAD` `972eb7d`. All
  four requested gates pass on the rebased tree (`check-fmt`, `typecheck`,
  `lint`, `test`), and `make test` is green on its default recipe — the
  descriptor exhaustion from 2026-10-04 did not recur.
- 2026-10-13: M6 items 1 and 3 re-checked against the published tip and their
  advisories are satisfied without further change: `data_files` and
  `read_fixture` in `tests/fence_regions.rs` both return `Result` and both
  sweeps propagate with `?`, and `docs/users-guide.md` documents
  `classify_regions` and all three `Region` variants. M6 item 4 completed by
  rewriting the `### Strategy enum` section; the two
  `docs/adrs/0006-single-pass-idempotence.md` references were checked and are
  accurate. M6 item 2 is the outstanding one.
- 2026-10-13: Rebased a third time, onto `origin/main` at `bf6c46b` from
  `2df4c4a`, replaying 61 commits over the exclusive boundary `425cd2f` (the
  parent of `main`'s single squash commit, confirmed linear with zero merges in
  range). Seven conflicts, in `verus/lib.rs`, `src/verified_kernel_macros.rs`,
  `scripts/check-classifier-mutation.sh`, `tests/verus_harness.rs`,
  `.github/workflows/verus.yml`, `docs/verification.md`, and
  `docs/developers-guide.md`; every one was two descriptions of a single
  artefact, resolved toward `main` where `main`'s wording was newer and toward
  the branch only where the branch's subject was absent from `main`.
  `range-diff` shows twelve changed commits and no dropped work; the sole
  apparent deletion is the branch's rename of `src/classify_kernel_macros.rs` to
  `src/verified_kernel_macros.rs`, whose body is `main`'s byte for byte. The
  replay bypassed the Weave driver with a command-scoped override because this
  host still selects Weave from a global attributes block; see the surprises
  entry. `NEW_HEAD` `2965b52`, force pushed with a lease bound to the recorded
  remote head `2df4c4a`. All ten gates pass on the rebased candidate:
  `make verus` reports 172 verified, 0 errors; `verus-selftest` rejects the
  false proof as designed; each of the three mutation gates fails verification
  with its own named postcondition; and `check-fmt`, `markdownlint`,
  `typecheck`, `lint`, and `test` are green (`test` ran 69 suites). Post-rebase
  fixes landed as `a0864e1` (formatter re-wrap after the hand splices),
  `1b2d3fb` (mutation-gate evidence), and the ExecPlan record.
