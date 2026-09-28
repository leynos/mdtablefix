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
      `src/wrap/tests/fence_kernel_tests.rs`. The two documentation warnings are
      in flight.
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

Milestones M4 and M3 are sequenced ahead of M3's riskier proof work, so that a
proof that breaches its tolerance leaves a complete, useful deliverable behind.
See the decision log.

## Surprises & discoveries

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

3. **LEM-REWRITE-PRESERVES-CLOSER-RELATION.** Stated one-sidedly, as the proof
   turned out to require. For a guarded interior line — one that does not close
   the original opener and that `spec_rewrite_permitted` admits — the line
   closes the rewritten opener no more than it closed the original, and both
   runs classify it as `Literal`. Method:
   `proof fn lemma_rewrite_preserves_closer_relation` over parsed
   `LineFeatures` and the pure `spec_compressed` / `spec_rewrite_permitted`
   predicates. The symmetric biconditional is *false*, and the Surprises entry
   records why: a tilde line closes a tilde opener but not the three-backtick
   opener the pass writes. Non-vacuity: the mutation gate drops the
   marker-character check and must report `postcondition not satisfied` naming
   the `spec_closes` contract; the doc comment on the lemma states why that
   check is what makes it true.

4. **LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS**, for one pre-parsed block
   interior. For a body whose lines each satisfy the compression guard, the
   original and compressed seeded states produce equal region sequences at
   every prefix — proved by `lemma_normalization_preserves_regions` (the
   induction, via `lemma_state_preserved`) over `LineFeatures`, not over
   `compress_fences`. Non-vacuity: `lemma_witness_interior_is_literal` and
   `lemma_witness_conflict_is_rejected` supply both directions, and the guard
   is load-bearing rather than decorative — without it a rewritten opener is
   closed by an interior line that did not close the original, which is issue
   #480.

   Whole-pass equality is *not* claimed as a Verus theorem, because
   `compress_fences` buffers, caches, and has two flush paths that the proof
   does not model. What discharges it is executable corpus evidence: the sweep
   in `tests/fence_regions.rs` asserts
   `regions(compress_fences(lines)) == regions(lines)` for every fixture under
   `tests/data/` plus the `UNCLOSED_DOCUMENTS` reproducer, and the issue #480
   shape is pinned as a required case. Subject to the first tolerance.

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

## Revision note

- 2026-09-28: Initial draft. Reconnaissance found both prerequisite defects
  already fixed on main and a working Verus harness, so the supplied Task 1 is
  recorded as verification rather than construction, and the plan proceeds from
  the kernel extraction.
- 2026-09-28: M0 completed with reproduction evidence, and M4 resequenced ahead
  of M3 to protect the deliverable against the proof tolerance.
