# Add `--check` and `--diff` reporting modes to the `mdtablefix` CLI

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: COMPLETE — `EP-M0`–`EP-M7` are discharged. `EP-M6` ran after the rebase
onto `408c76a`, the upstream fix for the non-idempotent `--headings` class that
had blocked it: the milestone's own command found 46 mutants and the run
finished with 39 of the 40 viable ones killed, 6 unviable, and one equivalent
mutation recorded with its justification rather than papered over. See
`Progress`, `Outcomes & retrospective`, and `Artefacts and notes → EP-M6 run`.

## Purpose / big picture

Today `mdtablefix` can only print reformatted Markdown to standard output or
rewrite files in place. There is no way to ask "would this command change
anything?" without either discarding the answer or mutating the working tree.
That makes `mdtablefix` unusable as a continuous-integration formatting gate.

After this change a user gains two read-only reporting modes.

`mdtablefix --check FILE...` reads every supplied file, computes what the
formatter would write, and reports each file that would change together with
the number of lines that would be inserted and deleted. It writes nothing to
disk. It exits `1` when at least one file would change and `0` when none
would.

`mdtablefix --diff FILE...` performs the same analysis but prints a unified
diff for each file that would change. It also writes nothing, and it exits `1`
when at least one file would change, so a single `--diff` run both shows the
drift and fails the build.

Observable success looks like this, given a ragged `broken.md` and an
already-formatted `clean.md`:

```console
$ mdtablefix --check broken.md clean.md
broken.md +3 -3
1 file would be reformatted, 1 file left unchanged.
$ echo $?
1
$ mdtablefix --diff broken.md
--- broken.md
+++ broken.md
@@ -1,3 +1,3 @@
-|A|B|
-|---|---|
-|1|2|
+| A | B |
+| --- | --- |
+| 1 | 2 |
$ echo $?
1
$ mdtablefix --diff clean.md; echo $?
0
$ cmp --silent broken.md broken.md.orig && echo unmodified
unmodified
```

Delivering this honestly requires two further changes, both of which exist
because a check mode that lies is worse than no check mode at all.

First, line-ending preservation. The formatter currently normalizes every
input to LF, so on a CRLF file `--check` would report that every line changes
even when no Markdown content changes. That is GitHub issue #451, whose own
rationale says line-ending normalization "prevents check-only formatting gates
from comparing formatter output directly with valid CRLF source files".

Second, byte-order-mark handling. `Dir::read_to_string` retains a leading
U+FEFF, so the first line of a BOM-prefixed file becomes `\u{FEFF}|A|B|`, the
table detector never matches, no reflow happens, and `--check` reports **clean
on a genuinely ragged file**. A gate that passes while the tree is dirty is
the worst possible failure for this feature, so the BOM must be split off
before formatting and restored on output.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- `--check` and `--diff` must never write to, create, truncate, or change the
  modification time of any file, nor create any adjacent file such as a
  backup or lock. This must be enforced by the type system: the read-only
  path receives a capability that has no write method, so a wrong `match` arm
  cannot write. It must not rest on a test double alone.
- `--check` must report drift for a file if and only if `--in-place` would
  alter that file's bytes, for the same transform flags. Both must derive
  their answer from one `Assessment` value produced by one function, and the
  transform closure must be constructed once and shared, not per mode.
- Drift produces exit `1` under the two read-only reporting modes, `--check`
  and `--diff`, and never outside them. In particular a successful
  `mdtablefix --in-place` over drifting files exits `0`, as does a bare
  invocation. Exit status is therefore a function of mode and observation,
  never of observation alone.
- `--check`, `--diff`, and `--in-place` are mutually exclusive, rejected by
  argument parsing before any file is read. `--check` and `--diff` are two
  renderings of one analysis with identical exit semantics, so combining them
  would be redundant rather than useful; see `Decision log`.
- Report output must be deterministic: no timestamps, no colour under any
  circumstances, no wall-clock-dependent diff configuration, no
  locale-dependent formatting. `similar`'s `TextDiffConfig::timeout` and
  `deadline` are therefore forbidden.
- Diff computation must be bounded without sacrificing determinism. Above a
  fixed line-count threshold the implementation switches to
  `similar::Algorithm::Patience`, which is deterministic; it must never use a
  time-based cut-off.
- The line delta must not be computed for a file whose bytes are unchanged.
  `is_changed()` is a byte comparison; the diff runs only when it is true.
  This is what keeps a clean-tree gate cheap.
- Multi-file report order must equal command-line argument order. This must be
  guaranteed by construction, by tagging each unit of work with its argument
  index and ordering on that index, not by relying on `rayon`'s collection
  order. See `AX-4` in `Verification plan` for why.
- Existing behaviour of a bare invocation, `--in-place`, and standard input
  mode must be preserved apart from three deliberate changes recorded in
  `Decision log`: operational errors exit `2` rather than `1`; output line
  endings follow the input; and a leading byte-order mark is preserved rather
  than being fed to the formatter.
- Standard input mode currently prints a single newline for empty input, which
  `tests/parallel.rs:20-24` asserts as `.stdout("\n")`. Preserve it exactly.
- `--in-place` without file arguments must remain a usage error, as
  `tests/cli.rs:28-35` asserts. `--check` and `--diff` inherit the same rule.
- No transform logic changes. `src/process.rs`, `src/table.rs`, `src/wrap/`,
  `src/footnotes/`, and the other transform modules are out of bounds. Every
  body transform continues to route through
  `mdtablefix::process::process_with_frontmatter`.
- The library must not export an opaque error type. `AGENTS.md:266-270`:
  "Never export the opaque type from a library. Convert to domain enums at API
  boundaries, and to `eyre` only in the main `main()` entrypoint." The new
  library modules are therefore infallible, and `anyhow` stays in the binary,
  which is the application boundary.
- No Rust source file may exceed 400 lines. `tests/cli.rs` is at exactly 400
  and `docs/execplans/cli-matrix-testing.md:51-54` forbids growing it.
  `src/process.rs` is at 375 and is out of bounds for this work. Per-file
  budgets are given in `Interfaces and dependencies`.
- Report lines go to standard output; the human summary and all diagnostics go
  to standard error, so `--check`'s standard output is a clean machine
  contract.
- Dependency requirements use caret ranges, per `AGENTS.md:249-255`.
- Clippy warnings are denied; suppressions are a last resort, tightly scoped,
  and carry a `reason`.
- Every new module opens with a `//!` comment; every public item carries `///`
  documentation; attributes follow doc comments.
- Prose is en-GB-oxendict per `docs/documentation-style-guide.md`.

## Tolerances (exception triggers)

- Scope: stop and escalate above 32 files or roughly 1700 net lines across
  source, tests, and documentation.
- Dependencies: one runtime dependency (`similar`) and four development
  dependencies (`rstest-bdd`, `rstest-bdd-macros`, `googletest`,
  `pretty_assertions`) are proposed and must be approved at the gate. Any
  further dependency stops work. `cargo-mutants` in `EP-M6` is a developer
  tool, not a manifest entry; if it is unavailable, skip that step and record
  the gap.
- Interface: stop if any existing public library signature must change
  incompatibly. Also stop if `io::rewrite`'s or `io::rewrite_no_wrap`'s
  observable **behaviour** changes in any way not explained by line-ending or
  byte-order-mark preservation; signature stability alone is not sufficient
  protection for existing library consumers.
- Idempotence: if `INV-IDEMPOTENT` fails, meaning `--check` reports drift on
  the formatter's own output, stop immediately. That indicates a
  non-idempotent transform, which is a pre-existing defect outside this plan's
  scope and makes the feature unusable as a gate. Record the failing input and
  escalate rather than working around it.
- Behaviour: if line-ending or byte-order-mark handling changes any existing
  test fixture's output in a way not explained by those two concerns, stop.
  That indicates a transform regression.
- Snapshots: `EP-M5` deliberately raises the usual snapshot-churn limit to 40
  new or changed snapshots, because expanding the option matrix from two
  execution modes to four cannot be done under a limit of 30. If the actual
  churn exceeds 40, stop and narrow the curated subset further.
- Performance: if `--check` over the repository's own `docs/` tree takes more
  than two seconds on a warm cache, stop and investigate before adding
  features.
- Iterations: if a gate still fails after three focused fix cycles, stop and
  record the command, the `/tmp` log path, and the likely cause.
- Ambiguity: if any requirement admits two readings that produce materially
  different user-visible behaviour, stop and present the options.

## Risks

- Risk: a transform may not be idempotent, so `--check` would report drift on
  the formatter's own output and the gate could never go green. Severity:
  critical. Likelihood: low but unquantified. Mitigation: `EP-M2` adds
  `INV-IDEMPOTENT` as a property test over the existing fixture corpus
  *before* any CLI surface exists, so the answer is known early and cheaply.
  `tests/cli.rs:307-356` already round-trips `--in-place` twice for its own
  cases, which is partial evidence but not a general result.
  Materialised, twice, as predicted: two classes under `--wrap`/`--breaks`
  (issue #468, fixed by pull request #470) and a third under `--headings`
  (issue #474, fixed upstream by pull request #477, merged as `408c76a` and
  absorbed by this branch's rebase). All three are pre-existing and none is
  reachable through
  `make fmt`'s flag set today. The mitigation worked as intended — the answer
  was known before any CLI surface existed — but the *unquantified* likelihood
  in the original wording is now measured and is not low: the third class is
  reachable by the property test's own generator. See
  `Surprises & discoveries`.

- Risk: the `INV-IDEMPOTENT` property is flaky rather than merely imperfect, so
  a green `test` gate is not evidence that the invariant holds. Severity: high.
  Likelihood: was confirmed, and the class behind it is now fixed. Mitigation:
  none available inside this plan — the generator draws from a large space and
  reaches the failing shape in only a fraction of runs, so `with_cases(48)`
  sometimes passes and sometimes fails on an unchanged tree. Pinning a passing
  seed or excluding the test would turn the gate green while the counterexample
  stands, which `Tolerances` forbids. The fix was left where it belonged, with
  the transform defect: issue #474 required the generator to reach the shape
  deterministically, and pull request #477 instead removed the absorption the
  shape exposed. It also landed the deterministic fixtures this plan's
  `Artefacts and notes` asked for, as `tests/data/idempotence/T1`–`T6`. The
  sampled shape is a fixed point now, so the property has been green in every
  run since, and Revision 17 measures it green inside a fully green `make test`
  (1860 passed, 0 failed). That retires this class and not the sampling: the
  property still draws from a fraction of a large space, so a green run stays
  evidence rather than proof. The fixtures are what make the shape's presence in
  the suite unconditional rather than sampled.

- Risk: `EP-M1` rewrites the serialization path used by every mode, including
  the one that mutates users' files, and the repository has **no existing
  CRLF, byte-order-mark, lone-`\r`, or empty-file coverage at all** (verified:
  the only `\r\n` anywhere is a tokenizer unit test at
  `src/wrap/tokenize/mod_tests.rs:72`). Severity: high. Likelihood: high.
  Mitigation: `EP-M1` adds those fixtures **before** the refactor, so the
  regression oracle actually covers the inputs the change is about. The claim
  that the existing suite is a sufficient oracle was wrong and has been
  removed.

- Risk: whole-file majority line-ending detection homogenizes endings inside
  fenced code blocks. A mostly-CRLF document containing an LF-authored shell
  snippet has that snippet rewritten to CRLF, which is a content change, not a
  formatting change. Severity: medium. Likelihood: medium. Mitigation: issue
  #451 specifies majority detection, so this plan implements it, but adds a
  mixed-endings-inside-a-fence fixture and documents the behaviour explicitly
  in `docs/users-guide.md`. Flagged for the approval gate as a consequence the
  requester should confirm.

- Risk: `similar`'s default Myers algorithm is O(ND); `--wrap` on a large
  unwrapped document changes nearly every line, so D approaches 2N and the
  cost approaches O(N squared). A gate that hangs is worse than one that is
  wrong. Severity: medium. Likelihood: low. Mitigation: a deterministic
  line-count threshold switches to `Algorithm::Patience`. Time-based cut-offs
  are forbidden by `Constraints` because they would make snapshots unstable.

- Risk: changing operational errors from exit `1` to exit `2` silently breaks
  a caller written as `mdtablefix ...; [ $? -eq 1 ] && handle_failure`, which
  would stop firing. Severity: medium. Likelihood: low. Mitigation: bump the
  crate from `0.5.1` to `0.6.0`, record it in `CHANGELOG.md` and ADR 0009, and
  assert all three statuses explicitly. Note that `tests/parallel.rs:56-103`
  uses `assert_cmd`'s `.failure()`, which only checks for non-zero and would
  not have caught this.

- Risk: `--in-place` uses create-truncate-write with no atomic rename and no
  backup, so a kill or a full disk leaves files truncated. Severity: high.
  Likelihood: low. Mitigation: **discharged.** This plan kept it out of scope
  and raised it as GitHub issue #465, which pull request #467 then fixed while
  this plan was halted at `EP-M2`. The rebase took that fix, and
  `src/main.rs` now writes through `replace_file`'s write-then-rename. See
  `Artefacts and notes → Rebase onto origin/main`.

- Risk: `rstest-bdd` has never been used here, and its user's guide does not
  cover subprocess testing, so the harness is a repository-local convention
  being invented alongside the feature. Severity: medium. Likelihood: medium.
  Mitigation: `EP-M0` proves the wiring with a canary before anything depends
  on it, and carries an explicit fallback to plain `rstest` plus `assert_cmd`
  if the canary exceeds one day.

- Risk: promoting `similar` to a direct runtime dependency could duplicate it
  in the build graph. Severity: low. Likelihood: low. Mitigation: request
  `similar = "2.7"`, which unifies with the `similar 2.7.0` already resolved
  through `insta` (`Cargo.lock:449-458`, `Cargo.lock:1108-1111`); verify with
  `cargo tree --duplicates`.

- Risk: a file with no trailing newline always reports drift, because the
  formatter appends one. Severity: low. Likelihood: high. Mitigation: correct
  and intended; document it prominently in `docs/users-guide.md`.

- Risk: `similar` treats a lone `\r` as a line separator
  (`similar-2.7.0/src/text/abstraction.rs:112-115`) while `str::lines()` does
  not, so the two disagree about what a line is. Severity: low. Likelihood:
  low. Mitigation: `LEM-COUNT` is stated purely over `similar`'s tokenization
  so it is self-consistent, and the divergence is documented as a limitation.

## Progress

- [x] EP-M0 Prototyping spike: `similar` output shape, `rstest-bdd` wiring.
      Complete. `similar = "2.7"` resolves to a single copy in the graph; the
      unified-diff transcript matches the plan's expected console output
      byte-for-byte; the `rstest-bdd` canary passes and deleting a step
      definition is a compile error, so strict validation is active. The
      version bump to `0.6.0` landed with the manifest change, as
      `Concrete steps` directs. Spike and canary deleted. CodeRabbit review
      (post-gates) returned `review_completed` with zero findings.
- [x] EP-M1 Document boundary: byte-order-mark and line-ending preservation,
      closing issue #451. Eleven `.dat` fixtures and
      `tests/document_properties.rs` pinned the pre-refactor bytes first, then
      `src/document.rs` landed red-to-green, then `src/main.rs` and `src/io.rs`
      were routed through `SourceDocument` and the six affected expectations
      flipped. All gates pass; `CHANGELOG.md` records the byte change and that
      an already-rewritten file cannot be un-rewritten by reverting. The
      lone-`\r` limitation remains and stays on `EP-M7`'s documentation list.
      **Rebased onto `origin/main`:** pull request #469 had meanwhile landed an
      equivalent line-ending implementation as `src/io/line_endings.rs`, so the
      rebase took `main`'s for endings and re-landed only the byte-order-mark
      half, as `src/io/document.rs`. See
      `Artefacts and notes → Rebase onto origin/main`.
- [x] EP-M2 Pure reporting domain (`src/report/`), including the idempotence
      result. **Was halted at step 3** because `INV-IDEMPOTENT` failed: the red
      state was built and observed (`cargo test --test check_properties count`
      panics on the `todo!()` bodies; `cargo test --lib report` reports
      1 passed, 33 failed), the deterministic corpus cases passed, and the
      generated document found two pre-existing non-idempotent transforms. Per
      `Tolerances`, work stopped and the finding was escalated rather than
      worked around; see `Surprises & discoveries` and
      `Artefacts and notes → EP-M2 idempotence failure`. Raised as GitHub issue
      #468 with the reproduction corpus and its acceptance criteria. **Merged
      pull request #470 then fixed both defect classes and closed issue #468**,
      which unblocked the green step. `LineDelta::between`,
      `render_report_line`, `render_summary`, and `write_unified_diff` are
      implemented on the rebased tree; `cargo test --lib report` and
      `cargo test --test check_properties` are green, and every idempotence case
      passes. All three `Verification plan` negative controls were applied as
      temporary local mutations, each was rejected, and each was reverted; see
      `Artefacts and notes → EP-M2 green transcripts`.
- [x] EP-M3 Driver, read-only capability, `--check`, exit-status contract.
      Complete. `src/driver.rs` and `src/driver_tests.rs` are new, `src/main.rs`
      is now an adapter over `driver::analyse`, and
      `tests/features/check_mode.feature` with `tests/steps/reporting.rs` and
      `tests/bdd_reporting.rs` drive the real binary. All gates pass, and the
      post-gates CodeRabbit pass returned `review_completed` with zero findings
      on the pushed commit `6c9dd90`. See
      `Artefacts and notes → EP-M3 red and green transcripts`. The version bump
      to `0.6.0` is already in the manifest and unreleased, so this milestone
      changes the exit status under an existing version rather than raising it;
      the record moves to `ADR 0009`.
- [x] EP-M4 `--diff`, sharing `--check`'s exit semantics. Complete. `Mode::Diff`
      is wired through `src/main.rs` and `src/driver.rs`; the diff is rendered
      from the same `Assessment` the other modes use, so it cannot disagree with
      `--in-place` about what would be written. Covered by
      `tests/features/diff_mode.feature` (six scenarios),
      `tests/bdd_reporting.rs`, `tests/cli_diff.rs` (five tests), the `Mode::Diff`
      cells of the `src/driver_tests.rs` exit-status cross product, and the fourth
      mode in `tests/cli_check.rs`'s matrix. The `INV-DETERMINISTIC` negative
      control was applied and removed; it confirmed the hazard on the transition
      band and exposed a blind spot in the method, both recorded in
      `Artefacts and notes → INV-DETERMINISTIC negative control`. All six gates
      are green (43 suites, 1812 passed, 0 failed, 20 ignored), the post-gates
      CodeRabbit pass returned `review_completed` with zero findings on the
      pushed commit `cf8995d`, and the milestone's first gate run was red on two
      rustfmt diffs that `cargo fmt --all` resolved. See
      `Artefacts and notes → EP-M4 red and green transcripts` and
      `→ CodeRabbit review after EP-M4`.
- [x] EP-M5 Curated CLI matrix coverage for the two new modes. Complete. The
      reporting invariants live in the new `tests/cli_matrix/reporting.rs`;
      `ExecutionMode` grew `Check` and `Diff` with a shared `reports()`
      predicate, and each base row declares the reporting modes it runs.
      `row_000`, `row_010`, and `row_111` were chosen by measurement (only
      `row_010` unwrapped is a fixed point, so the subset covers the no-drift
      branch as well as the drifting one) and run both modes in both wrap
      variants. `cargo test --test cli_matrix` passes with 51 tests; the
      snapshot regeneration added twelve files and changed none of the 88
      pre-existing ones. `docs/developers-guide.md` gained the reporting
      subsection in the same commit, its fixture-extension rule now carries the
      staging reason, and its portability section no longer claims every
      snapshot reads `status: code: 0`. The first regeneration run was **red**
      and exposed a real gap in the diff invariant rather than a snapshot
      problem; see `Artefacts and notes → EP-M5 red and green transcripts`.
- [x] EP-M6 Targeted mutation testing of the counting and aggregation
      functions. **Was blocked before its first command; the blocker was
      cleared in Revision 17 and the milestone ran in Revision 18.** `cargo
      mutants --file
      src/report/delta.rs --file src/driver.rs` finds the 43 mutants and then
      refuses to test them, because its baseline `cargo test` fails:
      `tests/check_properties.rs::generated_documents_reach_a_fixed_point` has
      a deterministic counterexample (`document = "|1|2|\n|---|---|\n---",
      mask = 128`) from a **third** pre-existing non-idempotent transform class
      under `--headings`. Per `Tolerances`, the plan does not work around it and
      does not exclude the test to get a score; the class is raised as GitHub
      issue #474 and the milestone resumes once the `test` gate is green. See
      `Surprises & discoveries` and
      `Artefacts and notes → EP-M6 baseline blocked`. The `make test` gate was
      **red** at `a06bab6` as a result. **Cleared in Revision 17:** pull request
      #477 fixed the transform, the branch rebased onto it, and the gate runner
      measured the whole suite green — including
      `generated_documents_reach_a_fixed_point` itself, which now reports `ok`
      — so `cargo mutants`'s precondition is met and the milestone's first
      command can run. **Run in Revision 18:** that command collected **46**
      mutants at this tip, three more than the 43 enumerated at `a06bab6`
      (`4a599ed` added all three, as `Inputs::resolve` and the
      `Mode::InPlace if is_changed` guard), and its baseline `cargo test` passed,
      so a score exists: the first run took 14 minutes and returned
      **38 caught, 6 unviable, 2 missed, 0 timeouts**. One survivor,
      `LineDelta::has_changes`'s `deletions < 0`, was killed by an assertion
      added to `src/report/delta.rs`'s unit tests and re-measured as caught; the
      other, `Mode::Diff if is_changed` behaving as `if true`, is an equivalent
      mutation whose premise is pinned by a new `src/report/render.rs` test. A
      third, complete run over the final tree then measured the score in one
      reading rather than a sum: **39 caught, 1 missed, 6 unviable,
      0 timeouts**, the miss being the equivalent mutant. Final state: **39 of
      the 40 viable mutants killed, the fortieth recorded as equivalent**. See
      `Artefacts and notes → EP-M6 run`.
- [x] Interleaved: forward-compatibility with the `--git` plan (pull request
      #466). Four small changes, requested by that plan's author, that stop this
      branch's shape from foreclosing a second input source: the `mode` group now
      requires a named `inputs` group holding `files`; `driver::Inputs` replaces
      `main`'s `cli.files.is_empty()` test, so "no paths were named" is no longer
      conflated with "the source matched nothing"; input-resolution errors reach
      `exit_status` and exit `2` like every other operational failure; and
      `--in-place` no longer writes a file whose bytes would not change.
      `--git` itself is **not** implemented — file discovery is that plan's
      scope. `cargo test --bin mdtablefix` (46 passed) and
      `cargo test --test cli_check` (7 passed), `--test in_place_atomic`
      (10 passed) are green on the new tests. See `Decision log`,
      `Surprises & discoveries`, and
      `Artefacts and notes → Forward-compatibility for --git (#466)`.
- [x] EP-M7 Documentation, ADRs, changelog, and issue closure. Complete, in
      commit `95ec57c`. `docs/users-guide.md`
      gained a `Command-line usage` section (flag table, the three file modes,
      the exit-status table, how to read a report line, line-ending and
      byte-order-mark behaviour, the trailing-newline rule, the empty-glob
      hazard, and the symlink limitation); `README.md`'s flag list is now a
      synopsis linking to it; `docs/architecture.md` gained
      `Check and diff reporting` with a sequence diagram, a `report` and
      `driver` class, and corrected symbol names; `docs/developers-guide.md`
      gained the CLI driver and reporting architecture sections and lost the
      two stale `format_to_string` / `rewrite_in_place` blocks that named
      functions deleted in EP-M3; `CHANGELOG.md`, `docs/contents.md`, and
      `docs/v0-6-0-migration-guide.md` are updated; both ADRs are written;
      and both vendored guides carry a provenance header. Step 9 is measured
      rather than assumed: `make markdownlint` (34 files, 0 errors) and
      `make nixie` (10 diagrams) pass, and `make fmt` was declined with the
      drift table and the reasoning in
      `Artefacts and notes → make fmt measured, and declined`. Step 10 is done:
      #451 was already closed by an earlier milestone, and #452 is closed with
      a comment recording the `--concise` supersession. Step 11 was added in
      Revision 14 and is complete: `INV-PREDICTS` had been claimed against
      `tests/check_properties.rs`, which does not test it, and is now
      discharged by `tests/check_prediction.rs` and its corpus module, with
      both negative-control runs recorded. Step 12 was added in Revision 15
      and is complete: `docs/developers-guide.md`'s shared-closure argument no
      longer stops at the structure, and names the test that covers what the
      structure cannot. **At Revision 17 the plan is not yet `COMPLETE`:**
      `EP-M6` has not run, and the `make test` gate it depends on is green
      again — restored by an upstream fix rather than by anything this branch
      changed; Revision 18 runs the milestone and declares the plan `COMPLETE`.
      Revision 17
      rebases the branch onto `origin/main` at `408c76a`, the commit that fixes
      #474, and measures every gate green afterwards: `check-fmt`, `lint`,
      `typecheck`, `markdownlint` (34 files, 0 errors), `nixie`, and `test`
      (45 test binaries plus the doc-test target: 1860 passed, 0 failed, 20
      ignored). Gates for Revision 14, run through the gate runner:
      `check-fmt`, `lint`, `typecheck`, `markdownlint` (34 files, 0 errors),
      and `nixie` pass; `check_prediction` passes 11/11 inside `make test`,
      which is red only on `check_properties`; and the `--no-fail-fast` run
      executes all 43 binaries with that same single failure and passes the
      doctests, so no part of the suite is left unrun. Logs are listed in
      Revision 14. Revision 15 is a documentation revision, so its gate run is
      the Markdown pair: `markdownlint` (34 files, 0 errors) and `nixie` pass,
      and the Rust gates were not re-run because no Rust, test, or
      configuration file changed. Revision 16 closes the two threads Revision
      15 left open: the CodeRabbit review of Revision 15's own commit, which
      returned 0 findings in 55 seconds, and a re-measurement of the `make fmt`
      drift at that commit, which is the same ten files out of 35 tracked.

## Surprises & discoveries

- Observation: a leading byte-order mark defeats table detection entirely, so
  `--check` would report clean on a ragged file.
  Evidence: `Dir::read_to_string` returns the U+FEFF; `src/main.rs:126` feeds
  the result straight to `content.lines()`, making the first line
  `\u{FEFF}|A|B|`, which no table pattern matches.
  Impact: byte-order-mark handling moved from "not considered" into
  `Constraints` and `EP-M1`. Without it the feature's headline guarantee is
  false for a class of real Windows-authored files, and the failure is silent.

- Observation: `--in-place` over drifting files would have exited `1` under
  the first draft's design, because `Outcome::exit_code` mapped `Drift` to `1`
  unconditionally while `Mode` was not in scope.
  Evidence: the draft's own `INV-EXIT` said "`1` when at least one file drifts
  **under `--check`**", a qualifier no interface expressed.
  Impact: exit status is now a function of mode and observation. A test that
  `--in-place` on a drifting file exits `0` is a required acceptance item.

- Observation: `open_file_parent` (`src/main.rs:100-119`) returns a `Dir` for
  the parent plus the **bare file name**, so a store keyed on that name would
  have reported `a.md` for `--check docs/a.md`, contradicting the plan's own
  decision that paths echo as supplied. A single store also cannot serve
  `--check docs/a.md src/b.md`.
  Evidence: `src/main.rs:110-116`.
  Impact: storage key and display path are now separate parameters, and the
  capability is constructed per file inside the parallel stage.

- Observation: `rayon`'s `collect::<Vec<_>>()` does not document order
  preservation. `FromParallelIterator<T> for Vec<T>`
  (`rayon-1.12.0/src/iter/from_par_iter.rs:24-34`) routes through
  `par_extend` with no ordering statement; the documented order-preserving API
  is `IndexedParallelIterator::collect_into_vec`.
  Impact: `src/main.rs` already relies on this for stdout ordering. Rather
  than deepen that reliance, the new code tags each unit of work with its
  argument index and orders on it, removing the assumption entirely.

- Observation: `similar::DiffableStr::tokenize_lines` is a true partition for
  all inputs, including the final unterminated line and a lone `\r`.
  Evidence: `similar-2.7.0/src/text/abstraction.rs:101-127` — terminators are
  retained by inclusive ranges, a lone `\r` separates at lines 112-115, and
  the trailing fragment is pushed at lines 122-124.
  Impact: two texts have equal token sequences exactly when they are
  byte-equal, which makes `INV-AGREE` meaningful. It also means `similar` and
  `str::lines()` disagree about lone-`\r` files, recorded in `Risks`.

- Observation: `docs/users-guide.md` contains no command-line flag reference;
  the canonical list is in `README.md:52-106`, contradicting
  `docs/documentation-style-guide.md:103-129`, which places command-line
  reference material in the user's guide.
  Impact: `EP-M7` adds a proper interface section to the user's guide and
  reduces the README to a synopsis plus a link.

- Observation: `docs/state-machine-abstractions-roadmap.md` exists but is
  absent from `docs/contents.md`, against the style guide's own rule.
  Impact: a one-line index fix is included in `EP-M7`.

- Observation: `src/io.rs` duplicates `src/main.rs`'s read-format-write
  pattern including the trailing-newline rule, is never called by the binary,
  yet is public API documented in `README.md:202,217,259`. It also uses
  `std::fs`, against `AGENTS.md:232-234`'s preference for `cap_std`/`camino`.
  Impact: `EP-M1` removes the duplication by routing both through
  `mdtablefix::document`, so library consumers gain the same fixes. Migrating
  `src/io.rs` off `std::fs` is **not** in scope; issue #418 tracks that work.

- Observation: `.github/workflows/ci.yml` does not run `make test`; it runs a
  shared coverage action that recompiles with instrumentation, so new
  development dependencies are compiled twice per run.
  Impact: noted so the plan does not claim a CI cost it has not measured. No
  CI change is required.

- Observation: `make test` does not run doctests, because its recipe passes
  `--all-targets`, which cargo documents as excluding them. `AGENTS.md:154`
  claims the gate runs `cargo test --workspace`, which would include them, so
  the Makefile diverges from its own documentation.
  Evidence: `grep -niE 'doc-?test'
  /tmp/test-mdtablefix-check-option.out` returns nothing, while
  `cargo test --doc --all-features` reports `28 passed; 0 failed; 20 ignored`.
  Impact: this plan adds doctests to every new public item, so their contract
  would otherwise be unverifiable by the deterministic gates. Mitigation: the
  `test` recipe now runs `cargo test --doc --all-features` as a second
  command, and `make test` reports `28 passed; 0 failed; 20 ignored` for the
  doctest section. The pre-existing doctest suite passed before the change, so
  widening the gate could not break unrelated work.

- Observation: `INV-IDEMPOTENT` **fails**. Two independent, pre-existing
  defects make `format(format(x)) != format(x)` for reachable inputs. This is
  the `Tolerances` stop condition, so `EP-M2` halted at its first property run
  and the finding is escalated rather than worked around.
  Evidence, class A — normalised thematic breaks stop being breaks. `--breaks`
  rewrites every thematic break to 70 underscores, but `wrap` recognises a
  break only through the table-separator pattern `^[\s|:-]+$`
  (`src/table.rs:117-120`, reached via `is_table_or_separator`,
  `src/wrap.rs:67-69`), which matches hyphens and colons only. A normalised
  break is therefore folded into the adjacent paragraph on the next pass.
  Minimal synthetic input: `alpha\n` + 70 underscores + `\nbeta\n` under
  `--wrap` becomes `alpha ____…____` on one line. Minimal original input:
  `---\nprose words here` under `--wrap --breaks` →
  `____…____\nprose words here\n` → `____…____ prose\nwords here\n`. The same
  predicate means `--wrap` alone already folds `***` and `___` breaks into
  paragraphs on the first pass. Existing fixture affected:
  `tests/data/cli-matrix/frontmatter-breaks.dat` (1 of 123 under the full flag
  set; 0 of 123 under `--wrap` alone).
  Evidence, class B — wrap tail reflow depends on the source line's trailing
  token. A list item whose first line ends with an inline code span re-wraps
  differently on the second pass. Minimal input (83 chars, `--wrap` alone,
  found by character-level delta debugging):
  ``- *Ownership.** Owned by the wrap module (`src/wrap/tracing_snapshot_support.rs`)\nn``
  → pass 1 breaks after `module`, pass 2 joins the code span to the following
  line. Affected in-repo: `docs/developers-guide.md` (1 of 28 Markdown files,
  under `--wrap` alone and under the full `mdformat-all` flag set).
  Both classes converge after two passes and neither can loop, but both are
  reachable with flags `make fmt` already uses: `mdformat-all` runs
  `--wrap --renumber --breaks --ellipsis --fences --in-place`.
  Pre-existence: this branch modifies only `src/lib.rs` and adds `src/report*`,
  so `src/wrap*`, `src/breaks.rs`, and `src/table.rs` are untouched; both
  defects are present at `HEAD` and in released versions.
  Impact: `--check` cannot be a one-pass gate for the affected classes. A
  single `--in-place` pass leaves the file still drifting, so check → fix →
  check does not go green without a second fix. `Tolerances` forbids narrowing
  the property as a workaround, so the decision is the user's: fix both
  defects on this branch, or halt.

- Observation: a unified diff does **not** spell out the whole right-hand side
  of the document. The first version of the `--diff` matrix invariant
  reconstructed it from the body's context and insertion lines, which holds only
  where every line falls inside some hunk's context.
  Evidence: `row_000_nowrap_diff` failed that comparison, short by the trailing
  blank line, `Title`, and `=====`. The `similar` payload is `@@ -1,4 +1,6 @@`
  for a seven-line file: the last change is on line 1, so the three lines of
  trailing context end at line 4 and lines 5-7 are absent from the payload
  entirely, exactly as GNU `diff` omits them. The invariant was replaced by a
  patch applier that copies the lines no hunk covers from the file, which is
  what any consumer of the diff must do; the case now passes without weakening
  the assertion, and unit tests pin the gap and tail cases directly. Measuring
  twice here was worth it: the naive version was one fixture away from being
  silently vacuous.
  Date/Author: 2026-09-11.

- Observation: `mdformat-all` is not `mdformat`. It runs this crate's own CLI
  over every Markdown-like file — `mdtablefix --wrap --renumber --breaks
  --ellipsis --fences --in-place` — and then `markdownlint-cli2 --fix`.
  Evidence: the wrapper script at `~/.local/bin/mdformat-all`; `mdformat` is not
  installed at all. Two consequences for documentation work. First, the scoped
  equivalent of `make fmt` for one file is now available as a check:
  `mdtablefix --wrap --renumber --breaks --ellipsis --fences --check <file>`.
  Second, the wrapper re-wraps a paragraph as a whole rather than line by line,
  so prose whose every line fits within 80 columns can still be re-flowed: the
  new `docs/developers-guide.md` prose was re-balanced across lines even though
  no line exceeded the limit. Those sections were therefore taken from that
  command's own output rather than hand-wrapped.
  Date/Author: 2026-09-11.

- Observation: the repository's Markdown does not satisfy its own formatter at
  `HEAD`. All three of `README.md`, `docs/users-guide.md`, and
  `docs/developers-guide.md` drift under the `mdformat-all` flag set.
  Evidence: `mdtablefix --wrap --renumber --breaks --ellipsis --fences --check
  <file>` exits `1` for each, with `+101 -103` for `docs/developers-guide.md` at
  `HEAD` before this milestone's edits. Consequence for `EP-M7`: running
  `make fmt` there will rewrite unrelated prose across the documentation set, so
  that step must be reviewed as its own change rather than folded into the
  content edit.
  Date/Author: 2026-09-11.

- Observation: a **third** non-idempotent transform class exists, and it is both
  pre-existing and reachable by the plan's own test suite. Under `--headings`
  alone, a table whose delimiter row is immediately followed by a `---` line is
  not a fixed point: the `---` makes the delimiter row a Setext heading
  candidate, so pass 1 rewrites `|---|---|` as `## | --- | --- |` and pads the
  body row to the delimiter row's cell widths, and pass 2 re-pads that body row
  against the new heading's narrower cells.
  Evidence: `printf '|1|2|\n|---|---|\n---'` → pass 1
  `| 1   | 2   |\n## | --- | --- |\n` → pass 2
  `| 1 | 2 |\n## | --- | --- |\n`; five of six corpus cases drift, and the
  sixth (`|A|B|\n|---|---|\nTitle\n---\n`) is the control in which a line of
  prose between the delimiter row and the `---` prevents the absorption. The
  pass-1 bytes are **byte-identical** to those of a binary built from
  `git archive origin/main` (v0.5.1), and this branch's `src/` diff against
  `origin/main` contains no transform code, so the class is pre-existing rather
  than introduced by the reporting work — the same finding as the `EP-M2`
  classes, and the same reason issue #468's closure was incomplete.
  Date/Author: 2026-09-11.

- Observation: `tests/check_properties.rs::generated_documents_reach_a_fixed_point`
  **samples the failing class too rarely to fail reliably**, so the `test` gate
  has been passing by luck. The generator's `markdown_lines()` can emit
  `"|---|---|"` and `"---"` but draws a document of at most twelve lines, so the
  three-line shape that triggers the class is one of a very large space of
  draws and `with_cases(48)` does not reliably reach it.
  Evidence: repeated runs in one unchanged tree gave 10 passes, then 3 passes
  followed by 27 failures, then 5 failures out of 5; another tree gave a pass
  followed by failures. With `PROPTEST_RNG_SEED=0` the failure is reproducible
  and always shrinks to `document = "|1|2|\n|---|---|\n---", mask = 128` (the
  `--headings` bit). `PROPTEST_RNG_SEED` is honoured by proptest 1.11.0, so the
  seed explains which draw is taken, but **why some earlier unseeded runs in the
  same tree passed is not fully explained**: stale-binary, feature-flag,
  target-directory, `RUSTFLAGS`, path-dependence, assert_cmd-resolution, and
  binary-nondeterminism explanations were each tested and each ruled out (30 of
  30 identical invocations at five different paths). The honest statement is that
  the property is flaky because the generator is sparse, which is exactly the
  vacuity hazard issue #468's acceptance criteria name; the residual variance is
  recorded as an open question rather than asserted away.
  Date/Author: 2026-09-11.

- Observation: resolving the positional arguments up front **changes what one
  unusable path does to a run**. Before, a path that is not valid UTF-8 was
  rejected per file by `open_file_parent`, so it was counted as one file's error
  while its siblings were still analysed and reported. Now the whole run fails
  before the parallel stage, prints no reports, and exits `2`.
  Evidence: `tests/cli_check.rs::a_non_utf8_path_argument_exits_error` runs
  `--check clean.md <invalid>` and asserts exit `2`, an empty standard output, and
  a message naming the problem; the source behaviour is
  `Inputs::resolve`'s single conversion, and
  `src/driver_tests.rs::resolve_declines_a_non_utf8_path` pins it at unit level.
  The change is deliberate: a path the tool cannot name cannot be a per-file
  report, because the report would have to name it. It is called out here because
  it is a user-visible difference that the `--git` request implied rather than
  stated.
  Date/Author: 2026-09-11.

- Observation: making the clean-file write a no-op also **removes the symlink
  refusal for clean targets**. `--in-place link.md`, where `link.md` is a symlink
  to a file that already matches the formatter's output, used to fail with
  `refusing to replace the symlink …` and now succeeds without touching the link
  or its target.
  Evidence: measured on the built binary — a symlink to `| A | B |\n| 1 | 2 |\n`
  exits `0`, and the link, its target, and both inodes and modification times are
  unchanged. A symlink to a *drifting* file still fails, which
  `tests/in_place_atomic.rs::in_place_declines_symlinked_target` pins beside the
  new `in_place_accepts_a_symlink_to_a_clean_file`; `src/io_tests.rs` and
  `src/io_tracing_tests.rs` pin the refusal at the `replace_file` level and are
  untouched, because the refusal is a property of the replacement and there is
  now no replacement to make. Declared to the `--git` plan rather than left for
  a reviewer to find.
  Date/Author: 2026-09-11.

- Observation: **a rebase can report success and still be wrong.** The first
  attempt at the second rebase exited clean, with no conflict, and silently
  corrupted three Markdown documents: `docs/architecture.md`'s footnotes example
  had its `Before:` lines rewritten to the `After:` form and lost the `After:`
  label, its blank line, the opening fence, and the `Text.` line under it, and
  fourteen blank lines appeared from nowhere across `docs/architecture.md` (one),
  `docs/developers-guide.md` (five), and `docs/users-guide.md` (eight). The only
  signal was a stream of `weave: N entities auto-resolved (<confidence>)`
  notices, at `high`, `very_high`, and once `conflict` confidence — a merge
  driver selected by the global attributes file for Markdown paths, reporting
  confidence in an entity merge that had changed the meaning of a worked
  example.
  Evidence: the three blobs compared three ways (`origin/main:<path>`,
  `2a73a58:<path>`, working file) — the corrupt copies, the rebase log, and both
  diagnostic diffs are preserved under `/tmp/weave-corrupt/`; the recovery run
  under `git -c core.attributesFile=/dev/null` produced a tree whose delta
  against `2a73a58` is exactly `408c76a`'s own stat. Recorded in
  `Artefacts and notes → Second rebase onto origin/main (#477)`.
  Lesson: for this repository a clean driver exit is not evidence of intended
  semantics, so the default is Git's built-in merge and a byte-level comparison
  against both parents is what makes a driver's result acceptable. The
  deterministic gates would have caught the damage in the end, and not only the
  blank lines: the preserved copies fail `markdownlint` with 21 errors under the
  repository's own configuration — 14 `MD012` from the inserted blanks, three
  `MD053` on the `[^1]`, `[^2]`, and `[^10]` definitions the entity merge
  invented, two `MD051`, and `MD031`/`MD040` on the fence it dropped — while the
  same three files as merged report 0, which is what makes the corruption the
  cause rather than a pre-existing finding. What the blob comparison bought was
  finding it before the corrupted replay was committed and pushed, which the
  gate run alone would not have done.
  Date/Author: 2026-09-11.

## Decision log

- Decision: `--check` reports concisely as `<path> +<ins> -<del>` and `--diff`
  is a separate unified-diff mode; the `--concise` flag proposed in GitHub
  issue #452 is not implemented.
  Rationale: issue #452 proposed `--check` emitting diffs with `--concise`
  reducing it to filenames. The requested design inverts this, so `--check` is
  already the concise form and a third flag would be redundant. Confirmed by
  `@leynos`.
  Date/Author: 2026-09-09.

- Decision: `--diff` exits `1` on drift, exactly as `--check` does. The two
  remain mutually exclusive.
  Rationale: the original requirement had `--diff` exit `0` regardless of
  drift. Design review established that `ruff format --diff`, `rustfmt
  --check`, `dprint check` and modern `gofmt -d` all exit non-zero when a diff
  exists, and `@leynos` adopted that behaviour on the principle of least
  surprise. `ruff format --diff` is documented as "exit with a non-zero status
  code **and** the difference between the current file and how the formatted
  file would look"; `gofmt -d` was changed to exit non-zero in golang/go#46289
  for the same reason.
  This also dissolves the second half of the original concern. The reason
  Black, ruff, and `terraform fmt` permit combining check with diff is so that
  one run can both show the drift and fail the build. Now that `--diff` does
  both by itself, combining the flags would be redundant, so `--check` and
  `--diff` stay mutually exclusive and a CI job needs only one invocation,
  reading every file once. That preserves the `Quality criteria` requirement
  that no file is read more than once, which the earlier design would have
  violated for any job wanting both outputs.
  `--check` and `--diff` are therefore two renderings of one analysis with
  identical exit semantics: `--check` is the concise rendering for logs,
  `--diff` the verbose rendering for diagnosis.
  Date/Author: 2026-09-09, planning agent; revised 2026-09-09 on explicit
  instruction from `@leynos` after design review.

- Decision: exit `0` clean, `1` drift under `--check` or `--diff`, `2`
  operational error, computed as `exit_status(mode, any_drift, any_error)`.
  `fn main` returns `std::process::ExitCode`. Bump the crate to `0.6.0`.
  Rationale: a gate must distinguish "needs formatting" from "could not run";
  today both exit `1`. Making the status a function of mode is what prevents
  `--in-place` and a bare invocation from exiting `1` on a successful run. An
  operational error dominates drift, because an incomplete analysis must not
  be reported as a merely drifted result. Confirmed by `@leynos`; the version
  bump was added after design review as the migration signal, and `--diff` was
  brought under the drift status in the same revision.
  Date/Author: 2026-09-09.

- Decision: `--check` reports drift if and only if the formatted bytes differ
  from the file's current bytes.
  Rationale: the value of `--check` is that it predicts `--in-place`. Any
  comparison ignoring differences `--in-place` would write produces a gate
  that passes while the tree is dirty. Confirmed by `@leynos`.
  Date/Author: 2026-09-09.

- Decision: implement line-ending preservation (issue #451) and byte-order-mark
  preservation as `EP-M1`, before any reporting feature.
  Rationale: with byte-exact comparison, unconditional LF normalization makes
  `--check` report whole-file drift on every CRLF file, and a retained
  byte-order mark makes it report clean on a ragged file. The first is useless,
  the second is dangerous. Line-ending preservation was confirmed by `@leynos`;
  byte-order-mark handling was discovered during design review and is included
  because without it the feature's headline guarantee is false.
  Date/Author: 2026-09-09.

- Decision: the report line is `<path> +<insertions> -<deletions>` with plain
  decimal counts.
  Rationale: chosen by `@leynos`. Recorded caveat from design review: this
  matches no existing formatter, and `git diff --numstat` is in fact
  `insertions<TAB>deletions<TAB>path` with git-style quoting for awkward paths.
  Consumers must therefore parse by taking the final two whitespace-separated
  fields as the counts and everything before them as the path. Paths
  containing a newline cannot be represented; such paths are rejected with an
  operational error rather than emitting an unparseable line.
  Date/Author: 2026-09-09.

- Decision: a changed line counts as one insertion plus one deletion, matching
  `git diff --numstat`.
  Date/Author: 2026-09-09, planning agent.

- Decision: report lines go to standard output; the summary line and all
  diagnostics go to standard error.
  Rationale: standard output becomes a pure machine contract that needs no
  filtering, which is the point of the concise format. Black does the same.
  Added after design review, which found the draft mixed a prose sentence into
  the machine stream.
  Date/Author: 2026-09-09.

- Decision: the summary states changed, unchanged, and errored counts, with
  clauses elided at zero and correct singular and plural forms. Exact grammar
  is specified in `Interfaces and dependencies` and snapshot-tested.
  Rationale: the draft showed three mutually inconsistent summary strings for
  the same class of run, which by the plan's own ambiguity tolerance should
  have stopped work. An errored count is required so a user can tell that
  three of five files were analysed.
  Date/Author: 2026-09-09, added after design review.

- Decision: use `similar = "2.7"` for counting and unified-diff rendering.
  Rationale: correct minimal line diffing and conformant unified-diff
  rendering are solved problems with real subtleties. `similar 2.7.0` is
  already in `Cargo.lock` via `insta`, is Apache-2.0 (permissive, compatible
  with distributing an ISC-licensed binary), and has no mandatory
  dependencies. Requesting `"2.7"` rather than `"3"` keeps one copy in the
  graph.
  Date/Author: 2026-09-09. Requires approval at the gate.

- Decision: do not introduce a `DocumentStore` trait. Instead introduce a
  `ReadOnlyDir` newtype wrapping `cap_std::fs_utf8::Dir` and exposing only
  reads; `--check` and `--diff` receive that type, `--in-place` receives the
  `Dir`.
  Rationale: the draft's trait had exactly one production adapter and one test
  adapter with no second backend in prospect, which `AGENTS.md`'s abstraction
  policy would classify as speculative generality. Worse, it did not deliver
  its stated benefit: the draft passed the store into `apply`, so every mode
  had a `write` method in scope and the read-only guarantee still rested on a
  panicking test double. A newtype with no write method makes the guarantee
  hold by construction, adds no public library surface, and matches
  `AGENTS.md:217-231`'s explicit preference for newtypes over ad hoc
  abstraction. Reversed after design review.
  Date/Author: 2026-09-09.

- Decision: the driver lives in the **binary** as `src/driver.rs`, not in the
  library. The library gains only the pure modules `document` and `report`.
  Rationale: the draft placed it in the library, arguing that was the only
  practical way to unit-test mode dispatch. That was factually wrong: binary
  crates take unit tests, and `src/main.rs:227-301` already runs `proptest`
  inside `#[cfg(test)] mod tests`. Library placement would have made `Mode`,
  the exit-status mapping, and a filesystem port into permanent semver surface
  of a Markdown-transform library, and would have forced `anyhow` into public
  library API in direct violation of `AGENTS.md:266-270`. `src/io.rs` is the
  cautionary precedent: public library API the binary never calls. Keeping the
  driver in the binary leaves the library infallible and error-type-free.
  Reversed after design review.
  Date/Author: 2026-09-09.

- Decision: the parallel stage returns a small `FileReport` plus a rendered
  `String`, dropping each `Assessment` inside its closure; ordering is by
  explicit argument index.
  Rationale: the draft's `apply(..., out: &mut impl Write)` could not be
  called from a `par_iter` at all, and retaining every `Assessment` until
  printing meant peak memory of roughly twice the total input. Returning a
  value type also makes a future `--format=json` a leaf addition rather than a
  re-plumb. Added after design review.
  Date/Author: 2026-09-09.

- Decision: defer atomic write-then-rename for `--in-place` to GitHub issue
  #465 rather than including it here.
  Rationale: design review identified that `--in-place` truncates before
  writing, so a kill or a full disk leaves files empty, and that this plan
  already rewrites the seam where the fix belongs. `@leynos` directed that it
  be raised separately, keeping this plan scoped to the reporting modes and
  the two document-boundary fixes that `--check` correctness depends upon.
  Sequence #465 immediately after this work so the serialization path is
  edited once.
  Date/Author: 2026-09-09, on explicit instruction from `@leynos`.

- Decision: accept whole-file majority line-ending detection, including its
  effect on fenced code blocks.
  Rationale: design review noted that a mostly-CRLF document containing an
  LF-authored snippet inside a fence has that snippet rewritten to CRLF, which
  is a content change rather than a formatting one. Issue #451 specifies
  majority detection, and `@leynos` accepted the consequence. The obligation
  `INV-DOCUMENT` therefore carries a mixed-endings-inside-a-fence case so the
  behaviour is pinned rather than incidental, and `EP-M7` documents it
  explicitly in the user's guide.
  Date/Author: 2026-09-09, on explicit instruction from `@leynos`.

- Decision: accept the five proposed dependencies.
  Rationale: `similar` at runtime, and `rstest-bdd`, `rstest-bdd-macros`,
  `googletest`, and `pretty_assertions` for tests. Approved by `@leynos`. The
  design review's cost objection to the assertion libraries is recorded above
  and is mitigated by scope rather than removal.
  Date/Author: 2026-09-09, on explicit instruction from `@leynos`.

- Decision: `--check`, `--diff`, and `--in-place` all require file arguments.
  Rationale: consistency with `--in-place`'s existing `requires = "files"`,
  which `tests/cli.rs:28-35` asserts. Recorded hazard: under a shell without
  `nullglob`, `mdtablefix --check docs/*.md` matching nothing becomes a usage
  error exiting `2`, failing a job that should pass, whereas Black and ruff
  exit `0` on an empty file set. Documented in the user's guide rather than
  changing tested behaviour. Raised at the gate as a reversible choice.
  Date/Author: 2026-09-09.

- Decision: when no line-ending style holds a strict majority, or the file has
  none, emit LF.
  Rationale: issue #451 requires deterministic documented behaviour in the
  no-majority case; LF preserves today's behaviour and is the ecosystem
  default. Counting must count `\r\n` occurrences and subtract them from the
  total `\n` count to obtain lone LFs; counting `\n` naively double-counts
  every CRLF and makes CRLF unable to win.
  Date/Author: 2026-09-09.

- Decision: unified-diff headers use the display path on both sides with
  directory separators normalized to `/`, no timestamps, context radius three,
  and the missing-newline marker retained.
  Rationale: timestamps would make output non-deterministic and
  unsnapshot-able. Backslash separators would not resolve on other platforms.
  Recorded limitation: without a tab delimiter, GNU `patch` truncates a path at
  the first space, so `--diff` output is for human review and is not
  guaranteed to apply cleanly for paths containing whitespace. The draft's
  acceptance criterion requiring `patch` to reproduce the file is therefore
  dropped, which also removes an undeclared external tool dependency from the
  test suite.
  Date/Author: 2026-09-09, amended after design review.

- Decision: never colourize output, under any terminal or environment
  setting.
  Rationale: determinism is a hard constraint and the snapshot tests run
  without a terminal, so a colour regression would be invisible to them.
  Added after design review, which found the draft silent on this.
  Date/Author: 2026-09-09.

- Decision: cut the Verus proof milestone entirely rather than making it
  optional.
  Rationale: the draft rejected Kani on the ground that a bounded model check
  over a synthetic tag sequence would verify a re-implementation rather than
  the real diff engine, then proposed Verus over the identical synthetic
  domain. That is special pleading. More decisively, the proposed goal reduces
  to `|E| + |I| = |E| + |D| + |I| - |D|`, an arithmetic identity, not a lemma;
  all the content lives in `AX-2`, which is `similar`'s documented behaviour
  and is axiomatized precisely because it is third-party. `AGENTS.md` requires
  a proof to be substantive and not a restatement of an assumed property, so
  cutting this is compliance, not evasion. `rust-toolchain.toml` also pins
  `nightly-2026-03-26` while Verus ships its own pinned toolchain and build
  system, so the go/no-go condition was almost certainly unsatisfiable.
  Replaced by `EP-M6`, targeted mutation testing, which attacks the actual
  stated risk — that the counting mis-attributes a tag — empirically.
  Date/Author: 2026-09-09, after design review.

- Decision: keep `googletest`, `pretty_assertions`, and `rstest-bdd` despite
  the review's objection.
  Rationale: all three were named as authorized and requested. The review's
  case is recorded honestly: they take the repository from two assertion
  idioms to four, and the behavioural scenarios here map one-to-one onto plain
  `rstest` plus `assert_cmd` cases. The mitigation is scope rather than
  removal: `rstest-bdd` owns the readable end-to-end specification,
  `tests/cli_check.rs` owns only the mechanical cases that read badly as
  Gherkin, and `EP-M5` is narrowed, so the feature has two test surfaces
  rather than three.
  Date/Author: 2026-09-09.

- Decision: vendor `docs/rstest-bdd-users-guide.md` and
  `docs/reliable-testing-in-rust-via-dependency-injection.md` with a
  provenance header naming the source repository and commit.
  Rationale: both are signposted by the task but absent here, and this plan is
  the first to introduce `rstest-bdd` and injected boundaries to this
  repository. A bare copy would drift silently; a provenance header makes the
  fork visible and re-syncable. `docs/netsuke-design.md` and
  `docs/ortho-config-users-guide.md` are also absent but not applicable: this
  repository parses arguments with plain `clap`, not `ortho-config`, and
  adopting the Netsuke lint baseline is tracked separately as issue #441.
  Date/Author: 2026-09-09.

- Decision: raise the two idempotence defects as GitHub issue #468, with the
  reproduction corpus attached and an acceptance bar of corpus **and**
  property test, and leave `EP-M2` green work suspended until the scope
  decision is made.
  Rationale: the defects are pre-existing in `--wrap` and `--breaks`, outside
  the reporting feature this plan was scoped to build, so fixing them here
  would silently widen the change under review; leaving them undocumented
  would let `--check` ship against a guarantee it cannot meet. An issue with a
  failing corpus is the artefact that keeps the finding actionable without
  binding this plan to a fix whose blast radius is not yet agreed. Raised on
  explicit instruction from `@leynos`, who set the two-part acceptance bar.
  Date/Author: 2026-09-09, on explicit instruction from `@leynos`.

- Decision: resolve the rebase onto `origin/main` by taking pull request #469's
  line-ending implementation and re-landing only the byte-order-mark half of
  `EP-M1`.
  Rationale: #469 (issue #451) had already landed `src/io/line_endings.rs` with
  equivalent behaviour, so keeping both would leave two competing definitions of
  the same policy in one crate. But a `git grep` over `main` showed no
  byte-order-mark handling anywhere: a marked file would reach the transforms
  with `U+FEFF` fused to its first line, defeating them and making `--check`
  report a ragged file clean. That is a silent false negative in the
  user-facing guarantee, so the mark still had to land.
  Date/Author: 2026-09-11.

- Decision: move the document boundary from the planned `src/document.rs` to
  `src/io/document.rs`, and take `LineEnding`, `LineEndingCounts`,
  `count_line_endings`, and `serialize_lines` from `src/io/line_endings.rs`
  instead of redefining them.
  Rationale: with #469 merged, a top-level `src/document` and an `src/io` that
  both needed the line-ending policy would form a cyclic module dependency, and
  a second public `LineEnding` beside the crate-root re-export would be two
  types with one name in the published surface. `SourceDocument` keeps its shape
  and its type-level guarantee.
  Date/Author: 2026-09-11.

- Decision: write the check-and-diff ADR as `0009` and give the byte-order mark
  its own `0008`, rather than reusing the `0006` and `0007` numbers this plan
  had reserved.
  Rationale: #470 and #469 had already written `0006-single-pass-idempotence.md`
  and `0007-line-ending-detection.md`. An ADR number is a stable reference, so
  renumbering merged records to reclaim the reserved slots would break every
  citation of them; the new records take the next free numbers instead. `0007`
  covers line endings only and does not mention the mark, so the mark still
  needs a record of its own.
  Date/Author: 2026-09-11.

- Decision: treat the atomic `--in-place` write as discharged by pull request
  #467 (issue #465) rather than implementing it here.
  Rationale: this plan had deliberately deferred write-then-rename to issue #465
  and recorded it as a high-severity risk. #467 landed it as `replace_file`,
  which is what `src/main.rs` and this branch's document-boundary work now call,
  so `EP-M3` inherits the guarantee instead of restating it.
  Date/Author: 2026-09-11.

- Decision: keep the idempotence cases in `tests/check_properties.rs` but scope
  them to the document boundary, leaving the general suites to
  `tests/idempotence.rs` and `tests/idempotence_properties.rs`.
  Rationale: #470 landed those two suites for document *structure*, with a
  stronger generator than this plan's, so carrying an unscoped copy here would
  duplicate both the runtime and the maintenance. The *boundary* dimension —
  ending style, byte-order mark, and trailing terminator, under every singleton
  flag — is not sampled by those suites and is exactly what this plan changed,
  so dropping it entirely would lose real coverage of the new code.
  Date/Author: 2026-09-11.

- Decision: sample *related* text pairs in
  `count_conserves_tokens_and_agrees_with_byte_equality`, in addition to
  independent random pairs.
  Rationale: applying the `LEM-COUNT` negative control showed the property test
  caught nothing, because two independent `any::<String>()` draws never share a
  line and both assertions then hold for any diff at all. The property test now
  fails under all three controls. This is the plan's own argument that the
  conservation law is not falsifiable by itself, applied to the generator rather
  than only to the golden fixtures.
  Date/Author: 2026-09-11.

- Decision: the three gate failures reported by the first `EP-M2` gate run were
  fixed rather than waived: four `cast_possible_wrap` errors in
  `tests/check_properties.rs`, nine rustfmt diffs, and three markdownlint
  errors in this plan.
  Rationale: green tests are not a gate. `make check-fmt` had not been run over
  `src/io/document.rs`, which an earlier segment of this session committed, so
  its `SourceDocument::parse` doc example was still a single over-long line;
  the rest of the rustfmt diffs were in this milestone's own new code. The
  `MD029` errors were structural rather than cosmetic:
  `.markdownlint-cli2.jsonc` sets `MD029` to `ordered`, and the
  negative-control transcripts' column-zero fences ended the numbered list, so
  the items numbered `2.` and `3.` each started a fresh list that has to begin
  at `1.`. Those items are now dash-bulleted `**Control N.**` entries, which
  keeps the numbering visible and removes the ordered list entirely. `MD018`
  was one wrapped paragraph line beginning `#469`. The `MD029` and `MD018`
  fixes were verified by re-running markdownlint over the plan alone, and the
  cast errors by `cast_signed()`, which is what clippy itself suggests.
  Date/Author: 2026-09-11.

- Decision: the reporting subset is the three base rows `row_000`, `row_010`,
  and `row_111`, not a spread across the matrix.
  Rationale: the rows were chosen by measurement rather than by taste. Running
  every row under both wrap variants and both reporting modes showed that only
  `row_010` **unwrapped** is already a fixed point, so that row is the only
  source of the no-drift branch (`--check` and `--diff` silent, exit `0`);
  `row_000` is the plain table case a user meets first, and `row_111` carries
  the frontmatter boundary, which is where the report line and the diff can
  disagree about the document's extent. Twelve snapshots are added, well inside
  the milestone's raised churn limit, and no existing snapshot changes.
  Date/Author: 2026-09-11.

- Decision: the matrix stages every fixture as `input.dat` and runs the binary
  with the temporary directory as its working directory.
  Rationale: both reporting modes name the file they report, so a staged path
  the harness invented (a `tempdir()` path) would be written into a snapshot and
  make it machine-specific. The relative name is stable, and the existing
  `.dat` fixture self-test now carries the second reason for its rule. The cost
  is that a case cannot distinguish two same-named inputs, which no matrix row
  needs.
  Date/Author: 2026-09-11.

- Decision: the `--diff` invariant applies the payload to the file rather than
  reconstructing the printed document from the body's marker lines.
  Rationale: the payload is a patch, so applying it is the semantics a consumer
  relies on, and it is the only version that holds once a change is far enough
  from the end of the file (see `Surprises & discoveries`). The applier asserts
  each context and deleted line against the file as it goes, so a diff of the
  wrong text fails loudly rather than reconstructing something merely different.
  Date/Author: 2026-09-11.

- Decision: `RunResult::envelope` keeps emitting the `[file]` block for the
  read-only modes, marked `<not applicable>`, rather than omitting it.
  Rationale: omitting the block would change all 32 pre-existing matrix
  snapshots for a purely cosmetic gain. Keeping the block means the milestone
  adds twelve snapshots and rewrites none, so the reviewer reads exactly the new
  evidence. The block also records the mode's defining property — a reporting
  run does not write — which the harness then asserts against the fixture bytes.
  Date/Author: 2026-09-11.

- Decision: `tests/cli_matrix/support.rs` is left above 400 lines rather than
  split again to satisfy the `AGENTS.md` file-length rule.
  Rationale: the rule is written for production modules, and this repository
  already carries test files over the limit (`tests/idempotence_properties.rs`
  537, `tests/fences.rs` 493). Splitting the harness further would add a module
  boundary that exists only to move lines, while the reporting helpers already
  live in their own `tests/cli_matrix/reporting.rs` because they are a distinct
  concern rather than because of the cap.
  Date/Author: 2026-09-11.
  **Superseded 2026-09-12** by the review round recorded in `Revision 22`. The
  rule is enforced on every file, test or not: the repository's other over-limit
  test files are pre-existing debt this plan does not add to, and the review
  asked for the split. The catalogue now lives in `tests/cli_matrix/cases.rs`
  and the harness's unit tests in `tests/cli_matrix/support_tests.rs`, leaving
  `support.rs` at 380 lines.

- Decision: `EP-M6` is halted before its first command, and the mutation run is
  deferred until the `test` gate is green again.
  Rationale: `cargo-mutants` refuses to test any mutant when the unmutated
  baseline `cargo test` fails (`ERROR cargo test failed in an unmutated tree, so
  no mutants were tested`), and the baseline is red on `a06bab6` because
  `generated_documents_reach_a_fixed_point` has a genuine counterexample. The
  options were to fix the transform, to exclude the flaky test from the baseline,
  or to defer. Excluding it was rejected outright: it would report a mutation
  score for a tree whose own gate is red, which is the "hide the failure"
  outcome `Tolerances` forbids. Fixing the transform is the correct outcome but
  is a change to `src/headings.rs` (or its call site) that this plan's
  `Constraints` place out of scope, and the established precedent for exactly
  this situation — the `EP-M2` classes, which became issue #468 and pull request
  #470 — is a separate issue and a separate change. So the milestone is deferred,
  the gap is recorded rather than skipped, and the plan does not claim a mutation
  result it does not have.
  Date/Author: 2026-09-11.

- Decision: the new defect class is escalated as GitHub issue #474, with a
  reproduction corpus and a non-vacuity requirement, in the shape issue #468
  used, rather than folded into this plan's remaining milestones.
  Rationale: the user's instruction for #468 fixed the acceptance bar —
  "the issue cannot be considered fixed until both the reproduction corpus and a
  substantive property check demonstrate idempotency" — and this class passes
  neither half today: the corpus does not carry the shape, and the property
  check that would have caught it is the flaky one. Raising it separately keeps
  the reporting feature's own acceptance criteria honest, because a green `test`
  gate reached by narrowing the property would close the feature while leaving
  the defect live.
  Date/Author: 2026-09-11.

- Decision: work does not proceed past the halt into `EP-M7` as though the tree
  were healthy, and no further commit claims all gates green.
  Rationale: `make test` is red at `HEAD` and the failure is deterministic under
  a fixed seed. `EP-M7` is documentation-only, so its own diff can still be gated
  by `markdownlint` and `nixie`, but the `Gates:` line of any commit that touches
  code or tests must not assert a passing test suite while this counterexample
  stands. Recording the true state is the whole point of the `Progress` section.
  Date/Author: 2026-09-11.

- Decision: the `mode` group requires a new `inputs` group that holds `files`,
  so the input source is a named thing rather than a flag-to-positional
  dependency.
  Rationale: `requires("files")` states the same runtime behaviour, but it binds
  every mode flag to one particular source. The requesting plan's author measured
  both shapes on `clap` 4.6.6 and found them behaviourally identical (`[]` and
  `["--in-place"]` refused; `["a.md", "b.md"]` and `["--in-place", "a.md"]`
  accepted; `["--check", "--diff", …]` refused), and
  `tests/cli_check.rs::mode_flags_require_an_input_source` pins that matrix from
  this branch's side, including the two-file cell that a group with
  `multiple(false)` could plausibly have broken.
  Date/Author: 2026-09-11, requested by the `--git` plan's author.

- Decision: `main` no longer chooses between standard input and files by testing
  whether the file list is empty. `driver::Inputs::resolve` returns
  `Inputs::Stdin` or `Inputs::Files(Vec<Utf8PathBuf>)`, and `run` matches on it.
  Rationale: emptiness-as-sentinel conflates "no paths were named, so read
  standard input" with "the source matched no paths". A source that discovers its
  own inputs must be able to match nothing and exit `0`, rather than block on a
  terminal's standard input. The conversion to `Utf8PathBuf` happens once in the
  same place, so the parallel stage no longer carries a path that cannot name a
  file in a `Dir` capability. See `AX-6`.
  Date/Author: 2026-09-11, requested by the `--git` plan's author.

- Decision: input-resolution failures return through `exit_status(mode, false,
  true)` rather than propagating out of `run`.
  Rationale: the documented contract is that an operational error exits `2`, and
  a pre-flight failure — a path that cannot be named, and later a missing `git`
  or a directory that is not a repository — is an operational error like any
  other. Folding it into `exit_status` keeps one place that decides the status,
  and `tests/cli_check.rs::a_non_utf8_path_argument_exits_error` pins exit `2`
  for the one such failure that exists today.
  Date/Author: 2026-09-11, requested by the `--git` plan's author.

- Decision: `--in-place` does not write a file whose bytes would not change.
  Rationale: the write is invisible in the text but not in the file. `--check`
  reports drift only when the bytes differ, so a clean file is by definition one
  `--in-place` has nothing to do to; an unconditional `replace_file` still
  renames a temporary over the target, swapping the inode and the modification
  time, so a staleness check downstream sees a rebuild where there was nothing
  to rebuild. `tests/in_place_atomic.rs` and `src/driver_tests.rs` now pin the
  inode and the modification time of a clean target, each beside a pair that
  proves a drifting file *is* replaced, so neither test can pass by never
  writing at all. Behaviour change accepted and declared to the `--git` plan: a
  symlink to a *clean* file is no longer declined, because a declined
  replacement is a property of a replacement that no longer happens.
  Date/Author: 2026-09-11. Offered as optional by the `--git` plan's author and
  accepted here.

- Decision: the `--git` plan's remaining requests need no change here, and none
  was made. `ReadOnlyDir`, `Assessment`, `Mode`, `exit_status`, and
  `in_argument_order` stay named items, which is what `--git --list-files` needs
  to take a `ReadOnlyDir` and be a `Mode` variant; no file discovery was added,
  because directory walking, globbing, and `git ls-files` are that plan's scope
  and not this one's.
  Rationale: it also closes the `clap` trap the requesting plan described. That
  trap — `requires` from one boolean flag to another being unreliable once a
  positional is present — needs a flag-to-flag relationship, and this CLI has
  none: every requirement here is flag-to-group, and the acceptance and rejection
  cells are measured by `tests/cli_check.rs::mode_flags_require_an_input_source`
  rather than reasoned about. The `AX-4` citation in the ADR stays, as the
  requesting plan asked: `rayon`'s `collect` still does not document order
  preservation.
  Date/Author: 2026-09-11, requested by the `--git` plan's author.

## Outcomes & retrospective

**This plan is `COMPLETE`.** `EP-M0`–`EP-M7` are discharged. `EP-M6` was
blocked by issue #474, a pre-existing non-idempotent transform class under
`--headings`, and that blocker cleared without a change from this branch: pull
request #477 fixed the transform, the branch rebased onto it, and every gate is
green again (`make test` at 1860 passed, 0 failed). The milestone then ran, in
Revision 18, on the rebased tip: 46 mutants, 39 caught, 6 unviable, 1 missed, no
timeouts — the miss recorded as an equivalent mutation whose premise is now a
test, the other initial survivor having been killed by a new assertion and the
score then re-measured in a single complete run over the final tree. No
obligation is outstanding,
so `Surprises & discoveries` and
`Artefacts and notes → EP-M6 baseline blocked` are the record of a defect class
this plan found and an upstream fix closed, not live issues, exactly as the
previous revision of this paragraph asked the reader to take them.

What was delivered, against the obligations:

- `INV-PREDICTS`: **the obligation this retrospective first claimed for the
  wrong artefact.** A draft of this bullet said that
  `tests/check_properties.rs` "runs both paths over two real copies and
  compares bytes". It does not: that file drives the formatter in print mode
  and tests the delta and idempotence claims, and no test anywhere ran
  `--check` against `--in-place`. The obligation is discharged by
  `tests/check_prediction.rs`, written afterwards and after the claim, which is
  the order the plan's own method forbids. The test could not be the obvious
  two-run comparison either: both modes consult one shared change decision, so
  a decision that answered wrongly would move both sides together and the
  agreement would hold vacuously. Each case therefore runs the document a
  third time in print mode, which renders the shared formatter's output without
  consulting that decision, and measures both the writer's bytes and the
  report's answer against it. The lesson is the one the plan already states and
  this bullet is the counter-example to: a claim about evidence is itself a
  claim, and it needs the same treatment as the code.
- `LEM-COUNT`, `INV-AGREE`: discharged in `tests/check_properties.rs`, and the
  same draft sentence was wrong about these too — the file makes in-process
  assertions on `LineDelta` rather than running modes over copies. The
  conservation law is stated over generated text pairs, and the golden
  `tests/data/numstat/` fixtures are what give it bite, because the law alone
  is satisfied by returning whole-file line counts. `INV-AGREE`'s
  line-ending-only and terminator-only pairs are folded into the same property,
  so "zero exactly when the bytes are equal" is tested on the two cases a naive
  implementation gets wrong. Corrected here rather than left standing, because
  a claim about evidence is itself a claim.
- `INV-IDEMPOTENT`: **the obligation that mattered most, and the one that is
  only partly discharged.** It was stated before any CLI surface existed, and
  it found two pre-existing defect classes immediately (issue #468, fixed by
  pull request #470) and a third later (issue #474, open when this was written
  and since fixed by pull request #477). The lesson is that the obligation was
  worth writing before the code; the residual gap is that its evidence is a
  property test over a sampled domain, so a green run is evidence rather than
  proof. The fix closed the known hole in that domain rather than widening it:
  the sampled shape is now a fixed point, and it is pinned unconditionally by
  fixtures as well, so the invariant no longer depends on the generator
  reaching it.
- `INV-NOWRITE`: discharged as the type-level `ReadOnlyDir` argument plus
  `tests/cli_check.rs`'s directory snapshot, with the same snapshot assertion
  run against `--in-place` as its non-vacuity control.
- `INV-BOM`: discharged end to end, the ragged and already-formatted
  byte-order-marked pair, in `tests/cli_check.rs`.
- `INV-DOCUMENT`: discharged by the `src/io/document.rs` unit tests and
  `tests/document_properties.rs`, including the fenced-code homogenisation case
  and both recorded negative controls.
- `INV-ORDER`, `INV-EXIT`, `INV-SUMMARY`, `INV-DETERMINISTIC`,
  `INV-FRONTMATTER`: discharged in the driver's unit test modules,
  `tests/cli_check.rs`, and the BDD scenarios. `INV-DETERMINISTIC`'s negative
  control exposed a
  blind spot in the method itself — the hazard sits on a transition band that
  a single fixed input cannot straddle — and the finding is recorded rather
  than papered over.
- `AX-4` was correct and is now load-bearing in two places: the design does not
  rely on `rayon`'s collection order, and `ADR 0009` cites the reasoning. The
  `--git` plan (pull request #466) asked for the citation to stay.

Discoveries that changed the plan rather than being absorbed by it:

- The document boundary is two features, not one. Pull request #469 landed the
  line-ending half on `main` while this plan was in flight, so the rebase took
  `main`'s implementation and re-landed only the byte-order-mark half, as
  `src/io/document.rs` with `ADR 0008`. The reserved ADR numbers `0006` and
  `0007` were taken by merged pull requests before `EP-M7` was reached; nothing
  was renumbered.
- `--in-place` writing only changed files is a behaviour change, not an
  optimisation: the replacement renames over the target, so a clean file's
  inode and modification time would move. It makes a symlink to a clean file
  succeed where a symlink to a drifting one is still declined.
- A non-UTF-8 path argument now fails the run as a whole (exit `2`) instead of
  counting as one file's error, because input resolution moved ahead of the
  parallel stage for the `--git` plan's benefit.
- `make fmt` is repository-wide and cannot be scoped, and it runs this project's
  own binary; 10 of the 31 tracked Markdown files already drift under its flag
  set. Its measurement caught a regression this plan introduced in
  `docs/contents.md`. See
  `Artefacts and notes → make fmt measured, and declined`.

The method that worked, recorded for whoever continues: write the falsifiable
obligation before the code, run the negative control as a real mutation, and
measure the tooling instead of assuming it — `cargo mutants`' baseline refusal,
`mdformat-all`'s scope, and the property test's own flakiness were each found by
running the thing rather than by reading about it.

## Context and orientation

`mdtablefix` is a Rust command-line tool that repairs and reflows Markdown
tables and optionally applies further transforms such as paragraph wrapping
and footnote conversion. It is one crate plus a small `test-macros` helper
crate, at version `0.5.1`.

Five existing pieces matter here.

**The transform pipeline.** `src/process.rs` exposes pure `&[String] ->
Vec<String>` functions that perform no input or output: `process_stream_inner`
at `src/process.rs:95` and `process_with_frontmatter` at `src/process.rs:275`,
the canonical boundary that splits leading YAML frontmatter, applies a body
function, and rejoins. `Options` at `src/process.rs:42` is a `Copy` struct of
six booleans. Note that the CLI's `FormatOpts` has **eight** flags: `renumber`
and `breaks` are applied by `src/main.rs:84-98` *outside* `process_stream_inner`.
Anything reasoning about "all transform options" must cover the eight, not the
six.

**The binary.** `src/main.rs` (302 lines) defines the `clap` `Cli` with an
`in_place` flag, a flattened `FormatOpts`, and `Vec<PathBuf>` files.
`open_file_parent` at `src/main.rs:100` is documented as "the only ambient
filesystem boundary for CLI file processing": it opens the file's **parent
directory** as a `cap_std::fs_utf8::Dir` capability and returns that plus the
**bare file name**. A capability here is a handle granting access to one
directory and nothing outside it, as opposed to ambient access where any path
may be opened. `format_to_string` at `src/main.rs:122` reads through the
capability and returns formatted text; `rewrite_in_place` at `src/main.rs:136`
writes it back. Files are processed with `rayon`'s `par_iter`, and
`report_results` at `src/main.rs:144` prints every error to standard error but
propagates only the first, so `main` exits `1` on any failure.

**Serialization.** `src/main.rs:129-133` and `src/io.rs:21-25` independently
implement the same rule: an empty result yields an empty file; a non-empty
result is joined with `"\n"` and given exactly one trailing newline. Both read
with `str::lines()`, which strips `\n` and a preceding `\r` but does **not**
treat a lone `\r` as a separator. So CRLF input silently becomes LF output, a
file without a trailing newline gains one, and a byte-order mark is passed
through into the first line's content. `src/io.rs`'s `rewrite` and
`rewrite_no_wrap` are public API documented in `README.md` but are not called
by the binary.

**Testing.** Each `.rs` file directly under `tests/` compiles to its own test
binary, so shared helpers are re-declared per binary with
`#[path = "..."] mod ...;`; there is no central registration.
`tests/support/` provides `run_cli_with_args` and `run_cli_with_stdin`.
`tests/cli_matrix.rs` with `tests/cli_matrix/support.rs` implements a pairwise
option matrix expanding curated base rows into wrap and no-wrap variants and
then into standard-output and `--in-place` runs, snapshotting each with
`insta`; `RunResult::envelope` at `tests/cli_matrix/support.rs:191` builds a
labelled block of case identifier, mode, arguments, exit status, standard
output, standard error, and resulting file content. Snapshots live flat under
`tests/snapshots/`. `tests/cli.rs` is at exactly the 400-line cap.
`src/main.rs:252-301` contains `formatting_matches_in_place_output`, a
`proptest` that writes two copies of an input, runs `format_to_string` on one
and `rewrite_in_place` on the other, and compares bytes. That is the precedent
this plan's strongest obligation extends.

**The tracked requirements.** GitHub issue #452 is the check-mode request;
issue #451 is its prerequisite. There is no `docs/roadmap.md`; these two
issues are the roadmap entries this plan discharges.

Terms used throughout:

- Drift: a file whose formatted bytes differ from its current bytes.
- Assessment: a file's current text paired with its formatted text.
- Line delta: counts of inserted and deleted lines, with a changed line
  counting as one of each.
- Display path: the path as supplied on the command line, used in reports.
  Storage key: the bare file name used against a directory capability.

## Conformance basis

There is no Terms of Reference or technical design document in this
repository, and none should be invented. Upstream artefacts:

- `AGENTS.md` at commit `c792270`: style, the 400-line cap, testing
  obligations, dependency policy (`:249-263`), error handling (`:262-283`),
  newtype guidance (`:217-231`), `cap_std`/`camino` preference (`:232-234`),
  observability (`:286-306`), and documentation duties.
- `docs/documentation-style-guide.md`: prose, Markdown, and ADR conventions.
- `docs/architecture.md`: current component narrative and diagrams.
- `docs/developers-guide.md`: internal API reference, the
  "callers select the function that matches their intent rather than passing a
  Boolean mode flag" convention at `:111-113` — superseded in `EP-M7` by the
  shared-closure rule at `:159-163`, since the two functions it contrasted
  (`format_to_string` and `rewrite_in_place`) no longer exist — the CLI matrix
  harness, and observability.
- `docs/adrs/0004-state-machine-abstractions.md`: the ADR header format to
  follow.
- `docs/execplans/cli-matrix-testing.md`: inherited constraints on test
  placement, `.dat` fixtures, and snapshot discipline.
- GitHub issue #452 (partially superseded, see `Decision log`) and issue #451
  (discharged in full).

New ADRs created here, joining the basis once accepted:
`docs/adrs/0008-byte-order-mark-preservation.md` and
`docs/adrs/0009-check-and-diff-reporting.md`. The numbers this plan originally
reserved, `0006` and `0007`, were taken by merged pull requests #470 and #469
before the plan reached `EP-M7`; see `Decision log`. Pull request #469's
`docs/adrs/0007-line-ending-detection.md` covers the line-ending half of the
document boundary only, so the byte-order mark still needs its own record.

Trace links:

```plaintext
ISSUE-451 -> ADR-0007 -> EP-M1 -> tests::document_properties::crlf_round_trips
ISSUE-452-check -> ADR-0009 -> EP-M3 -> tests::cli_check::reports_drift_and_exits_one
ISSUE-452-diff -> ADR-0009 -> EP-M4 -> tests::cli_diff::emits_unified_diff_and_exits_zero
ISSUE-452-no-write -> ADR-0009 -> EP-M3 -> tests::cli_check::directory_snapshot_unchanged
ISSUE-452-exit -> ADR-0009 -> EP-M3 -> tests::driver::exit_status_matrix
ISSUE-452-multifile -> ADR-0009 -> EP-M3 -> tests::cli_check::reports_every_file_in_order
```

Issue #452's `--concise` criteria are deliberately untraced; see
`Decision log`.

## Verification plan

Third-party internals are not verified: `similar`'s diff algorithm, `clap`'s
parsing, `rayon`'s scheduling, and `cap_std`'s capability enforcement are
axioms. Repository-owned logic built on them is verified against the real
interface.

### Axioms

- AX-1: `similar::DiffableStr::tokenize_lines` partitions its input; the
  concatenation of tokens equals the input, terminators are retained, a lone
  `\r` separates, and a final unterminated fragment is emitted. Evidence:
  `similar-2.7.0/src/text/abstraction.rs:101-127`, specifically the inclusive
  ranges at 109/113/117 and the trailing push at 122-124. Consequence: two
  texts have equal token sequences exactly when they are byte-equal.
- AX-2: for `similar::TextDiff::iter_all_changes`, the subsequence tagged
  `Equal` or `Delete` reproduces the old token sequence in order, and the
  subsequence tagged `Equal` or `Insert` reproduces the new one.
- AX-3: `cap_std::fs_utf8::Dir` confines operations to the opened directory.
  This says nothing about ambient writes elsewhere in the process, which is
  why `INV-NOWRITE` snapshots the directory rather than only the inputs.
- AX-4: **not an axiom.** `rayon`'s `collect::<Vec<_>>()` does not document
  order preservation (`rayon-1.12.0/src/iter/from_par_iter.rs:24-34` routes
  through `par_extend` with no ordering statement). The design therefore does
  not rely on it: each unit of work carries its argument index and results are
  ordered on that index. Recorded here so a future reader does not
  reintroduce the assumption.
- AX-5: `str::lines()` splits on `\n`, strips one preceding `\r`, and does
  **not** treat a lone `\r` as a separator. It therefore disagrees with AX-1
  on lone-`\r` input. Every obligation below is stated over exactly one of the
  two notions, never both.
- AX-6: an empty positional list says only that no paths were named; it is not a
  statement about where the input comes from. The driver therefore resolves the
  command line into an `Inputs` value — `Stdin` or `Files(Vec<Utf8PathBuf>)` —
  rather than testing the list for emptiness. This is a design decision rather
  than a third-party fact, recorded here because the `--git` plan (pull request
  #466) depends on it: a source that discovers its own inputs must be able to
  match nothing and exit `0` without falling through to a terminal's standard
  input.

### Obligations

- **INV-PREDICTS**: for every input and every combination of the CLI's eight
  transform flags, `--check` exits `1` if and only if running `--in-place`
  over an identical copy changes that copy's bytes, and the reported counts
  equal the delta between the copy's before and after bytes.
  Method: property test that actually runs both paths over two copies, with a
  third run as the oracle. **Two runs are not enough**, and the reason is the
  same one the rationale records: both modes consult one shared change
  decision, so a decision that answered wrongly would move the report and the
  write in the same direction and the agreement would hold vacuously. Each
  case therefore also runs the document in print mode, which renders the shared
  formatter's output without consulting that decision, and asserts both
  `--in-place`'s bytes and `--check`'s exit status against the printed bytes.
  Rationale: the first draft claimed this held "structurally" because both
  modes read one `Assessment`, and proposed asserting
  `is_changed() == (original != formatted)`. That is the definition of
  `is_changed`, not a test of it, and the structural claim was weaker than
  advertised because the transform closure is supplied by the adapter and
  nothing forced both modes to receive the same one. Running both paths and
  comparing real bytes cannot be satisfied by a wrong closure.
  Domain: generated Markdown mixing tables, prose, lists, fenced code, and
  frontmatter; LF, CRLF, mixed, and byte-order-marked; with and without a
  trailing newline; over the eight-flag powerset, sampled; including CJK and
  combining-mark content, since table padding is width-sensitive. The corpus
  adds one measured fixture per flag, because a flag whose fixture never drifts
  is untested however many cases name it.
  Artefact: `tests/check_prediction.rs` and its corpus module
  `tests/check_prediction/corpus.rs`, split so neither file breaks
  `AGENTS.md`'s 400-line limit. **This obligation was first claimed
  against `tests/check_properties.rs`, which does not test it** — that file
  drives the formatter in print mode and never runs `--check`. The mistake is
  recorded in `Outcomes & retrospective` rather than corrected quietly, since
  the plan's method is to write the falsifiable obligation before the code and
  a claim about evidence is itself a claim.
  Evidence: `cargo test --test check_prediction`.
  Non-vacuity: each corpus case asserts that both a drifting and a clean
  document were observed, and that each fixture drifts under the flag it was
  chosen for; the generated property samples the eight-flag powerset from a
  bitmask and asserts `--check` printed exactly one report line carrying the
  delta recomputed from the two byte strings. Negative control: make `--check`
  compare trimmed strings; a trailing-newline case must fail. **Run, and it
  fails as required** — the corpus fixture `unterminated_clean` drifts by one
  terminator byte only, and the property shrinks to `document = "prose words
  here"`. Transcript in `Artefacts and notes → EP-M7 prediction control`.

- **INV-IDEMPOTENT**: `--check` over the formatter's own output reports clean,
  for every input and flag combination.
  Method: property test.
  Rationale: **the single most important obligation, and it was missing from
  the first draft.** If any transform is not idempotent, a repository can
  never make the gate go green: formatting produces output that the gate then
  rejects. That destroys the feature's purpose. `tests/cli.rs:307-356` already
  round-trips `--in-place` twice for its own cases, which is partial evidence
  only. This obligation must be discharged in `EP-M2`, before any CLI surface
  exists, so a negative result is cheap.
  Domain: as INV-PREDICTS, plus the entire existing `tests/data/` corpus.
  Artefact: `tests/check_properties.rs`.
  Evidence: `cargo test --test check_properties idempotent`.
  Non-vacuity: assert that the first pass genuinely changed something for at
  least some inputs; a generator producing only already-formatted documents
  would pass trivially. Escalate rather than work around a failure, per
  `Tolerances`.

- **INV-NOWRITE**: `--check` and `--diff` leave the working directory
  byte-identical, including entry set, file lengths, and modification times,
  and create no adjacent files.
  Method: type-level argument plus a directory-snapshot end-to-end test.
  Rationale: `ReadOnlyDir` has no write method, so the read-only path cannot
  write through the capability at all — that is the structural argument, and
  it is stronger than the draft's panicking test double. The snapshot covers
  what the type cannot: an ambient `std::fs` write, a `.orig` backup, or a
  lock file, none of which the draft's port-scoped double would have caught.
  Domain: clean, drifting, empty, byte-order-marked, and unreadable files, and
  multi-file batches mixing them.
  Artefact: `tests/cli_check.rs`, `tests/cli_diff.rs`.
  Evidence: `cargo test --test cli_check directory_snapshot_unchanged`.
  Non-vacuity: run the same snapshot assertion against `--in-place`, where it
  must fail. A snapshot helper that always passes is otherwise undetectable.

- **LEM-COUNT**: the reported counts are the insertion and deletion counts of
  a minimal line diff, satisfying
  `formatted_tokens == original_tokens + insertions - deletions` and agreeing
  with `git diff --numstat` on fixed fixtures.
  Method: property test for the conservation law plus golden fixtures for
  minimality.
  Rationale: the conservation law alone is **not** falsifiable. An
  implementation returning `{ insertions: formatted_lines, deletions:
  original_lines }` — no diff at all — satisfies it identically, and is
  indistinguishable from a correct one on this plan's own worked example
  (`+3 -3` on a three-line table). Golden fixtures checked against
  `git diff --numstat` are what make the obligation bite: a single changed
  line in a twenty-line file must report `+1 -1`, not `+20 -20`.
  Domain: empty against non-empty, pure insertion, pure deletion, pure
  replacement, identical, single-line change in a long file.
  Artefact: `tests/check_properties.rs` and `tests/data/numstat/`.
  Evidence: `cargo test --test check_properties count`.
  Non-vacuity: the six classes above must each be exercised. Negative
  controls: count `Equal` as `Insert`; and return whole-file line counts. Both
  must be rejected.

- **INV-AGREE**: the delta is zero in both components exactly when the two
  texts are byte-equal.
  Method: one property assertion, folded into LEM-COUNT's test rather than
  standing alone.
  Rationale: `is_changed` is a byte comparison and the delta comes from
  `similar`; cross-checking two independent routes to the same fact detects a
  mistake in either. By AX-1 it must hold.
  Domain: pairs differing only in line-ending style, and only in the presence
  of a trailing newline — precisely the cases a naive implementation gets
  wrong.
  Evidence: `cargo test --test check_properties agree`.
  Non-vacuity: assert both such pairs are generated. Negative control: compute
  the delta from `str::lines()`-split text; the CRLF-only case must fail.

- **INV-DOCUMENT**: `SourceDocument::parse` followed by `render_lines` over
  the unmodified lines reproduces the input byte-for-byte for any input whose
  line endings are uniform and which has a trailing newline; and for all
  inputs, rendering is a fixed point. Line-ending selection is CRLF exactly
  when CRLF occurrences strictly exceed lone-LF occurrences, and LF otherwise.
  A leading byte-order mark is removed before formatting and restored on
  output.
  Method: parameterized tests for the partition plus a property test for the
  fixed point.
  Domain: pure LF; pure CRLF; mixed with an LF majority; mixed with a CRLF
  majority; an exact tie; no line endings; empty; lone `\r`; leading
  byte-order mark with each ending style; **and mixed endings inside a fenced
  code block**, which is the case that reveals homogenization.
  Artefact: `src/io/document.rs` unit tests and `tests/document_properties.rs`.
  Evidence: `cargo test --lib document` and
  `cargo test --test document_properties`.
  Non-vacuity: the tie case must assert LF. Negative controls: change the
  majority comparison from strictly-greater to greater-or-equal, which the tie
  case must reject; **and count LF with `content.matches('\n').count()`
  without subtracting CRLF occurrences**, which double-counts every CRLF and
  makes CRLF unable to hold a majority — the likelier defect, and the one the
  first draft's control missed entirely.

- **INV-BOM**: a byte-order-marked ragged file reports drift, and a
  byte-order-marked already-formatted file reports clean, with the mark
  preserved in both cases.
  Method: parameterized end-to-end test.
  Rationale: without this the feature silently reports clean on dirty files.
  It is separated from INV-DOCUMENT because the failure mode is a false
  negative in the user-facing guarantee, not a serialization error.
  Artefact: `tests/cli_check.rs`.
  Evidence: `cargo test --test cli_check bom`.
  Non-vacuity: negative control is the current behaviour — do not strip the
  mark; the ragged case must then wrongly report clean and the test must fail.

- **INV-ORDER**: report output lists files in command-line argument order.
  Method: unit test over the ordering function plus an end-to-end test.
  Rationale: since AX-4 is not a contract, this is load-bearing. The first
  draft's control was self-admittedly probabilistic ("would very likely
  produce a different order"), and with eight small files `rayon` will
  normally hand the whole range to one worker, so an unordered implementation
  would pass essentially always. Making ordering explicit by argument index
  turns this into a deterministic unit test on a pure function.
  Domain: eight files alternating clean and drifting, ordered so that
  alphabetical, size, and completion orderings all differ from argument order;
  and, for the unit test, an explicitly shuffled input.
  Artefact: `src/driver.rs` unit tests and `tests/cli_check.rs`.
  Evidence: `cargo test --test cli_check reports_every_file_in_order`.
  Non-vacuity: the unit test feeds results in reverse order and asserts they
  come back in argument order; an implementation that returns them as received
  fails 100% of the time.

- **INV-EXIT**: exit status equals `exit_status(mode, any_drift, any_error)`,
  where any error yields `2` in every mode, drift yields `1` under `--check`
  and `--diff`, and everything else yields `0`.
  Method: exhaustive parameterized unit test plus end-to-end assertions.
  Domain: the full cross product of {all clean, some drift, all drift} with
  {no error, some error} for each of the four modes. The "no files" cell is
  unreachable because all three mode flags require file arguments, so it is
  excluded rather than padding the domain.
  Artefact: `src/driver.rs` unit tests, `tests/cli_check.rs`,
  `tests/cli_diff.rs`.
  Evidence: `cargo test --bin mdtablefix exit_status_matrix`.
  Non-vacuity: three cells carry the real risk and all must be asserted
  explicitly — `--in-place` with drift and no error must be `0`, `--diff` with
  drift and no error must be `1`, and `--check` with both drift and an error
  must be `2`. Negative controls: drop `mode` from the mapping, which the
  first cell must reject; treat `--diff` like `--in-place`, which the second
  must reject; and swap the error-over-drift precedence, which the third must
  reject. The first two controls are distinct: a mapping that suppresses drift
  for every mode except `--check` passes the `--in-place` cell while failing
  `--diff`, so testing only one would leave the other undetected.

- **INV-DETERMINISTIC**: for a fixed input and flag set, standard output is
  byte-identical across repeated runs and independent of file ordering within
  a directory.
  Method: end-to-end test running the same invocation ten times.
  Rationale: determinism is a hard constraint with no obligation in the first
  draft, and every snapshot test depends on it.
  Artefact: `tests/cli_diff.rs`, including an above-threshold case whose
  changes are far enough apart that prefix and suffix trimming cannot reduce
  it to the changed table.
  Evidence: `cargo test --test cli_diff diff_is_deterministic`.
  Non-vacuity: the negative control — enabling `TextDiffConfig::timeout` — was
  applied and removed. It confirmed the hazard on the transition band, where
  ten identical invocations produced up to six distinct outputs, and it failed
  the above-threshold case outright, where a tripped deadline reports 1020
  untouched prose lines as deleted. It also showed that the ten-run method
  detects a budget only when the budget lands mid-computation, which is
  recorded as a residual gap rather than a passing control; see
  `Artefacts and notes → INV-DETERMINISTIC negative control`.

- **INV-SUMMARY**: the summary line renders correctly for every combination of
  zero, one, and many in each of the changed, unchanged, and errored counts.
  Method: exhaustive parameterized test plus `insta` snapshots.
  Rationale: the first draft contained three mutually inconsistent summary
  strings for the same class of run and specified neither pluralization nor
  zero-clause elision. That is exactly the ambiguity its own tolerance says
  should stop work.
  Domain: the 27 combinations of {0, 1, 2} across the three counts.
  Artefact: `src/report/render.rs` unit tests.
  Evidence: `cargo test --lib report::render::summary`.
  Non-vacuity: includes the all-zero case, which must still print something
  intelligible.

- **INV-FRONTMATTER**: an assessment preserves frontmatter bytes exactly,
  including CRLF inside frontmatter, a frontmatter-only file, and an
  unterminated delimiter.
  Method: parameterized test.
  Rationale: `Constraints` mandates routing through `process_with_frontmatter`
  but the first draft had no obligation checking the mandate held.
  Artefact: `tests/check_properties.rs`.
  Evidence: `cargo test --test check_properties frontmatter`.
  Non-vacuity: an unterminated-delimiter fixture must be present, since that
  is where the splitter is most likely to mis-handle the boundary.

### Rigour and residual gaps

`proptest` is used where an invariant ranges over generated inputs;
parameterized tests where the partition is finite and enumerable; `insta`
where output format stability is the requirement; `rstest-bdd` for the
readable end-to-end specification. No bounded model check or formal proof is
planned, and that is a considered judgement rather than an omission: the only
candidate obligation, LEM-COUNT, decomposes into `AX-2` — third-party
behaviour, correctly axiomatized — and an arithmetic identity that reduces to
`a + b = a + c + b - c`. A proof of that would be a restatement of an assumed
property, which `AGENTS.md` explicitly disallows. `EP-M6` spends the effort on
mutation testing instead, which attacks the genuine risk that the counting
mis-attributes a tag.

Remaining gaps to record honestly: a lone `\r` is not a line separator for the
formatter (AX-5) but is for `similar` (AX-1), so counts for such files use
`similar`'s notion; this is documented rather than reconciled. Symlinks
pointing outside their parent directory fail under the capability model where
`cat` would succeed. Paths containing a newline cannot be represented in the
report format and are rejected as operational errors. `INV-DETERMINISTIC` is
held by construction rather than enforced: the renderer chooses its algorithm
from a line count and names no clock, but the ten-run test can only see a
wall-clock budget that lands mid-computation, so a future budget that is never
crossed would pass every suite while making output depend on machine speed. The
control that established this, and the two candidate ways to close it, are in
`Artefacts and notes → INV-DETERMINISTIC negative control`.

## Interfaces and dependencies

### Dependencies

In `Cargo.toml`, using caret ranges per `AGENTS.md:249-255`:

```toml
[package]
version = "0.6.0"

[dependencies]
similar = "2.7"

[dev-dependencies]
googletest = "0.14"
pretty_assertions = "1"
rstest-bdd = "0.5.0"
rstest-bdd-macros = { version = "0.5.0", features = ["strict-compile-time-validation"] }
```

`similar = "2.7"` unifies with the copy `insta` already resolves; verify with
`cargo tree --duplicates`. `strict-compile-time-validation` turns a missing
step definition into a compile error. Both `rstest-bdd` crates need Rust 1.85
or newer; this repository pins `1.89` in `Cargo.toml` and
`nightly-2026-03-26` in `rust-toolchain.toml`.

### `src/io/document.rs` (new, library, budget 200 lines)

**Relocated and narrowed during the rebase onto `origin/main`.** This module
was specified as `src/document.rs` with its own `LineEnding` enum. PR #469 then
landed `src/io/line_endings.rs` carrying an equivalent `LineEnding`,
re-exported from the crate root and already asserted by
`tests/document_properties.rs`. Two things followed:

- The module moved under `src/io/`. A top-level `src/document` and an `src/io`
  that both needed the line-ending policy would be a cyclic module dependency,
  and defining a second public `LineEnding` beside the crate-root re-export
  would leave two types with one name.
- `LineEnding`, `LineEndingCounts`, `count_line_endings`, and `serialize_lines`
  now come from `super::line_endings` rather than being redefined here.

`SourceDocument` is unchanged in shape and still guarantees at the type level
that a document's lines are rendered with that document's own style. Read the
interface below with those two substitutions; it is otherwise the original
specification.

The shared document boundary, replacing the duplicated logic in
`src/main.rs:129-133` and `src/io.rs:21-25`. Infallible; no error type.

```rust
/// The line-ending style used by a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// A single line feed, `\n`.
    Lf,
    /// A carriage return followed by a line feed, `\r\n`.
    Crlf,
}

impl LineEnding {
    /// Returns the characters written between lines.
    pub fn as_str(self) -> &'static str;

    /// Selects the style holding a strict majority of `content`'s line
    /// endings, defaulting to [`LineEnding::Lf`] on a tie or when there are
    /// none.
    ///
    /// Counts CRLF occurrences, then subtracts them from the total line-feed
    /// count to obtain lone line feeds. Counting line feeds without that
    /// subtraction double-counts every CRLF and makes CRLF unable to win.
    pub fn detect(content: &str) -> Self;
}

/// A parsed document: its lines, the line-ending style to restore, and
/// whether it began with a byte-order mark.
///
/// The byte-order mark is split off before formatting because leaving it
/// attached to the first line prevents every content transform from matching,
/// which would make `--check` report a ragged file as clean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    has_byte_order_mark: bool,
    lines: Vec<String>,
    line_ending: LineEnding,
}

impl SourceDocument {
    /// Splits `content` into lines, recording its byte-order mark and
    /// majority line-ending style.
    pub fn parse(content: &str) -> Self;

    /// Borrows the parsed lines, with line endings and any byte-order mark
    /// removed.
    pub fn lines(&self) -> &[String];

    /// Serializes `lines` using this document's byte-order mark and
    /// line-ending style.
    ///
    /// An empty slice yields an empty string. A non-empty slice is joined
    /// with the line ending and terminated with one further line ending.
    ///
    /// This is a method rather than a free function so a caller cannot pass a
    /// line-ending style belonging to a different document.
    pub fn render_lines(&self, lines: &[String]) -> String;
}
```

### `src/report.rs` and `src/report/` (new, library, budget 150 + 200 + 250)

Pure reporting. No input or output, no paths opened, no error type. Named
`report` rather than `check` because it serves `--check` and `--diff` equally.

```rust
// src/report.rs
pub use crate::report::delta::LineDelta;
pub use crate::report::render::{
    render_report_line, render_summary, write_unified_diff, DiffOptions,
};

/// What one file's analysis produced, ready to render.
///
/// Returning a value rather than pre-rendered text keeps a future
/// `--format=json` a leaf addition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    /// The path as supplied on the command line.
    pub display_path: camino::Utf8PathBuf,
    /// Whether the file's bytes would change.
    pub is_changed: bool,
    /// Line counts, zero in both components when `is_changed` is false.
    pub delta: LineDelta,
}
```

```rust
// src/report/delta.rs

/// Counts of lines inserted and deleted between two texts.
///
/// A modified line counts as one insertion and one deletion, matching
/// `git diff --numstat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LineDelta {
    insertions: usize,
    deletions: usize,
}

impl LineDelta {
    /// Counts the line-level changes turning `original` into `formatted`.
    ///
    /// Callers must not invoke this when the texts are byte-equal; the caller
    /// compares bytes first so a clean tree costs no diff work.
    pub fn between(original: &str, formatted: &str) -> Self;

    /// The number of lines that would be added.
    pub fn insertions(self) -> usize;

    /// The number of lines that would be removed.
    pub fn deletions(self) -> usize;

    /// Whether either count is non-zero.
    pub fn has_changes(self) -> bool;
}
```

Note `has_changes`, not `is_empty`: the first draft named it `is_empty` while
documenting the opposite meaning, and `is_empty` would also pull in Clippy's
`len_without_is_empty` expectations.

```rust
// src/report/render.rs

/// Renders one report line, for example `docs/a.md +12 -8`.
///
/// Consumers parse by taking the final two whitespace-separated fields as the
/// counts and everything before them as the path.
pub fn render_report_line(display_path: &camino::Utf8Path, delta: LineDelta) -> String;

/// Renders the human summary, for example
/// `2 files would be reformatted, 1 file left unchanged.`
///
/// Clauses are elided at zero and use singular or plural forms as
/// appropriate. When all three counts are zero the result is
/// `No files were analysed.`. This goes to standard error so that standard
/// output stays a machine contract.
pub fn render_summary(changed: usize, unchanged: usize, errored: usize) -> String;

/// Configuration for unified-diff rendering.
#[derive(Debug, Clone, Copy)]
pub struct DiffOptions {
    /// Lines of context around each hunk. Always three.
    pub context_radius: usize,
    /// Above this many lines on either side, switch from Myers to Patience so
    /// the cost stays bounded without a wall-clock cut-off.
    pub patience_threshold: usize,
}

/// Streams a unified diff for `display_path` into `out`.
///
/// Headers name `display_path` on both sides with directory separators
/// normalized to `/`, and carry no timestamps, so output is deterministic and
/// snapshot-stable. The `\ No newline at end of file` marker is retained; it
/// can only ever appear on the `-` side, because the formatter always emits a
/// trailing terminator. Output is never colourized.
///
/// # Errors
/// Returns an error if `out` fails.
pub fn write_unified_diff(
    out: &mut impl std::io::Write,
    display_path: &camino::Utf8Path,
    original: &str,
    formatted: &str,
    options: DiffOptions,
) -> std::io::Result<()>;
```

The summary grammar, fixed and snapshot-tested:

```plaintext
<n> file(s) would be reformatted, <n> file(s) left unchanged, <n> file(s) could not be read.
```

with any clause whose count is zero omitted, `file`/`files` agreeing with its
own count, and `No files were analysed.` when all three are zero.

### `src/driver.rs` (new, binary-private, budget 300 lines)

Declared by `src/main.rs` as `mod driver;`. Lives in the binary because that
is the application boundary where `anyhow` is permitted; the library stays
infallible. Not part of the published library surface.

```rust
/// A directory capability that can only read.
///
/// `--check` and `--diff` receive this instead of a [`cap_std::fs_utf8::Dir`]
/// so that a wrong `match` arm cannot write. The read-only guarantee is
/// therefore a property of the type, not of a test double.
pub struct ReadOnlyDir(cap_std::fs_utf8::Dir);

impl ReadOnlyDir {
    /// Wraps a directory capability, discarding write access.
    pub fn new(directory: cap_std::fs_utf8::Dir) -> Self { Self(directory) }

    /// Reads `name` as UTF-8 text.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or is not valid UTF-8.
    pub fn read(&self, name: &camino::Utf8Path) -> anyhow::Result<String>;
}

/// A file's current text paired with the text the formatter would write.
pub struct Assessment {
    original: String,
    formatted: String,
}

impl Assessment {
    /// Whether writing the formatted text would change the file's bytes.
    ///
    /// A direct byte comparison, and the authoritative answer.
    pub fn is_changed(&self) -> bool { self.original != self.formatted }
}

/// What the caller asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode { Print, InPlace, Check, Diff }

/// The documented process exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus { Success, Drift, Error }

/// Maps mode and observations onto an exit status.
///
/// An error yields [`ExitStatus::Error`] in every mode, because an incomplete
/// analysis must not be reported as merely drifted. Drift yields
/// [`ExitStatus::Drift`] under the read-only reporting modes [`Mode::Check`]
/// and [`Mode::Diff`]; in particular a successful `--in-place` over drifting
/// files yields [`ExitStatus::Success`], as does a bare invocation.
pub fn exit_status(mode: Mode, any_drift: bool, any_error: bool) -> ExitStatus;

/// Where the text to format comes from. See `AX-6`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inputs {
    /// No paths were named: the document is read from standard input.
    Stdin,
    /// The named files, in argument order.
    Files(Vec<camino::Utf8PathBuf>),
}

impl Inputs {
    /// Resolves the command line's positional arguments.
    ///
    /// # Errors
    /// Returns an error naming the offending path if any argument is not
    /// valid UTF-8.
    pub fn resolve(files: Vec<std::path::PathBuf>) -> anyhow::Result<Self>;
}

/// Reads a file and pairs its text with the formatted result.
///
/// Takes [`ReadOnlyDir`], so this function cannot write. `storage_key` is the
/// bare file name within the capability, and is what the line-ending report
/// names; the display path is attached by the caller, the only place that
/// knows how the file was written on the command line.
///
/// # Errors
/// Returns an error if the file cannot be read.
pub fn assess(
    directory: &ReadOnlyDir,
    storage_key: &camino::Utf8Path,
    format: &(dyn Fn(&SourceDocument) -> String + Sync),
) -> anyhow::Result<Assessment>;

/// Writes the formatted text back.
///
/// Only reachable from [`Mode::InPlace`], and only for a file whose bytes would
/// change: the replacement renames a temporary over the target, so an
/// unconditional call would swap the inode of a file it left byte-identical.
///
/// # Errors
/// Returns an error if the file cannot be written.
pub fn write_back(
    directory: &cap_std::fs_utf8::Dir,
    storage_key: &camino::Utf8Path,
    assessment: &Assessment,
) -> anyhow::Result<()>;

/// Orders indexed results by argument index.
///
/// Ordering is explicit rather than inherited from `rayon`'s collection
/// order, which is not a documented guarantee. See `AX-4`.
pub fn in_argument_order<T>(results: Vec<(usize, T)>) -> Vec<T>;
```

The parallel stage produces `(usize, anyhow::Result<(FileReport, String)>)`
per file and drops each `Assessment` inside its closure, so retained memory is
proportional to the rendered reports rather than to twice the total input.

### `src/main.rs` (modified, binary, budget 350 lines)

Adapter only: `clap` inbound, `cap_std` and the standard streams outbound.

```rust
#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
#[command(group(clap::ArgGroup::new("inputs").args(["files"])))]
#[command(group(clap::ArgGroup::new("mode").multiple(false).requires("inputs")))]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", group = "mode")]
    in_place: bool,
    /// Report which files would be reformatted, and by how many lines
    #[arg(long = "check", group = "mode")]
    check: bool,
    /// Print a unified diff for each file that would be reformatted
    #[arg(long = "diff", group = "mode")]
    diff: bool,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}
```

The `inputs` group holds the input *sources* — today the `files` positional
alone — and `mode` requires it, so a mode flag still demands something to act
on while the source itself is a named thing to extend. The requesting `--git`
plan needs the separate group: its `--git` is a second source, and a
flag-to-positional `requires` would have bound every mode flag to files.

`run` resolves the command line into an `Inputs` value before doing anything
else — see `AX-6` — and routes a resolution failure through
`exit_status(mode, false, true)`, so it exits `2` like every other operational
error rather than propagating out of `run`. `run_stdin` and `run_files` hold the
two halves that the empty-list test used to jump between.

`fn main` returns `std::process::ExitCode`. A broken pipe on standard output,
as in `mdtablefix --check *.md | head`, must be caught and treated as a
successful early exit rather than allowed to panic into exit `101`, which
would be a fourth undocumented status. The formatting closure is built once
and shared by every mode, so `--check` and `--in-place` cannot diverge.

### `src/io.rs` (modified, library)

`rewrite` and `rewrite_no_wrap` keep their exact signatures. Only
`rewrite_with`'s body changes, routing through `SourceDocument` so library
consumers gain the same preservation. Migrating this module from `std::fs` to
`cap_std` is out of scope; issue #418 tracks it.

## Milestones and plateaus

### EP-M0: prototyping spike

Outcome: two unknowns answered, then deleted. What exactly does `similar`'s
unified diff produce for a small ragged table, and where does the
missing-newline marker appear? Does a minimal `rstest-bdd` scenario compile,
link, and run here with `strict-compile-time-validation`?

Acceptance: the diff transcript is pasted into `Artefacts and notes`; the
canary scenario passes, and deleting a step definition produces a compile
error, proving strict validation is active.

Fallback: if the canary exceeds one day, abandon `rstest-bdd` for this plan,
use plain `rstest` plus `assert_cmd` for the behavioural tests, and record the
decision. The Gherkin scenarios below map one-to-one onto `rstest` cases.

Recovery: `git restore` the spike files.

Compatibility decision: none required.

### EP-M1: document boundary

Outcome: `src/io/document.rs` exists; `src/main.rs` and `src/io.rs` both use
it; line endings and byte-order marks are preserved in every mode. Issue #451
is discharged: the line-ending half by merged pull request #469, whose
implementation was taken during the rebase, and the byte-order-mark half by
`src/io/document.rs`, which this branch re-landed because `git grep` showed
`main` had no mark handling anywhere. No new flag exists yet.

Requirements: `ISSUE-451`, `INV-DOCUMENT`, `INV-BOM` (partially; its
end-to-end half lands in `EP-M3`).

**Fixtures first.** The repository has no CRLF, byte-order-mark, lone-`\r`, or
empty-file coverage whatsoever. Add those fixtures and their assertions
against the *current* behaviour before touching the serialization path, so
there is a real oracle. Include the mixed-endings-inside-a-fence case.

Acceptance: `cargo test --lib document`, `cargo test --test
document_properties`, and the entire pre-existing suite pass. A CRLF fixture
survives `--in-place`. A byte-order-marked ragged file is reflowed and keeps
its mark.

Conformance check: `src/io.rs` signatures unchanged; `mdtablefix::document` is
a public addition; written bytes change for CRLF and byte-order-marked inputs,
which is the requested behaviour and is recorded in `CHANGELOG.md`.

Recovery: revert the commits. **Point of no return:** once `--in-place` has
run across a repository under the new behaviour, reverting this milestone does
not un-rewrite those users' files. This is the only irreversible step in the
plan, and it arrives first. Say so in the changelog.

Remaining gaps: no reporting modes.

Compatibility decision: none required; signatures are stable and the
behaviour change is the requested fix.

### EP-M2: pure reporting domain

Outcome: `src/report.rs`, `src/report/delta.rs`, `src/report/render.rs` exist
and are fully tested. Nothing in the binary calls them yet.

Requirements: `LEM-COUNT`, `INV-AGREE`, `INV-SUMMARY`, `INV-IDEMPOTENT`.

`INV-IDEMPOTENT` is discharged here, before any CLI surface exists, so that a
non-idempotent transform is discovered at its cheapest point. Per
`Tolerances`, a failure stops the plan rather than being worked around.

Acceptance: `cargo test --lib report`, `cargo test --test check_properties`,
including the six LEM-COUNT classes, both negative controls, and the golden
`git diff --numstat` fixtures.

Conformance check: `similar` is a direct runtime dependency;
`cargo tree --duplicates` shows one copy.

Recovery: the modules are additive and unreferenced; delete them.

Compatibility decision: none required.

### EP-M3: driver, `--check`, and the exit-status contract

Outcome: `src/driver.rs` exists with `ReadOnlyDir`, `Assessment`, `Mode`,
`ExitStatus`, `assess`, `write_back`, `exit_status`, and `in_argument_order`.
`src/main.rs` returns `ExitCode` and supports `--check`. The crate is
`0.6.0`. All four modes share one formatting closure.

Requirements: `ISSUE-452-check`, `ISSUE-452-no-write`, `ISSUE-452-exit`,
`ISSUE-452-multifile`, `INV-PREDICTS`, `INV-NOWRITE`, `INV-ORDER`, `INV-EXIT`,
`INV-BOM`, `INV-FRONTMATTER`.

Acceptance: `tests/cli_check.rs` and the `src/driver.rs` unit tests pass;
`tests/features/check_mode.feature` passes; all three exit statuses are
asserted, **including that `--in-place` over a drifting file exits `0`** and
that drift plus an error yields `2`.

Conformance check: `main`'s return type changed and the error status moved
from `1` to `2`, requiring ADR 0009; the manifest already declares `0.6.0` as
unreleased, so the bump recorded under `EP-M0` covers the change and no further
version bump is needed. `--check` is a new
public command-line interface approved at the gate; no trust boundary widens,
because access still flows through `open_file_parent`.

Recovery: revert; `EP-M2`'s modules become unreferenced but remain correct.

Compatibility decision: none required. The exit-status change is deliberate on
a pre-1.0 tool with no compatibility commitment, signalled by the minor
version bump rather than shimmed.

### EP-M4: `--diff`

Outcome: `--diff` is implemented, streams a deterministic unified diff per
changed file, and exits `1` on drift exactly as `--check` does, so one run
both shows the drift and fails the build.

Requirements: `ISSUE-452-diff`, `INV-DETERMINISTIC`, and the `--diff` cells of
`INV-EXIT`.

Acceptance: `tests/cli_diff.rs` and `tests/features/diff_mode.feature` pass;
the determinism test passes over ten runs, below and above the line-count
threshold; `--diff` over a drifting file exits `1` and over a clean file exits
`0`. The first draft's criterion that `patch` reproduce the file is dropped, per
`Decision log`, which also removes an undeclared external tool dependency from
the suite.

Control outcome: the wall-clock-budget control was applied and removed. It
rejected the above-threshold test and made the transition band nondeterministic,
and it showed the method's blind spot; see
`Artefacts and notes → INV-DETERMINISTIC negative control`.

Recovery: revert; `--check` remains functional.

Compatibility decision: none required.

### EP-M5: curated CLI matrix coverage

Outcome: `--check` and `--diff` are exercised over a **curated subset** of
transform combinations in the option matrix, not the full cross product, and
`RunResult::envelope` elides the resulting-file-content block for read-only
modes, where it is by definition unchanged.

Rationale: `tests/snapshots/` holds 32 matrix snapshots for two modes.
Expanding to four modes across the full matrix would add 32 more, exceeding
the usual churn limit and doubling review burden for no additional signal,
since `--check` and `--diff` share their entire analysis path with the
existing modes and differ only in rendering.

Acceptance: `cargo test --test cli_matrix` passes with the harness self-tests
updated to assert the curated expansion is complete and intentional.

Conformance check: `docs/developers-guide.md`'s matrix section is updated in
the same commit.

Recovery: revert; the standalone tests still cover both features.

Compatibility decision: none required; test-only surface.

### EP-M6: targeted mutation testing

Outcome: `cargo mutants --file src/report/delta.rs --file src/driver.rs`
reports no surviving mutants, or each survivor is either killed by a new test
or recorded with a justification.

Rationale: this replaces the first draft's Verus milestone, which `Decision
log` cuts. Mutation testing attacks the plan's actual stated risk — that the
counting mis-attributes a tag, or that the exit-status mapping loses a case —
directly and empirically, at a fraction of the cost of a toolchain adoption.

Fallback: if `cargo-mutants` is unavailable, skip and record the gap. It is a
developer tool, not a manifest entry, and is not added to any gate.

Blocker (recorded 2026-09-11): `cargo-mutants` is available, but this milestone
cannot start. It refuses to test any mutant while the unmutated baseline
`cargo test` fails, and the baseline is red on `a06bab6` because
`generated_documents_reach_a_fixed_point` reaches a genuine third
non-idempotent transform class with `--headings`. Excluding the failing test
from the baseline, or pinning a passing seed, would produce a score for a tree
whose own gate is red; both are rejected, so the milestone is deferred until
the transform is fixed (GitHub issue #474). See
`Artefacts and notes → EP-M6 baseline blocked`.

Blocker cleared (recorded in Revision 17): the transform was fixed upstream, by
pull request #477, which refuses Setext conversion of a table delimiter row. The
branch rebased onto that commit as `408c76a`, the gate runner then measured
`make test` green — `generated_documents_reach_a_fixed_point` itself reports
`ok` — and so the baseline precondition below is met. The milestone is unblocked
and remains unrun; the condition it was never allowed to satisfy itself, "green
by exclusion or by a pinned seed", therefore still holds.

Outcome (recorded in Revision 18): the milestone's command was run on the
rebased tip and the outcome above is met. Of the **46** mutants it collected —
28 in `src/driver.rs` and 18 in `src/report/delta.rs`, three more than the
enumeration below because `4a599ed` landed after it — the first run, in 14
minutes, **caught 38, found 6 unviable, and left 2 survivors, with no timeouts**,
and a third run over the final tree, in 9 minutes, measured **39 caught, 1
missed, 6 unviable, 0 timeouts** in a single reading. Both initial survivors are
resolved: `src/report/delta.rs:92:84` (`>` mutated to `<` in
`LineDelta::has_changes`) is killed by an assertion covering the deletion-only
delta, with its insertion-side twin added for symmetry, and re-measured as
caught; `src/driver.rs:287:23` (`Mode::Diff if is_changed` mutated to `if true`)
is recorded as an equivalent mutation, because the renderer writes nothing for
byte-equal texts, which is now a test rather than an argument. The final score
is **39 of 40 viable mutants killed**, one run having measured it. See
`Artefacts and notes → EP-M6 run`.

Recovery: additive; no production change unless a survivor is found.

Compatibility decision: none required.

### EP-M7: documentation and closure

Outcome: user-facing, architectural, and developer-facing documentation is
current; both ADRs are written; `CHANGELOG.md` records the four behaviour
changes; `docs/contents.md` indexes every new document; issues #451 and #452
are closed with an explanatory comment.

Acceptance: `make markdownlint` and `make nixie` pass; every new document is
reachable from `docs/contents.md`; both issues are closed.

Recovery: documentation-only.

Compatibility decision: none required.

## Behaviour specifications

Two Gherkin feature files drive the behavioural tests. Create them before the
implementation they specify. Paths in `#[scenario(path = "...")]` are relative
to the crate root.

### `tests/features/check_mode.feature` (EP-M3)

```gherkin
Feature: Report which Markdown files would be reformatted

  Scenario: A clean file reports no drift and succeeds
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--check" against those files
    Then the exit status is 0
    And standard output is empty
    And the summary reads "1 file left unchanged."
    And the working directory is byte-identical

  Scenario: A drifting file is reported with its line counts
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--check" against those files
    Then the exit status is 1
    And standard output is "ragged.md +3 -3"
    And the working directory is byte-identical

  Scenario: Every supplied file is reported in argument order
    Given a Markdown file "clean.md" that is already formatted
    And a Markdown file "zebra.md" with an unaligned table
    And a Markdown file "alpha.md" with an unaligned table
    When mdtablefix runs with "--check" against those files
    Then the exit status is 1
    And standard output lists "zebra.md" before "alpha.md"
    And the summary reads "2 files would be reformatted, 1 file left unchanged."

  Scenario: An unreadable file yields the error status, not the drift status
    Given a Markdown file "ragged.md" with an unaligned table
    And a path "missing.md" that does not exist
    When mdtablefix runs with "--check" against those files
    Then the exit status is 2
    And standard error mentions "missing.md"
    And the summary reports 1 file could not be read

  Scenario: A CRLF file needing no Markdown changes reports clean
    Given a Markdown file "windows.md" already formatted with CRLF endings
    When mdtablefix runs with "--check" against those files
    Then the exit status is 0
    And standard output is empty

  Scenario: A byte-order-marked ragged file is not reported as clean
    Given a Markdown file "bom.md" with a byte-order mark and an unaligned table
    When mdtablefix runs with "--check" against those files
    Then the exit status is 1
    And standard output is "bom.md +3 -3"

  Scenario: In-place formatting of a drifting file still succeeds
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--in-place" against those files
    Then the exit status is 0
    And "ragged.md" is reformatted

  Scenario: Check mode rejects being combined with in-place mode
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--check --in-place" against those files
    Then the exit status is 2
    And standard error mentions "cannot be used with"
```

### `tests/features/diff_mode.feature` (EP-M4)

```gherkin
Feature: Show what would change in Markdown files

  Scenario: A drifting file produces a unified diff and fails
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--diff" against those files
    Then the exit status is 1
    And the diff header names "ragged.md" on both sides
    And the diff contains a hunk header
    And the working directory is byte-identical

  Scenario: A clean file produces no diff
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--diff" against those files
    Then the exit status is 0
    And standard output is empty

  Scenario: A drifting file under in-place formatting still succeeds
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--in-place" against those files
    Then the exit status is 0

  Scenario: Diff output is byte-identical across repeated runs
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--diff" against those files ten times
    Then every run produced identical standard output

  Scenario: An unreadable file yields the error status
    Given a path "missing.md" that does not exist
    When mdtablefix runs with "--diff" against those files
    Then the exit status is 2
    And standard error mentions "missing.md"

  Scenario: Diff mode rejects being combined with check mode
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--diff --check" against those files
    Then the exit status is 2
    And standard error mentions "cannot be used with"
```

Step definitions live in `tests/steps/reporting.rs`, shared by both feature
files, with scenario bindings in `tests/bdd_reporting.rs`. Scenario state is
an `rstest` fixture rather than a global world, following the `rstest-bdd`
guidance that fixtures are the world: a `#[derive(ScenarioState)]` struct
carrying `Slot<TempDir>`, `Slot<Vec<Utf8PathBuf>>`, and
`Slot<std::process::Output>`. The `When` steps run the real binary through
`assert_cmd::Command::cargo_bin("mdtablefix")` and capture `Output`, so both
streams and the status can be asserted. The `rstest-bdd` user's guide does not
cover subprocess testing, so this harness is a repository-local convention and
must be documented in `docs/developers-guide.md`.

`tests/cli_check.rs` and `tests/cli_diff.rs` carry only the cases that read
badly as prose: the eight-file ordering batch, the `INV-EXIT` cross product,
and the directory-snapshot assertions.

## Plan of work

Stage A is this document, ending at the approval gate. Stage B writes the
failing tests and feature files for a milestone. Stage C implements that
milestone's production code and its verification artefacts together. Stage D
covers documentation and wider validation. Each stage ends with validation;
do not proceed past a failing stage.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---mdtablefix/worktrees/cfdaa2f9-abd0-4c67-9f5f-0531a93ac8e6`.

Log every gate so truncated console output can be reviewed afterwards:

```bash
make test 2>&1 | tee "/tmp/test-mdtablefix-$(git branch --show-current).out"
```

Substitute `check-fmt`, `typecheck`, `lint`, `markdownlint`, or `nixie` and
change the log prefix to match. Do not run gates in parallel; this environment
relies on build caching and sequential runs are fastest. Delegate full gate
runs to the `scrutineer` subagent and read the cited `/tmp` log rather than
re-running a gate to diagnose a failure.

### EP-M0

1. `cargo add similar@2.7`, then confirm `cargo tree --duplicates | grep
   similar` prints nothing.
2. Write a scratch test printing the unified diff for a two-line ragged table;
   run `cargo test --lib -- --nocapture spike` and paste the output into
   `Artefacts and notes`.
3. Add the four development dependencies, create `tests/features/canary.feature`
   with one trivial scenario and a matching step file, and run
   `cargo test --test bdd_reporting`. Delete a step definition and confirm a
   compile error, proving strict validation is active.
4. Delete the scratch test and the canary. Commit the dependency additions and
   the version bump alone.

### EP-M1

1. Add CRLF, mixed-endings, mixed-endings-inside-a-fence, byte-order-mark,
   lone-`\r`, empty-file, and no-trailing-newline fixtures under
   `tests/data/`, with tests asserting the **current** behaviour. Commit. This
   is the regression oracle, and it does not exist yet.
2. Red: add `src/io/document.rs` with signatures and `todo!()` bodies, plus its
   unit tests and `tests/document_properties.rs`. Run `cargo test --lib
   document` and observe the `todo!()` panics.
3. Green: implement `LineEnding::detect` (subtracting CRLF occurrences from
   the line-feed count), `SourceDocument::parse` (splitting off the byte-order
   mark), and `render_lines`.
4. Refactor: replace the join logic in `src/main.rs:129-133` and
   `src/io.rs:21-25` with `SourceDocument`. Update the step-1 fixture
   assertions to the new expected behaviour, reviewing each change
   individually — an unexplained change here is a transform regression and
   trips a tolerance.
5. Run all gates and commit.

### EP-M2

1. Red: create `src/report.rs`, `src/report/delta.rs`, `src/report/render.rs`
   with signatures and `todo!()` bodies; write `tests/check_properties.rs`,
   the golden `tests/data/numstat/` fixtures, and the unit tests. Observe the
   red failure.
2. Green: implement `LineDelta::between` over
   `TextDiff::from_lines(original, formatted).iter_all_changes()`, counting
   `ChangeTag::Insert` and `ChangeTag::Delete`; implement the renderers using
   `UnifiedDiff::to_writer` so `--diff` streams rather than materializing.
3. Run `INV-IDEMPOTENT` first among the property tests. **If it fails, stop
   and escalate**; do not proceed.
4. Apply each negative control from `Verification plan` as a temporary local
   mutation, confirm the intended failure, revert it, and record the observed
   message in `Artefacts and notes`.
5. Run all gates and commit.

### EP-M3

1. Red: write `tests/features/check_mode.feature`, `tests/steps/reporting.rs`,
   `tests/bdd_reporting.rs`, and `tests/cli_check.rs`. They will not compile,
   because `--check` does not exist; record the exact error. **Observed:** they
   *do* compile, and fail at run time instead — `--check` is an unknown
   *argument*, so the binary builds, `clap` rejects the flag, and the run exits
   `2`. See `Artefacts and notes → EP-M3`, which also records the test that
   passed vacuously in that state.
2. Green: add `src/driver.rs`; add the `mode` argument group and `--check`;
   change `fn main` to return `ExitCode` and to handle a broken pipe without
   panicking; build the formatting closure once and share it. `Mode::Diff` is
   deliberately *not* added here: `EP-M4` adds the variant with its arm and its
   `INV-EXIT` cells, so no variant is ever left unconstructed and no dead-code
   suppression is needed.
3. Add the `src/driver.rs` unit tests, including the `INV-EXIT` cross product
   and the `in_argument_order` reverse-order test.
4. Confirm the three statuses, and specifically that `--in-place` over a
   drifting file exits `0`.
5. Run all gates and commit.

### EP-M4

1. Red: write `tests/features/diff_mode.feature` and `tests/cli_diff.rs`.
2. Green: add `--diff` and the `Mode::Diff` arm, including the
   `patience_threshold` switch, and extend `exit_status` so `Mode::Diff`
   reports drift. Confirm the `--in-place` cell of `INV-EXIT` still yields
   `0`; that pair of assertions is what stops the two modes being conflated.
3. Run all gates and commit.

### EP-M5

1. Extend the matrix's execution-mode expansion to the curated `--check` and
   `--diff` subset, and make `RunResult::envelope` elide file content for
   read-only modes.
2. Regenerate with `INSTA_UPDATE=always cargo test --test cli_matrix
   cli_matrix_snapshots`, then review every changed `.snap` before staging.
   Do not accept snapshots mechanically.
3. Update `docs/developers-guide.md` in the same commit. Run all gates and
   commit.

### EP-M6

0. Precondition: `make test` is green, because `cargo-mutants` aborts on a red
   baseline. **Not met at `a06bab6`**; **met at `bb068f1`**, the rebased tip
   measured in Revision 17. See `Artefacts and notes → EP-M6 baseline blocked`.
   The `--headings` table/setext defect (issue #474) was fixed upstream by pull
   request #477 rather than by this branch. Do not satisfy the precondition by
   excluding the test or pinning a seed, in this tree or any later one.
1. `cargo mutants --file src/report/delta.rs --file src/driver.rs`.
2. Kill each survivor with a test, or record why it is acceptable.

### EP-M7

1. Add a command-line interface section to `docs/users-guide.md` covering
   every flag, the exit-status contract, line-ending and byte-order-mark
   behaviour, the trailing-newline rule, the fenced-code homogenization
   caveat, the lone-`\r` limitation, the symlink limitation, the empty-glob
   hazard, and how to parse the report line.
2. Reduce `README.md`'s flag list to a synopsis linking to the user's guide,
   updating the usage line for `--check` and `--diff`.
3. Add a "Check and diff reporting" section to `docs/architecture.md` near
   "Concurrency with `rayon`"; update the Module Relationships diagram for
   `document`, `report`, and `driver`; update the `## Contents` index. The
   existing diagram already names functions that no longer match
   `src/main.rs`; correct those while editing it.
4. Add sections to `docs/developers-guide.md` covering the `ReadOnlyDir`
   capability and its re-use policy, the shared-closure rule that keeps
   `--check` and `--in-place` in agreement, the explicit argument-index
   ordering and why `rayon`'s collection order is not relied upon, the
   `rstest-bdd` conventions and subprocess harness, and the binary-private
   status of `src/driver.rs`.
5. Write `docs/adrs/0008-byte-order-mark-preservation.md` and
   `docs/adrs/0009-check-and-diff-reporting.md` following ADR 0004's header
   format. Do not renumber the existing `0006` and `0007`, which merged pull
   requests already filled.
6. Vendor `docs/rstest-bdd-users-guide.md` and
   `docs/reliable-testing-in-rust-via-dependency-injection.md` with a
   provenance header naming the source repository and commit.
7. Add `CHANGELOG.md` entries for `--check`, `--diff`, line-ending
   preservation, byte-order-mark preservation, and the exit-status change,
   noting explicitly that `--in-place` now rewrites CRLF files differently and
   that this is not reversible for files already rewritten.
8. Update `docs/contents.md` for every new document, and add the missing entry
   for `docs/state-machine-abstractions-roadmap.md`.
9. Run `make markdownlint`, `make nixie`, and all Rust gates, then commit. Do
   **not** run `make fmt`: `mdformat-all` is repository-wide and is not a gate,
   and 10 of the 31 tracked Markdown files already drift under its flag set.
   See `Artefacts and notes → make fmt measured, and declined`.
10. Close issues #451 and #452 with a comment linking this plan and explaining
    the `--concise` supersession.
11. Added in Revision 14, after the closing audit found `INV-PREDICTS` claimed
    against a file that does not exercise the reporting modes: write
    `tests/check_prediction.rs` and its `tests/check_prediction/corpus.rs`
    module, which run `--check` and `--in-place` over byte-identical copies with
    the printer as the oracle, and run the obligation's negative control as a
    real mutation. See `Artefacts and notes → EP-M7 prediction control`.
12. Added in Revision 15: amend `docs/developers-guide.md`'s "One formatter,
    built once" so that the shared-closure argument is stated as necessary but
    not sufficient and the test that covers the rest is named. The section
    previously left a reader able to conclude that the structure alone makes
    the two modes agree, which is the reading Revision 14 refuted.

## Validation and acceptance

A reviewer should reproduce each of the following without reading source.

```bash
cargo build --bin mdtablefix
export MDT=./target/debug/mdtablefix
printf '| A   | B   |\n| --- | --- |\n| 1   | 2   |\n' > clean.md
printf '|A|B|\n|---|---|\n|1|2|\n' > ragged.md
cp ragged.md ragged.md.orig
```

`clean.md` holds the formatter's **own** bytes, not a hand-written
approximation: cells are padded to the delimiter row's width, so `| A | B |` is
itself reported as drift (`+2 -2`). See `Artefacts and notes → EP-M3`. The
padding is also why a ragged table reports `+3 -3` rather than `+2 -2`.

A clean file succeeds silently on standard output:

```console
$ $MDT --check clean.md; echo "status=$?"
1 file left unchanged.
status=0
$ $MDT --check clean.md 2>/dev/null; echo "status=$?"
status=0
```

A drifting file is reported and fails:

```console
$ $MDT --check ragged.md; echo "status=$?"
ragged.md +3 -3
1 file would be reformatted.
status=1
$ cmp ragged.md ragged.md.orig && echo unmodified
unmodified
```

An error is distinguishable from drift, and names the file that failed:

```console
$ $MDT --check missing.md; echo "status=$?"
reading missing.md

Caused by:
    No such file or directory (os error 2)
1 file could not be read.
status=2
```

In-place formatting of a drifting file still succeeds:

```console
$ cp ragged.md.orig ragged.md
$ $MDT --in-place ragged.md; echo "status=$?"
status=0
```

`--diff` prints a diff and fails on drift, matching `--check`, so one run
both shows the change and gates the build:

```console
$ cp ragged.md.orig ragged.md
$ $MDT --diff ragged.md; echo "status=$?"
--- ragged.md
+++ ragged.md
@@ -1,3 +1,3 @@
-|A|B|
-|---|---|
-|1|2|
+| A   | B   |
+| --- | --- |
+| 1   | 2   |
status=1
$ $MDT --diff clean.md; echo "status=$?"
status=0
$ cmp ragged.md ragged.md.orig && echo unmodified
unmodified
```

Modes are mutually exclusive:

```console
$ $MDT --check --in-place ragged.md; echo "status=$?"
error: the argument '--check' cannot be used with '--in-place'
status=2
```

CRLF and byte-order marks survive:

```console
$ printf '|A|B|\r\n|---|---|\r\n|1|2|\r\n' > windows.md
$ $MDT --in-place windows.md && file windows.md
windows.md: ASCII text, with CRLF line terminators
$ printf '\xef\xbb\xbf|A|B|\n|---|---|\n|1|2|\n' > bom.md
$ $MDT --check bom.md; echo "status=$?"
bom.md +3 -3
1 file would be reformatted.
status=1
```

Clean up `clean.md`, `ragged.md`, `ragged.md.orig`, `windows.md`, and
`bom.md` afterwards, or run the block in a scratch directory.

### Red, green, refactor evidence

Record in `Artefacts and notes`, per milestone: the red command and its
failure (the compiler error for tests referencing a flag that does not exist,
or the `todo!()` panic); the same command passing after the minimal
implementation; and the command sequence passing after cleanup. A test that
passes before the change is not a red test and must be strengthened.

### Verification evidence

For each obligation, record the command, the initial failure or
counterexample, the passing result, and each negative control's observed
rejection. An implementation change requiring a new invariant, lemma, or axiom
returns to `Verification plan` before continuing.

### Quality criteria

- Tests: `make test` passes with no warnings; that target already sets
  `RUSTFLAGS="-D warnings"`.
- Verification: every obligation in `Verification plan` is discharged with its
  stated evidence and non-vacuity check. `INV-IDEMPOTENT` and `INV-PREDICTS`
  are the two that must not be waived.
- Lint and typecheck: `make check-fmt`, `make typecheck`, and `make lint` pass.
- Documentation: `make markdownlint` and `make nixie` pass.
- Performance: `--check` over this repository's `docs/` tree completes in
  under two seconds on a warm cache, and computes no diff for unchanged files.
  No mode reads any file more than once; because `--diff` now gates on drift
  by itself, no continuous-integration usage requires a second invocation.
- Security: no widening of the filesystem capability; all access continues
  through `open_file_parent`, and the read-only path holds a capability with
  no write method.

### Quality method

```bash
make check-fmt && make typecheck && make lint && make test
```

## Idempotence and recovery

Every step is safe to repeat. `cargo add` is idempotent. Gate commands are
read-only apart from `target/` and `insta`'s pending snapshots. Snapshot
regeneration overwrites `.snap` files, so review `git diff` before staging and
use `git restore tests/snapshots/` to undo an unwanted regeneration.

Commit after each milestone so any milestone can be reverted independently.
The ordering means a later revert leaves a coherent state: reverting `EP-M4`
leaves `--check` working, and reverting `EP-M3` leaves the pure modules
unreferenced but correct. The one exception is `EP-M1`, recorded in its
milestone: reverting it does not un-rewrite files already processed under the
new serialization.

The manual validation commands create fixtures in the working directory; run
them in a scratch directory or delete them afterwards.

## Artefacts and notes

Populate during implementation with the `EP-M0` spike transcript, the red and
green transcripts per milestone, and the observed failure message from each
negative control. Keep each excerpt short and focused on what proves success.

### Rebase onto `origin/main`

Three pull requests merged while this plan was halted at `EP-M2`. The branch was
rebased onto `origin/main` before any further work:

| PR | Issue | Merged subject | Effect on this plan |
| --- | --- | --- | --- |
| #467 | #465 | `Write files atomically in --in-place mode` | lands the atomic `--in-place` write this plan had deferred; see `Decision log` |
| #469 | #451 | `Preserve the majority input line-ending style in formatter output` | supersedes the line-ending half of `EP-M1`, and writes `docs/adrs/0007-line-ending-detection.md` |
| #470 | #468 | `Make the formatter a fixed point in one pass` | discharges `INV-IDEMPOTENT`, unblocks `EP-M2` step 3, and writes `docs/adrs/0006-single-pass-idempotence.md` |

Conflicts were resolved in `Cargo.lock` (regenerated from the merged manifest),
`src/io.rs`, and `src/main.rs`, taking `main`'s line-ending implementation and
re-applying only this plan's byte-order-mark work on top. Twelve commits were
carried across the rebase; the BOM half of `EP-M1` is re-landed as
`src/io/document.rs`, and the working tree was clean before the `EP-M2` green
step began.

Three premises of this plan did not survive the rebase and have been corrected
in place:

- `EP-M1`'s module was specified as a top-level `src/document.rs`; it is now
  `src/io/document.rs`. See `Interfaces and dependencies`.
- `EP-M3`'s "version bump to `0.6.0`" had already landed, so the exit-status
  change ships under the existing unreleased `0.6.0` rather than raising it.
- `ADR 0006` and `ADR 0007` were already written by #470 and #469, so the
  check-and-diff record is now `ADR 0009` and the byte-order mark needs its own
  `ADR 0008`. See `Decision log`.

### Second rebase onto `origin/main` (#477)

One pull request merged after the first rebase above, and it is the one this
plan had been waiting for:

| PR | Issue | Merged subject | Effect on this plan |
| --- | --- | --- | --- |
| #477 | #474 | `Refuse Setext conversion of a table delimiter row (#474)` | fixes the `--headings` class recorded in `Artefacts and notes → EP-M6 baseline blocked`; clears `EP-M6`'s baseline precondition |

The initial tip was `2a73a58`; the branch was replayed onto `origin/main` at
`408c76a`, twenty-six commits, and **no file conflicted in either attempt**.
`Cargo.toml` and `Cargo.lock` were untouched by the merge: `408c76a` changes
neither, and the branch's own manifest changes were already in place, so there
was no lock file to rebuild and nothing to take from `main` on that account.

#### The first attempt rebased cleanly and was still wrong

`git rebase origin/main` reported `Successfully rebased and updated
refs/heads/check-option.`, with no conflict and nothing on stderr but
`weave: N entities auto-resolved (<confidence>)` notices, at `high`,
`very_high`, and once `conflict` confidence: the Weave merge driver, selected
for Markdown and Rust paths by the global attributes file, had auto-resolved the
documentation. Comparing the result against both parents showed what
"auto-resolved" had cost:

- `docs/architecture.md`: the footnotes example had its `Before:` lines
  rewritten from `1. First note` to `[^1]: First note` — the *After* form — and
  lost the `After:` label, its blank line, the opening fence, and the `Text.`
  line beneath it: seven lines replaced by five, so the worked example no longer
  demonstrates the transform it documents.
- Fourteen stray blank lines were inserted across the same three files: one in
  `docs/architecture.md`, five in `docs/developers-guide.md`, eight in
  `docs/users-guide.md`.

Both defects were found by comparing blobs, not by any driver message:
`git show origin/main:<path>`, `git show 2a73a58:<path>`, and the working file,
for each of the three documents. The evidence was then preserved in
`/tmp/weave-corrupt/` — copies of the three merged files, the rebase log, and
both diagnostic diffs (against the pre-rebase tip and against `origin/main`) —
**before** any command that would recreate or discard it.

Recovery followed the skill's procedure for a global Weave setup. The branch was
reset to `2a73a58`; `git -c core.attributesFile=/dev/null check-attr merge --
<path>` was measured to report `merge: unspecified` for a representative path,
confirming that the selection comes from the global attributes file and that
disabling it for the command leaves Git's built-in merge machinery in charge;
then the whole operation was re-run with the same override:

```plaintext
$ git -c core.attributesFile=/dev/null rebase origin/main
Successfully rebased and updated refs/heads/check-option.
```

The re-run resolved nothing silently and produced no conflict markers. The
difference between the rebased tip and `2a73a58` is exactly `408c76a`'s own
stat — 17 files, 1010 insertions, 551 deletions, the same 17 paths — which is
the check that the replay is the branch plus `main` and nothing else. The
footnotes example is byte-identical to `origin/main`'s, and the blank-line
insertions are gone.

The lesson generalises past this repository: a clean merge-driver exit means no
recorded conflict remains, not that the result has the intended semantics. The
default assumption for a rebase here is the built-in machinery, and a Weave
result is worth accepting only after a byte-level comparison against both
parents — which is the cheap thing to do and was not done on the first attempt
until afterwards.

#### Integrity checks after the re-run

- The branch-versus-`main` file set is unchanged by the rebase: 73 files before
  and after (`git diff --name-only origin/main...HEAD`).
- Fourteen of `main`'s seventeen changed paths are byte-identical to
  `origin/main`'s copies: `src/headings.rs`, `src/headings_tests.rs`,
  `tests/idempotence.rs`, `tests/idempotence_adjacencies.rs`,
  `tests/idempotence_drift.rs`, `tests/idempotence_properties.rs`,
  `tests/support/idempotence_harness.rs`, and the six
  `tests/data/idempotence/T*.dat` fixtures. The other three are the documents
  this branch also edits, so they differ by the branch's own additions and
  nothing else — every line `main` has there and the tree does not is one of the
  branch's own rewrites (`format_to_string` and `rewrite_in_place`, which `EP-M3`
  deleted, and the snapshot-portability sentence `EP-M5` corrected), checked one
  by one rather than assumed.
- Both sides' documentation edits are present: the branch's `--check`/`--diff`
  sections and `main`'s rewritten idempotence material coexist, the architecture
  footnotes example is byte-identical to `origin/main`'s (same MD5 over the
  block), and no conflict marker appears anywhere in the tree.
- The branch's intermediate commit `7b59154` (`EP-M7`) is intact, so the replay
  is of the branch's real history rather than a squashed approximation.

### EP-M0 spike transcript

`cargo test --test spike -- --nocapture`, temporary `tests/spike.rs`:

```plaintext
=== two-line ragged table, trailing newline ===
--- ragged.md
+++ ragged.md
@@ -1,3 +1,3 @@
-|A|B|
-|---|---|
-|1|2|
+| A | B |
+| --- | --- |
+| 1 | 2 |
=== unterminated original ===
--- ragged.md
+++ ragged.md
@@ -1,3 +1,3 @@
-|A|B|
-|---|---|
-|1|2|
\ No newline at end of file
+| A | B |
+| --- | --- |
+| 1 | 2 |
=== patience algorithm, one changed line in twenty ===
insertions=1 deletions=1
@@ -7,7 +7,7 @@
 line 7
 line 8
 line 9
-line 10
+line ten
 line 11
 line 12
 line 13
```

Findings: the default (`Myers`) and `Algorithm::Patience` renderings are
identical in shape; the missing-newline marker appears once, after the final
`-` line and before the first `+` line, confirming it can only ever appear on
the `-` side; `context_radius(3)` is the default. A lone `\r` tokenizes as a
separator — `TextDiff::from_lines("a\rb\n", …).old_slices()` is
`["a\r", "b\n"]` while `str::lines()` yields `["a\rb"]` — which confirms `AX-1`
and the `AX-5` divergence empirically.

### EP-M0 canary transcript

`cargo test --test bdd_reporting`, temporary canary scenario:

```plaintext
running 1 test
test canary ... ok
```

Deleting the `#[then]` step definition produced, from the same command:

```plaintext
error: No matching step definition found for 'Then the canary sings'
 --> tests/bdd_reporting.rs:9:1
  |
9 | #[scenario(path = "tests/features/canary.feature")]
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: this error originates in the attribute macro `scenario`
```

so `strict-compile-time-validation` is active. No fallback to plain `rstest`
is needed. `cargo tree --duplicates | grep similar` prints nothing, so
`similar` is a single copy in the graph.

### CodeRabbit review after `EP-M0`

Run by `scrutineer` as `coderabbit review --agent --committed`, after all six
deterministic gates passed. Full log:
`/tmp/coderabbit-mdtablefix-check-option.out`.

```json
{"type":"complete","status":"review_completed","findings":0,
 "reviewedFiles":["Cargo.lock","Cargo.toml","docs/execplans/check-option.md"]}
```

Zero findings, no rate limit, no seat-hour exhaustion. `--committed` was used
so that in-flight `EP-M1` edits stayed out of the review. Nothing to clear;
`EP-M1` may proceed.

### EP-M1 pre-refactor oracle

Eleven fixtures under `tests/data/document/`, driven by
`tests/document_properties.rs`: twelve `--in-place` cases plus one stdin case.
Every case asserts the exact bytes left on disk, captured from the
pre-refactor binary, so the serialization rewrite has a byte-exact oracle
rather than a hand-written expectation. `.dat` is deliberate: `make fmt`
formats only `.md`, `.markdown`, and `.mdx`, so these bytes survive the
formatter. `cargo test --test document_properties` reports 12 passed.

Escapes are Rust string escapes; `\u{FEFF}` is the byte-order mark.

| Fixture | Pre-refactor output | Flips in step 4 |
| --- | --- | --- |
| `crlf_ragged` | `\| A   \| B   \|\n\| --- \| --- \|\n\| 1   \| 2   \|\n` | yes, endings become CRLF |
| `crlf_clean` | same as `crlf_ragged` | yes, endings become CRLF |
| `mixed_lf_majority` | same as `crlf_ragged` | no |
| `mixed_crlf_majority` | same as `crlf_ragged` | yes, endings become CRLF |
| `mixed_tie` | `alpha\nbeta\n` | no |
| `mixed_in_fence` | table, blank line, then a `sh` fence holding `echo hi`, all LF | yes, every line becomes CRLF |
| `bom_ragged` | `\u{FEFF}\|A\|B\|\n\| 1   \| 2   \|\n\| --- \| --- \|\n` | yes, mark kept, table reflowed |
| `bom_crlf_clean` | `\u{FEFF}\| A \| B \|\n\| 1   \| 2   \|\n\| --- \| --- \|\n` | yes, mark kept, endings CRLF |
| `lone_cr` | `alpha\rbeta\n` | no |
| `empty` | empty | no |
| `no_trailing_newline` | table plus a trailing line feed | no |
| stdin, CRLF ragged | same as `crlf_ragged` | yes, endings become CRLF |

Two pre-refactor oddities are pinned deliberately. The first disappears in
step 4: the byte-order mark defeats table detection, so the first line
survives verbatim while the separator and data lines are reflowed — and the
data line is emitted *before* the separator. The second remains: a lone `\r`
is not a line ending to `str::lines`, so `alpha\rbeta` stays a single line and
the `\r` survives inside it. Only the byte-order-mark defect is in `EP-M1`'s
scope; the lone-`\r` limitation is the remaining gap recorded in
`Rigour and residual gaps` and must be documented in `EP-M7`.

### EP-M1 red and green transcripts

Red, from `cargo test --lib document` with `detect`, `parse`, and
`render_lines` stubbed as `todo!()` (full log:
`/tmp/red-document-mdtablefix-check-option.out`):

```plaintext
thread 'document::tests::render_lines_uses_the_document_ending' panicked at
src/document.rs:91:9:
not yet implemented: EP-M1 green step

test result: FAILED. 3 passed; 20 failed; 0 ignored; 0 measured; 720 filtered out
```

Green, from the same command after implementing the three bodies:

```plaintext
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 720 filtered out
```

Those 23 are not all `document` cases: the filter also matches two `ellipsis`
tests whose names contain "document"
(`grep -cE '^test document::tests::'` on the log gives 21, which is the count
of `document::tests::` cases). `src/document.rs` is 242 lines.

`as_str` stays implemented in the red state: `todo!()` is not callable in a
`const fn`, and the function is a two-arm match with no logic worth red-ing.
`cargo test --doc document` runs the four new doctests, all passing — but
`make test` does not run them, as `Surprises and discoveries` records.

### EP-M1 step 4: post-refactor oracle

`src/main.rs` gained `format_content`, which parses through
`SourceDocument`, formats `document.lines()`, and renders through
`render_lines`; both `format_to_string` and the stdin branch call it. The
stdin branch prints a bare newline for empty output, keeping
`tests/parallel.rs::test_cli_parallel_empty_file_list`'s `stdout("\n")`
contract. `src/io.rs`'s `rewrite_with` performs the same three steps over
`fs::read_to_string` and `fs::write`. No public signature changed.

Six expectations flipped, each because an ending or the mark is now preserved,
never because a content transform changed:

| Case | New expectation | Why |
| --- | --- | --- |
| `crlf_ragged` | `TABLE` with CRLF | CRLF majority |
| `crlf_clean` | `TABLE` with CRLF | CRLF majority |
| `mixed_crlf_majority` | `TABLE` with CRLF | CRLF majority |
| `mixed_in_fence` | table, blank line, `sh` fence, all CRLF | CRLF majority |
| `bom_ragged` | `TABLE` with LF, mark restored | mark split off, table now detected |
| `bom_crlf_clean` | `TABLE` with CRLF, mark restored | mark split off, ending kept |

The stdin case was renamed `stdin_preserves_line_endings` and now expects
CRLF. `mixed_lf_majority`, `mixed_tie`, and `no_trailing_newline` keep LF;
`empty` still yields nothing; `lone_cr` still yields `alpha\rbeta\n`, because
a lone `\r` is not a line ending to `str::lines()`.

Focused runs after the refactor: `cargo test --test document_properties`
reports 12 passed, and `cargo test --lib -- document:: io::` reports 28 passed.

Gates: `check-fmt`, `typecheck`, `lint`, `test`, `markdownlint`, and `nixie`
all pass. `make test` reports 1470 passed, 0 failed, 20 ignored across 34
suites, including the 28 doctests the widened recipe now gates.

EP-M1 CodeRabbit review: `coderabbit review --agent --committed` completed in
166s with exit code 0 and zero findings across 21 files. Log:
`/tmp/coderabbit-m1-mdtablefix-check-option.out`.

### EP-M2 idempotence failure

Red state observed before any green work, exactly as step 1 requires.
`cargo test --test check_properties count` fails with
`not yet implemented: EP-M2 green step` from `src/report/delta.rs:40`;
`cargo test --lib report` reports 1 passed, 33 failed. The deterministic
`formatting_is_idempotent` cases (10 flag combinations × 14 corpus documents)
pass, so the failure came from the generated-document property, which is the
point of having a generator.

Class A transcript, minimal original input `---\nprose words here`:

```text
$ printf -- '---\nprose words here' | mdtablefix --wrap --breaks
______________________________________________________________________
prose words here
$ printf -- '---\nprose words here' | mdtablefix --wrap --breaks | mdtablefix --wrap --breaks
______________________________________________________________________ prose
words here
```

Class A root cause probe, `--wrap` alone:

```text
alpha ______________________________________________________________________  <- 70 underscores folded
alpha|---|beta                                                               <- `---` untouched
alpha *** beta                                                               <- `***` folded
alpha ___ beta                                                               <- `___` folded
```

Class B transcript, minimal input (83 chars) under `--wrap` alone:

```text
input:  - *Ownership.** Owned by the wrap module (`src/wrap/tracing_snapshot_support.rs`)\nn
pass 1: - *Ownership.** Owned by the wrap module\n  (`src/wrap/tracing_snapshot_support.rs`)\nn\n
pass 2: - *Ownership.** Owned by the wrap module\n  (`src/wrap/tracing_snapshot_support.rs`) n\n
```

Repository-wide scan, two in-place passes per file, comparing bytes between
pass 1 and pass 2:

| Corpus | `--wrap` alone | `mdformat-all` flag set |
| --- | --- | --- |
| `tests/data/` (123 files) | 0 not fixed points | 1 (`cli-matrix/frontmatter-breaks.dat`) |
| `*.md` in `HEAD` (28 files) | 1 (`docs/developers-guide.md`) | 1 (`docs/developers-guide.md`) |

`frontmatter-breaks.dat` pass 1 → pass 2, showing the Setext underline
normalised by `--breaks` then absorbed by the next wrap:

```diff
-Heading
-______________________________________________________________________
+Heading ______________________________________________________________________
```

Convergence: every case above reaches a fixed point on pass 3. The defects
cost an extra pass, they do not loop.

Escalated rather than worked around, per `Tolerances`. Work on `EP-M2` green
steps is suspended pending the decision recorded in `Decision log`.

Red state preserved on the scratch branch `wip/ep-m2-red-state` (commit
`51f6701`), which carries `src/report*`, `tests/check_properties.rs`, and the
eight `tests/data/numstat/` fixtures with their `todo!()` bodies intact. That
branch is a red-state checkpoint and must not be merged as-is; resume by
continuing from it or by re-creating the files, which `Concrete steps →
EP-M2` fully specifies.

Raised as GitHub issue #468 on explicit instruction from `@leynos`. The issue
carries the self-checking corpus script verbatim, the observed pass-1 and
pass-2 bytes for all eight cases, the blast-radius table, and the root-cause
analysis above. Its acceptance criteria require both halves of the evidence
`@leynos` asked for: the corpus landed as `tests/data/idempotence/*.dat` with a
test that formats each fixture twice through the real binary, **and** a
generated-document property test over the sampled eight-flag powerset with
non-vacuity. Satisfying the eight cases alone does not close the issue, and
neither does a property test that never reaches the failing shapes.

### EP-M2 green transcripts

Red, from `cargo test --test check_properties` on the rebased tree with the four
bodies still stubbed as `todo!()`:

```plaintext
test result: FAILED. 11 passed; 9 failed; 0 ignored; 0 measured; 0 filtered out
```

All ten `formatting_is_idempotent` cases and
`generated_documents_reach_a_fixed_point` already passed in that red state.
That is what proved `INV-IDEMPOTENT` was discharged by #470 *before* any green
code was written — the nine failures were the `todo!()` panics and nothing
else. The milestone's `Tolerances` gate on idempotence therefore did not fire.

Green, from the milestone's own acceptance commands:

```plaintext
$ cargo test --lib report
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 886 filtered out

$ cargo test --test check_properties
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --doc report
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 50 filtered out
```

`similar` is a direct runtime dependency and `cargo tree --duplicates` reports
no second copy, satisfying the milestone's conformance check.

**Negative controls.** Each was applied as a temporary local mutation to
`src/report/delta.rs`, run, and reverted; `cmp` confirmed the file restored
byte-identical each time. Logs are
`/tmp/test-mdtablefix-check-option.control<N>.out`. Line numbers quoted below
are those of the tree when each control ran; a later rustfmt pass re-wrapped
one `CORPUS` entry and shifted them by three.

- **Control 1.** Count `Equal` as `Insert`
  (`ChangeTag::Equal => delta.insertions += 1`). Rejected: 6 of 20 cases
  failed, including the long-file golden fixture.

```plaintext
left: `2`,  right: `4`: tokens are conserved: after = before + insertions - deletions at tests/check_properties.rs:131.
left: (20, 1)  right: (1, 1)
```

- **Control 2.** Return whole-file line counts instead of a diff. Rejected: 6
  of 20 cases failed, the long-file fixture reporting `+20 -20` where the
  golden expects `+1 -1`.

```plaintext
left: `true`,  right: `false`: the delta is non-zero exactly when the bytes differ at tests/check_properties.rs:136.
left: (20, 20)  right: (1, 1)
```

- **Control 3.** Compute the delta from `str::lines()`-split text. Rejected: 5
  of 20 cases failed, including both INV-AGREE cases the obligation names.

```plaintext
failures:
    agree_reports_drift_exactly_when_bytes_differ::case_2_line_endings_only
    agree_reports_drift_exactly_when_bytes_differ::case_3_trailing_newline_only
```

Control 1 exposed a non-vacuity hole in the red state. Because
`count_conserves_tokens_and_agrees_with_byte_equality` drew `original` and
`formatted` from two independent `any::<String>()` generators, the pair
essentially never shared a line, and both assertions then hold for *any* diff
whatsoever: conservation follows from the pairing the diff performs, and
disagreement from the two strings being distinct. The property test caught none
of the three controls. `tests/check_properties.rs` now samples related pairs as
well — one document with a single line-level edit applied to it, which may be
byte-equal or differ only in line endings or only in the final terminator — and
all three controls now fail the property test as well as the golden fixtures.
This is the plan's own `LEM-COUNT` reasoning, that "the conservation law alone
is **not** falsifiable", turned on the generator rather than only on the
fixtures.

Idempotence artefacts were rescoped rather than duplicated.
`tests/idempotence.rs` and `tests/idempotence_properties.rs` arrived with #470
and are now the general suites, sampling document *structure*. The idempotence
cases in `tests/check_properties.rs` are narrowed to the document *boundary* —
ending style, byte-order mark, and trailing terminator — which those suites do
not generate and which is the dimension this plan changed. Both files say so in
their own documentation. No snapshot churn was introduced beyond the single new
`summary_grammar` snapshot, well inside the usual limit of 30.

**Close-out.** All six gates pass on the committed tree, with each gate's log
under `/tmp/<gate>-mdtablefix-check-option.out`, and `coderabbit review
--agent --base main` returned `findings: 0` (logged to
`/tmp/coderabbit-mdtablefix-check-option.out`). The accepted commits are
`91692d6` and `8bcf3eb`.

The branch was rebased before this milestone, so publishing it required
`--force-with-lease`: `origin/check-option` moved from `377e158` to `8bcf3eb`.
The twelve remote-only commits were the pre-rebase duplicates of commits that
survive locally, matched one for one by subject before the push.

### EP-M3 red and green transcripts

Red, before any `--check` existed: `cargo test --test cli_check` reported
`2 passed; 2 failed` (log
`/tmp/red-cli_check-mdtablefix-check-option.out`) and
`cargo test --test bdd_reporting` reported `1 passed; 7 failed` (log
`/tmp/red-bdd_reporting-mdtablefix-check-option.out`), every failure sharing one
cause:

```plaintext
error: unexpected argument '--check' found

  tip: to pass '--check' as a value, use '-- --check'

Usage: mdtablefix [OPTIONS] [FILES]...
```

**The red state was a run-time failure, not a compile-time one.** `EP-M3` step 1
predicted that the tests would not compile because `--check` does not exist; in
fact the binary still builds, `clap` rejects the unknown flag while parsing, and
the process exits `2`. Two consequences followed. First, a red test whose
failure is "the flag was rejected" proves much less than one whose failure is
"the flag was accepted and the behaviour was wrong", so `EP-M3`'s assertions
were checked for non-vacuity rather than taken on trust. Second, the one
scenario that passed — `--in-place` over a drifting file still exits `0` — is
the only one whose command line `clap` accepted *and* whose assertions held: it
rereads the file and requires those bytes to equal the fixture, so its passing
is what confirmed the corrected fixture below is the formatter's true output.

**One test passed for the wrong reason.** `directory_snapshot_unchanged`
asserted only that an unreadable file exits `2`, and `clap`'s usage error also
exits `2`, so it was satisfied by a run that never opened a file. It now also
requires standard error to name `missing.md`, which a usage error cannot do,
because `clap` never learns the file names. The companion test in the same file
asserts that the snapshot helper *does* detect a write, so the read-only
assertion is not vacuous in the other direction either.

**`| A | B |` is not a fixed point.** The first draft of the `CLEAN` fixture was
a hand-written approximation, which the `--in-place` scenario rejected with
`left: "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n"`. The formatter pads
every cell to the delimiter row's width, so `| A | B |` is itself drift. Both
test files now hold the formatter's own bytes, and `tests/line_endings.rs` holds
the same fixture. The plan's own `Validation and acceptance` block carried the
same error and has been corrected, as has its `--diff` transcript, whose body is
the padded output.

Green, from the milestone's acceptance commands:

```plaintext
$ cargo check --all-targets --all-features
(no output, no warnings)

$ cargo test --bin mdtablefix
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --test cli_check
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --test bdd_reporting
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The manual validation block was replayed at this point and matches, including
the `+3 -3` report for a ragged table, `unmodified` after `cmp`, `status=2` for
an unreadable file, and `status=0` for `--in-place` over that same drifting
file.

**Design decisions taken in the green step.**

- `exit_status` and the driver, not the parser, decide the status. `clap` still
  exits `2` on a rejected command line, which is the same code as
  `ExitStatus::Error`, so the two are indistinguishable to a caller and need no
  reconciling.
- The summary goes to standard error, and only in the reporting modes. A bare
  or `--in-place` run keeps its historical output byte for byte, so no existing
  suite changes; standard output stays a machine contract.
- A closed pipe is `ExitStatus::Success`. Rust ignores `SIGPIPE`, so the write
  returns `EPIPE`; `print!` would panic into the undocumented status `101`.
  Standard output is therefore written through an explicit handle, and every
  write error is inspected for `ErrorKind::BrokenPipe` before it is treated as
  a failure. `tests/cli_check.rs` pins this with the plan's own
  `--check *.md | head` shape, sized so that the child cannot finish writing:
  every file drifts, so `0` is reachable only through the early exit.
- Errors are printed once. The previous code printed the chain in
  `report_results` and then returned the error, which `main`'s `Termination`
  printed a second time as `Error: …`; the run now owns the printing and
  returns a status.
- An unreadable file moved from exit `1` to exit `2`, which is the change
  `ADR 0009` records.
- The per-file analysis returns `(FileReport, String)`, and drops each
  `Assessment` before returning, so retained memory is proportional to the
  rendered payload rather than to twice the whole input.
- `format_to_string` and `rewrite_in_place` became test-local helpers in
  `src/main_tests.rs`, built on `driver::analyse` with a fixed mode. They are
  one-line adapters, not a second implementation, and keeping them out of
  `src/main.rs` avoids dead code in the binary.
- `report_line_endings` moved from `src/main.rs` to `src/driver.rs`, because the
  driver is now what parses a document and therefore what selects an ending.
  Its message and fields are unchanged.
- The spec block's `assess` documentation mentioned a `display_path` parameter
  its signature does not take; the signature is authoritative, and the block has
  been corrected to say the caller attaches the display path.

**The first gate run was red, and for two avoidable reasons.** `make
check-fmt` and `make lint` failed on the new files while `typecheck`, `test`,
`markdownlint`, and `nixie` passed. Neither failure was a behavioural defect,
and both are recorded here because the milestone's steps put the gates at the
end, whereas what they checked was code written earlier in the same session:

- `cargo fmt` rewrapped four call chains in `src/driver_tests.rs` and reordered
  the import block in `src/main_tests.rs`. The formatter's own output was
  applied, and it touched only those two files.
- Three Clippy lints fired in `tests/cli_check.rs`, all in test fixtures rather
  than in the code under test: `format_push_string` and `format_collect` in the
  batch-file writer and its expected-string builder, and
  `bool_to_int_with_if` in the exit-status oracle. The first two became
  `str::repeat` and an explicit loop appending to a `String`; the third became
  `i32::from(files != Files::Clean && self == Self::Check)`, which states the
  drift condition positively instead of negating a disjunction.

The re-run was green on all six gates — 42 test suites, 1795 passed, 0 failed,
20 ignored — with the working tree byte-identical across the run.

### CodeRabbit review after `EP-M3`

Requested through the gate runner only once the deterministic suite was green,
so that the review was not asked to catch anything the gates could have caught
first. It ran against the pushed commit `6c9dd90` on `origin/check-option`,
reviewing the whole branch diff — 43 files, from `EP-M0` onward, not just this
milestone's — and reported no rate limit and no refusal:

```plaintext
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":[…]}
CODERABBIT_EXIT=0
```

Zero findings, so nothing carried into `EP-M4`. The full JSON-lines log is
`/tmp/coderabbit-mdtablefix-check-option.out`.

### EP-M4 red and green transcripts

Red, before `--diff` existed: `cargo test --test cli_diff` reported `0 passed;
4 failed` (log `/tmp/red-cli_diff-mdtablefix-check-option.out`),
`cargo test --test bdd_reporting` reported `9 passed; 5 failed`, and
`cargo test --test cli_check` reported `4 passed; 1 failed`. Every failure shared
one cause, and it was again a run-time rejection rather than a compile error:

```plaintext
error: unexpected argument '--diff' found

  tip: to pass '--diff' as a value, use '-- --diff'

Usage: mdtablefix [OPTIONS] [FILES]...

assertion left == right failed: a drifting file must exit 1 on every run,
attempt 0: error: unexpected argument '--diff' found
```

The `cli_check` failure names the mode and the shape it was checking, which is
what widening that matrix was for:

```plaintext
all_clean under Diff (with_error: false) exited 2,
stderr: error: unexpected argument '--diff' found
```

**`String` does not implement `io::Write`.** `write_unified_diff` streams into
`&mut impl io::Write`, and the first draft of the driver passed a `String`,
because `write!`-style rendering to a string feels as though it should work. It
does not: `String` implements `fmt::Write` and nothing else, so the type error
arrives from a direction that reads like a missing import. A standalone
`rustc --edition 2024 --crate-type lib --emit=metadata` snippet settled it before
the driver was changed. `render_diff` now renders into a `Vec<u8>` and validates
with `String::from_utf8`, which is also where the fallible step belongs: both
sides came from `String`s, so validation cannot fail in practice, but the driver
does not get to assume it.

**A determinism scenario that would have passed in the red state.** The spec's
`Diff output is byte-identical across repeated runs` compares ten runs against
each other, and ten rejections of an unknown flag are ten *identical empty*
outputs, so the scenario was satisfied before any implementation existed. Two
changes remove that vacuity: the scenario now also asserts that the diff contains
a hunk header, and the step `every run produced identical standard output`
requires the output to be non-empty on its own. An empty repeated output now
fails in both places.

Green, from the milestone's acceptance commands, after the negative control below
had been applied and removed:

```plaintext
$ cargo test --test cli_diff
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --test cli_check
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --test bdd_reporting
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --bin mdtablefix
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Log: `/tmp/green-ep-m4-mdtablefix-check-option.out`.

**The first full gate run was red, and only on formatting.** `make check-fmt`
found two rustfmt diffs — the six-line signature of
`unreadable_file_yields_the_error_status_in_diff_mode` in
`tests/bdd_reporting.rs`, which `rustfmt` collapses onto one line, and the
`assert_eq!` in the above-threshold determinism test, whose long message makes
`rustfmt` put the expected `0` on its own line. `cargo fmt --all` was applied,
it touched only those two files, and the second gate run was green on all six:
43 suites, 1812 passed, 0 failed, 20 ignored, with `git status --short`
byte-identical before and after. `make fmt` was deliberately not used, because
it also runs `mdformat-all` over the whole repository. As in `EP-M3`, both
diffs are formatting rather than behaviour, and both were in test code written
earlier in the same session while the gates were held to the end.

### `INV-DETERMINISTIC` negative control

The plan's non-vacuity requirement for `INV-DETERMINISTIC` is: "negative control
is enabling `TextDiffConfig::timeout`, which must make the test flaky or fail."
The control was applied, and the outcome is reported in two parts, because the
answer is not the one the plan predicted.

The mechanism is reachable. In `similar` 2.7.0 the deadline is checked once per
iteration of `myers::find_middle_snake` (`src/algorithms/myers.rs:178`) and once
per row of `lcs::make_table` (`src/algorithms/lcs.rs:169`), against
`Instant::now() > deadline`. When it trips, `find_middle_snake` returns `None`
and `conquer` takes its `else` branch (`myers.rs:314`), deleting the entire old
range and inserting the entire new range. The failure mode is therefore a
*wholesale replacement* of whatever region the search was working on, not a
partial or approximate answer.

**The hazard is real, and it is a budget-dependence rather than a per-run one on
the plateau.** With the renderer's timeout driven from the environment, a
1601-line corpus holding 400 scattered ragged tables — so that no common prefix
or suffix trimming can reduce the problem — renders two different files for the
same input and flags:

```plaintext
budget     1ns: 3202 lines of output (hash c4b9e382)
budget     1s: 2803 lines of output (hash 85e55838, equal to unbounded)
```

Sweeping the budget over ten runs at each value gives a step function with wide
plateaus, which is what `conquer`'s `else` branch implies: any trip inside one
node's search replaces that node's whole range, so the exact iteration that
tripped does not matter, only which node it tripped in.

```plaintext
budget  10µs–100ms     1 distinct output in 6 runs
budget  140ms          5 distinct outputs in 10 runs
budget  150ms          3           "
budget  160ms          4           "
budget  170ms          1           "
budget  180ms          2           "
budget  190ms          6           "
budget  200ms          5           "
budget  220ms          1           "
budget  240ms–unbounded 1          "
```

So the plan's prediction holds in the transition band, where the deadline lands
mid-computation: ten invocations of one command produce up to six different
answers, and the determinism test would fail there. On the plateaus the same
mutation is stable but *speed-dependent*, which is the other half of the hazard
and the one no single-machine repetition can see: a budget that is fast for one
machine is slow for another.

**The first version of the above-threshold test could not see the hazard at
all.** It used 1200 prose lines followed by one ragged table, and with a 1ns
budget it produced byte-identical output to the unbounded run. The reason is
trimming: the common prefix of 1200 lines is removed before any search, leaving
a three-line range whose wholesale replacement is indistinguishable from its
diff. The test was exercising the threshold's *algorithm selection* and nothing
else. It now uses four ragged tables spread across 1372 lines — above the
threshold, and with changes far enough apart that trimming has nothing to
remove — and asserts that no untouched prose line comes back as deleted:

```plaintext
$ cargo test --test cli_diff diff_is_deterministic_above  (budget 1ns)
the diff must localise the change to the tables; 1020 of the 1360 untouched
prose lines came back as deleted, which is a wholesale replacement rather than a
diff
test result: FAILED. 0 passed; 1 failed

$ cargo test --test cli_diff                            (budget unset)
test result: ok. 5 passed; 0 failed
```

Same mutated binary in both runs, so the red is attributable to the wall-clock
budget and not to the fixture or the assertion. Logs:
`/tmp/control-timeout-cli_diff-mdtablefix-check-option.out` and the sweep above.
The mutation was then removed with `git checkout -- src/report/render.rs`, and
`rg 'timeout|Instant|Duration|deadline' src/report/render.rs` finds nothing: the
algorithm is chosen by `line_count` and the file consults no clock.

**Residual gap.** The ten-run method detects nondeterminism only when a budget
lands mid-computation. A budget never crossed is invisible to it, and nothing in
the suite forbids one structurally, so a future `.timeout(500ms)` would pass
every test here while making output depend on machine speed. The invariant
currently rests on `write_unified_diff` reading `line_count` and nothing else.
Closing that gap means either a corpus large enough to cross a plausible budget
— which costs seconds per test — or a source-level assertion that the diff path
names no clock. Neither is in `EP-M4`'s scope; both are recorded here rather than
silently dropped.

### CodeRabbit review after `EP-M4`

Requested through the gate runner only once all six deterministic gates were
green on this exact tree — 43 suites, 1812 passed, 0 failed, 20 ignored — so the
review was not asked to find anything the gates could have caught first. It ran
against the pushed commit `cf8995d` on `origin/check-option`, reviewing the whole
branch diff rather than this milestone alone: 44 files, from `EP-M0` onward. It
reported no rate limit, no refusal, and no findings:

```plaintext
{"type":"review_context","reviewType":"all","currentBranch":"check-option","baseBranch":"main",…}
{"type":"status","phase":"analyzing","status":"reviewing"}
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":[…44 paths…]}
CODERABBIT_EXIT=0
```

The runner verified that the review's `reviewedFiles` list is set-equal to
`git diff --name-only origin/main...HEAD`, so the zero is a completed review of
the full change surface rather than a cache hit or a partial diff. Nothing
carried into `EP-M5`. Two caveats are worth keeping with the result: the review
completed in about 32 seconds over a roughly 3,400-line diff, so "no findings"
means the contracted reviewer raised nothing rather than that the diff was
exhaustively audited; and the same reviewer had already returned zero findings
for `EP-M3`'s commit, so most of what it saw had passed once before. The full
JSON-lines log is `/tmp/coderabbit-mdtablefix-check-option.out`.

### EP-M5 red and green transcripts

Red — the first `INSTA_UPDATE=always cargo test --test cli_matrix
cli_matrix_snapshots` run did not get as far as writing the new snapshots. It
panicked inside the new diff invariant:

```plaintext
thread 'cli_matrix_snapshots' panicked at tests/cli_matrix/reporting.rs:167:5:
assertion `left == right` failed: row_000_nowrap_diff: a diff must be a diff of
the printed document, not merely a non-empty one
  left: "| Name  | Notes…|\n\n1. first item\n3. second item …\n"
 right: "| Name  | Notes…|\n\n1. first item\n3. second item …\n\nTitle\n=====\n"
```

The left side was the reconstructed right-hand side of the payload; the right
side was the document the printing mode had produced. The payload for that case
is `@@ -1,4 +1,6 @@` against a seven-line file, whose last change is on line 1,
so its three lines of trailing context end at line 4 and lines 5-7 are not in
the payload at all — the diff was right and the reconstruction was wrong. (The
fixture is `tests/data/cli-matrix/table-prose.dat`, seven lines by `wc -l`:
table row, blank, `1. first item`, the long item, blank, `Title`, `=====`.) The
invariant was replaced by a patch applier rather than weakened, and two unit
tests pin the "lines between hunks" and "lines after the last hunk" cases
directly.

Green — the harness, after regeneration:

```plaintext
$ cargo test --test cli_matrix
test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Snapshot churn, measured against a baseline captured before the milestone
(`md5sum tests/snapshots/*.snap`, 88 entries) rather than eyeballed: twelve
files added, none modified. The twelve are `row_000`, `row_010`, and `row_111`
in both wrap variants and both reporting modes. What they pin, in order:

- `row_000_nowrap`: `input.dat +3 -1`; the diff opens `@@ -1,4 +1,6 @@` and
  shows the collapsed table row replaced by three rows, with the document's tail
  deliberately outside the hunk. This is the case that caught the reconstruction
  bug, so it is now the load-bearing one.
- `row_000_wrap`: `input.dat +6 -4`; the same table plus a re-wrapped list item
  and a Setext heading folded to `Title =====`.
- `row_010_nowrap`: **silent**, exit `0`, stderr `1 file left unchanged.` — the
  no-drift branch, which is why this row is in the subset at all.
- `row_010_wrap`: `input.dat +2 -1`; only the wrapped paragraph drifts.
- `row_111_{nowrap,wrap}`: `input.dat +3 -4`; the diff's first hunk line is a
  context line for `title: Matrix` inside the frontmatter, and the frontmatter's
  own `---` stays a context line while the document-level `---` becomes the
  normalized thematic break. That is the boundary the reporting modes have to
  respect, recorded in both renderings.

The subset's selection rule was re-measured as a script rather than remembered:
each base row's fixture was written to one file and the binary run over it with
that row's flags, with and without `--wrap`, and the output compared with the
input. Exactly one of the sixteen combinations is a fixed point.

```plaintext
row_000 nowrap drifts
row_000 wrap   drifts
row_001 nowrap drifts
row_001 wrap   drifts
row_010 nowrap FIXED POINT (clean, exit 0)
row_010 wrap   drifts
row_011 nowrap drifts
row_011 wrap   drifts
row_100 nowrap drifts
row_100 wrap   drifts
row_101 nowrap drifts
row_101 wrap   drifts
row_110 nowrap drifts
row_110 wrap   drifts
row_111 nowrap drifts
row_111 wrap   drifts
```

So the no-drift branch is not a hope that some row happens to be clean: it is
`row_010_nowrap`, the one combination the formatter leaves alone, and the
`clean > 0` assertion fails the moment that stops being true.

Every verdict above was cross-checked between the two modes by
`matrix_reporting_modes_agree`, which requires `--check`'s counts and `--diff`'s
marked lines to be equal for all six curated cases, so the transcripts cannot
drift apart without failing. Two further properties are measured rather than
written down: the file's bytes after a reporting run are compared against the
fixture, and the expected exit status is computed from `printed != source`. A
report that always fired would fail the `clean > 0` assertion in
`cli_matrix_snapshots`, and one that never fired would fail `drifting > 0`.

The docs change was checked the same way rather than trusted:
`mdtablefix --wrap --renumber --breaks --ellipsis --fences --check
docs/developers-guide.md` reports the file's drift, and the new sections were
taken from that command's own stdout, which is what `mdformat-all` would write.
The check also established that the file drifted at `HEAD` too (`+101 -103`), so
the residual drift is pre-existing and not introduced here; see
`Surprises & discoveries`.

### EP-M6 baseline blocked

`EP-M6` did not run. Its first command, `cargo mutants --file
src/report/delta.rs --file src/driver.rs`, collects the 43 mutants the milestone
targets and then refuses to test them because the unmutated baseline is red:

```plaintext
$ cargo mutants --file src/report/delta.rs --file src/driver.rs   # cargo-mutants 27.1.0
Found 43 mutants to test
FAILED   Unmutated baseline in 21s build + 11s test
*** baseline
*** .../cargo test --no-run --verbose --package=mdtablefix@0.6.0
ERROR cargo test failed in an unmutated tree, so no mutants were tested
```

The 43 survivors-to-be, as enumerated before the refusal (line and mutation
only; each is a distinct mutant of the two targets):

| File | Mutants |
| --- | --- |
| `src/driver.rs` | 25 |
| `src/report/delta.rs` | 18 |

That enumeration is a reading of the tree as it was, not of the tip: `a06bab6`
predates `4a599ed`, which is where `Inputs::resolve` and the
`Mode::InPlace if is_changed` guard came from, so the rebased tip yields 46
mutants, 28 of them in `src/driver.rs`. See `Artefacts and notes → EP-M6 run`.

The baseline failure is `tests/check_properties.rs:297`, and it is the third
non-idempotent transform class:

```plaintext
---- generated_documents_reach_a_fixed_point stdout ----
proptest: FileFailurePersistence::SourceParallel set, but failed to find lib.rs or main.rs
thread 'generated_documents_reach_a_fixed_point' panicked at tests/check_properties.rs:297:1:
Test failed: assertion failed: `(left == right)`
  left: `"| 1 | 2 |\n## | --- | --- |\n"`,
 right: `"| 1   | 2   |\n## | --- | --- |\n"`: document must be a fixed point under ["--headings"]
minimal failing input: document = "|1|2|\n|---|---|\n---", mask = 128
test result: FAILED. 19 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
error: test failed, to rerun pass `-p mdtablefix --test check_properties`
make: *** [Makefile:20: test] Error 101
```

A self-checking corpus script over six documents, two passes each, comparing
pass-1 bytes with pass-2 bytes:

| Case | Document (escaped) | `--headings` | `mdformat-all` set |
| --- | --- | --- | --- |
| `C1` | `\|1\|2\|\n\|---\|---\|\n---` | drifts | fixed point |
| `C2` | `\|1\|2\|\n\|---\|---\|\n---\n` | drifts | fixed point |
| `C3` | `prose\n\|1\|2\|\n\|---\|---\|\n---\n` | drifts | fixed point |
| `C4` | `\|A\|B\|\n\|---\|---\|\n---\n\|1\|2\|\n` | drifts | fixed point |
| `C5` | `\|---\|---\|\n\|1\|2\|\n---\n` | drifts | fixed point |
| `C6` | `\|A\|B\|\n\|---\|---\|\nTitle\n---\n` | fixed point | fixed point |

`C6` is the control: prose between the delimiter row and the `---` stops the
absorption. The `mdformat-all` column matters for blast radius — `make fmt` runs
`--wrap --renumber --breaks --ellipsis --fences --in-place` and does **not**
include `--headings`, so no repository document is affected by this class today.
The only route to it is the property test's sampled powerset.

`C1`, pass 0 through pass 3, on the branch's binary:

```plaintext
-- pass0:            -- pass1:
|1|2|               | 1   | 2   |
|---|---|               ## | --- | --- |
---                 -- pass2, pass3 (stable):
                    | 1 | 2 |
                    ## | --- | --- |
```

`C4`, whose trailing body row shows the re-padding rather than the loss of the
header row:

```plaintext
-- pass0:                     -- pass1, pass2, pass3 (stable):
|A|B|                         | A   | B   |     -> | A | B |
|---|---|                       ## | --- | --- |       ## | --- | --- |
---                           | 1 | 2 |             | 1 | 2 |
|1|2|
```

Pre-existence, measured against a binary built from `git archive origin/main`
(v0.5.1) and run on the same corpus:

```plaintext
$ /home/leynos/scratch/mdtablefix-main-probe/target/debug/mdtablefix --headings C1
| 1   | 2   |$
## | --- | --- |$

$ /home/leynos/scratch/mdtablefix-main-probe/target/debug/mdtablefix --headings C4
| A   | B   |$
## | --- | --- |$
| 1 | 2 |$
```

Those are the branch's pass-1 bytes exactly. A self-checking corpus script (the
same one the issue carries) takes the six cases above, formats each twice, and
asserts both the fixed point and the survival of the delimiter row: it reports
`1/6 cases pass` and exits `1`, identically on the branch's binary and on the
`origin/main` binary. This branch's `src/` diff against `origin/main` touches
`driver.rs`, `driver_tests.rs`, `io.rs`, `io/document.rs`, `io/replace.rs`,
`lib.rs`, `main.rs`, `main_tests.rs`, `report.rs`, `report/delta.rs`,
`report/render.rs`, and one snapshot — no transform code — so the class
predates the reporting work.

Gate variance, measured rather than assumed. In one unchanged tree, ten
unseeded runs passed; thirty further runs gave three passes and twenty-seven
failures; five more failed. A second tree gave one pass, then failures. With
`PROPTEST_RNG_SEED=0` the shrink is deterministic and always the same
three-line document. Ruled out as explanations, each by test: a stale binary
(inode and hard-link checked), a feature difference (the manifest has no
`[features]` section, so `--all-features` is a no-op), binary nondeterminism
(30 of 30 identical invocations), path dependence (five different directories,
identical output), and assert_cmd resolution (the worktree binary, the scratch
head binary, and the installed release binary were each run directly). The
residual variance is left as an open question in `Surprises & discoveries`
rather than explained away.

Raised as GitHub issue #474 on the same reasoning that produced #468, and with
the same acceptance bar: the two- to five-line corpus landed as fixtures (for
example under `tests/data/idempotence/`) with a test that formats each fixture
twice through the real binary, **and** a generated-document property whose
generator reaches the table-delimiter / `---` shape often enough that the
assertion fails deterministically rather than flakily. The issue also records
the recommended fix direction — exclude table delimiter rows from Setext
detection, since a delimiter row is table syntax rather than paragraph text —
and the isolated probes that pin the interaction to the two passes rather than
to either one alone. Satisfying the corpus alone does not close it, and neither
does a property test that only sometimes samples the shape — which is the state
the branch is in now.

**Resolution, recorded in Revision 17: closed upstream, by pull request #477,
merged as `408c76a` and absorbed by this branch's rebase.** Neither of the two
remedies the issue names as necessary was skipped there. The deterministic
corpus landed as `tests/data/idempotence/T1_delimiter_then_break.dat` through
`T6_prose_between.dat`, and the six files are the `C1`–`C6` documents in the
table above, renamed one for one and byte-identical (`C6`, the prose-between
control, is `T6`). The unconditional coverage landed as
`tests/idempotence_adjacencies.rs`, `tests/idempotence_drift.rs`, and
`tests/support/idempotence_harness.rs`, which formats every fixture twice through
the real binary, and as a rewritten `tests/idempotence.rs` and
`tests/idempotence_properties.rs`. The fix itself is the exclusion direction this
issue recommended: `src/headings.rs` no longer treats a table delimiter row as
Setext underline text. The documentation of the rule moved with it, in
`docs/adrs/0006-single-pass-idempotence.md`, `docs/architecture.md`,
`docs/developers-guide.md`, and `docs/users-guide.md`.

The rebase replayed this branch over those commits with no textual conflict in
any of the seventeen files, and the gate runner's post-rebase run measured the
`test` gate green, `generated_documents_reach_a_fixed_point` included, which is
what clears `EP-M6`'s precondition. That is a measurement of this tree, not a
proof that the generator can no longer reach a defect: what the resolution
removes is this class, and the fixtures are what keep its shape present in the
suite whatever the sampler draws.

### EP-M6 run

`EP-M6` ran on the rebased tip, through the milestone's own command, from a
clean tree:

```plaintext
$ TMPDIR=target/mutants-scratch cargo mutants -j 3 \
    --file src/report/delta.rs --file src/driver.rs -o target   # cargo-mutants 27.1.0
Found 46 mutants to test
ok       Unmutated baseline in 136s build + 168s test
 INFO Auto-set test timeout to 843s
MISSED   src/driver.rs:287:23: replace match guard is_changed with true in analyse in 4s build + 164s test
MISSED   src/report/delta.rs:92:84: replace > with < in LineDelta::has_changes in 5s build + 82s test
46 mutants tested in 14m: 2 missed, 38 caught, 6 unviable
```

The unmutated baseline passing is the precondition this milestone spent two
revisions waiting for, and it is measured here in the mutation tool's own
scratch tree rather than inherited from `make test`. Three substitutions are
deliberate and none of them changes what is measured: `-j 3` keeps three jobs on
a six-core machine other agents are using, and `TMPDIR` plus `-o` keep the
mutated copies, the build directories, and the results inside the ignored
`target/` directory, so `/tmp` holds a log and nothing else. The two console
transcripts are the durable evidence:
`/tmp/mutants-run-mdtablefix-check-option.out` for the round below and
`/tmp/mutants-iterate-mdtablefix-check-option.out` for the re-check that follows
it. The per-mutant files — `caught.txt`, `missed.txt`, `unviable.txt`,
`timeout.txt`, `mutants.json`, and one diff per mutant under
`target/mutants.out/diff/` — are rewritten by each run and so describe the run
that wrote them rather than the sequence; they live under the ignored `target/`
directory and are not evidence any commit carries.

**The count moved from 43 to 46 between the blocked run and this one**, and both
figures are readings of real trees rather than a correction of an error: the
enumeration above was taken at `a06bab6`. `git log -S` shows `4a599ed` ("Keep
the reporting shape open to a second input source") is where `Inputs::resolve`
and the `Mode::InPlace if is_changed` guard entered `src/driver.rs` — one mutant
for the first and two for the second — and `git merge-base --is-ancestor`
confirms that commit is not an ancestor of the blocked run's tree. Nothing was
lost from the earlier list; it simply predates three of the mutants.

The survivors, and what became of each:

| Mutant | Verdict |
| --- | --- |
| `src/report/delta.rs:92:84`: `>` → `<` in `LineDelta::has_changes` | killed by a new test |
| `src/driver.rs:287:23`: `Mode::Diff if is_changed` → `if true` | equivalent, recorded |

The first is the narrow kind of survivor a one-sided corpus hides. The mutated
body is `self.insertions > 0 || self.deletions < 0`, which agrees with the
original whenever `insertions` is non-zero, and separates only on a delta whose
only change is a deletion. No test asserted `has_changes()` for that shape: the
doctest's cases are `(1, 1)` and `(0, 0)`, the fixed cases check counts rather
than the flag, and the property that does compare the flag against byte
inequality draws 256 generated pairs — by the fact that this mutant survived
them, none of those pairs was a pure deletion. The kill is one assertion in a
test that already computed that exact delta:

```rust
    #[test]
    fn pure_deletion_counts_only_deletions() {
        let delta = LineDelta::between("alpha\nbeta\n", "alpha\n");
        assert_eq!((delta.insertions(), delta.deletions()), (0, 1));
        assert!(delta.has_changes(), "a deletion alone is a change");
    }
```

The symmetric insertion assertion was added to
`pure_insertion_counts_only_insertions` for the same reason, and
`src/report/render.rs` gained one test. Re-measured with `--iterate`, which
skips the mutants already caught and so re-tests exactly the two survivors:

```plaintext
$ TMPDIR=target/mutants-scratch cargo mutants -j 3 --iterate \
    --file src/report/delta.rs --file src/driver.rs -o target
 INFO Iteration excludes 44 previously caught or unviable mutants
Found 2 mutants to test
ok       Unmutated baseline in 20s build + 83s test
MISSED   src/driver.rs:287:23: replace match guard is_changed with true in analyse in 1s build + 86s test
2 mutants tested in 3m: 1 missed, 1 caught
```

The delta mutant is now in `caught.txt` and out of `missed.txt`, and that run's
baseline is green with the new assertions in place, which is how the kill is
measured rather than argued. Transcript:
`/tmp/mutants-iterate-mdtablefix-check-option.out`.

**A third run closes the loop, and it is the one the score is read from.** The
two runs above are a sequence — 38 caught, then the delta mutant killed — and a
score assembled from two runs is a sum rather than a measurement. So the full
set was run once more over the final tree, with the new assertions and the new
test in place:

```plaintext
$ TMPDIR=target/mutants-scratch cargo mutants -j 3 \
    --file src/report/delta.rs --file src/driver.rs -o target
Found 46 mutants to test
ok       Unmutated baseline in 77s build + 112s test
MISSED   src/driver.rs:287:23: replace match guard is_changed with true in analyse in 0s build + 69s test
46 mutants tested in 9m: 1 missed, 39 caught, 6 unviable
```

One run, one tally: **39 caught, 1 missed, 6 unviable, 0 timeouts**, the single
miss being the equivalent mutant justified below. Nothing else moved between the
two runs — the same six mutants are unviable and no mutant the first run caught
became survivable — and the per-mutant lists under `target/mutants.out/` are
this run's. Transcript: `/tmp/mutants-rerun-mdtablefix-check-option.out`.

The second survivor is accepted, on a justification that is itself a test. The
arm is `Mode::Diff if is_changed => render_diff(display_path, &assessment)?`;
mutated to `if true`, a `--diff` run over a file the byte comparison found
unchanged renders a diff of two identical texts. `similar` 2.7's
`UnifiedDiff::to_writer` takes the header inside the hunk loop and `iter_hunks`
filters empty op groups, so equal texts have no hunk, therefore no header, and
therefore no bytes — a property the dependency asserts for itself in
`src/udiff.rs`'s `test_empty_unified_diff`. Under the mutant the payload is
still the empty string, the `is_changed` field of the `FileReport` is computed
before the match and independently of which arm runs, and stdout, stderr, the
exit status, and every file's bytes are identical to the unmutated run. The
mutant's only effect is that a clean file pays for a render that cannot produce
output. Because that reasoning is a claim about this crate's boundary, it is now
pinned there:

```rust
    /// Byte-equal texts render as nothing at all, headers included: `similar`
    /// writes the header alongside the first hunk, and equal texts have no
    /// hunk. A document that is already formatted therefore cannot render as
    /// an empty diff carrying a file name.
    #[test]
    fn equal_texts_render_nothing() {
```

If the renderer ever gains an unconditional banner or header, that test fails
and the guard stops being optional — which is the condition under which this
justification would need revisiting. This is the one survivor the milestone's
own outcome allows to stand, by its second clause: "each survivor is either
killed by a new test or recorded with a justification".

Six mutants are **unviable**, meaning the mutated source does not compile and so
says nothing about test coverage: `exit_status`, `Inputs::resolve`, `assess`,
both `analyse` replacements, and `in_argument_order` mutated to
`vec![Default::default()]`. Each is a default-value substitution for a type that
has no `Default` impl — `ExitStatus`, `Inputs`, `Assessment`, and `FileReport`
derive `Debug`, `Clone`, `PartialEq` and `Eq`, not `Default` — or, for
`in_argument_order`, for an unconstrained generic `T`. They are excluded from
the score rather than counted as kills, which is why the denominator above is 40
rather than 46.

One caveat belongs with the score. `cargo-mutants` counts a mutant as caught
whenever any test fails, so the historically flaky idempotence property can only
move a mutant from missed to caught, never the reverse: the score is an upper
bound on the kills and cannot be the cause of a survivor. No mutant timed out
in either run, and the auto-set timeout — 843 s, then 420 s — was never
approached, the slowest mutant test taking 164 s.

### Forward-compatibility for `--git` (#466)

The `--git` plan's author asked for four changes and supplied a rationale and a
`clap` measurement for each. None of them implements `--git`; all four keep this
branch's shape from foreclosing it. The requests are recorded as A1–A4 in the
request that arrived, and in this plan's own terms:

| Request | This plan's change | Evidence |
| --- | --- | --- |
| mode group requires an `inputs` group | `inputs` holds `files`; `mode.requires("inputs")` | `tests/cli_check.rs::mode_flags_require_an_input_source` |
| an explicit input type, not an empty-list test | `driver::Inputs` with `Stdin` / `Files` | `src/driver_contract_tests.rs::resolve_*`, `AX-6` |
| resolution errors reach `exit_status` | `run` prints and returns `exit_status(mode, false, true)` | `tests/cli_check.rs::a_non_utf8_path_argument_exits_error` |
| `--in-place` is a no-op for clean files | the `Mode::InPlace` payload arm is guarded by `is_changed` | inode and modification-time assertions in `src/driver_in_place_tests.rs` and `tests/in_place_atomic.rs` |

Measurements taken while implementing, all on the built binary:

```plaintext
$ printf '| A | B |\n| 1 | 2 |\n' > clean.md; ln -s clean.md link.md
$ ls -i --full-time
8005586 -rw-r--r--. 1 leynos leynos 20 2026-09-11 15:48:16.884265088 +0200 clean.md
8005588 lrwxrwxrwx. 1 leynos leynos  8 2026-09-11 15:48:16.886265118 +0200 link.md -> clean.md
$ mdtablefix --in-place clean.md; echo $?
0
$ mdtablefix --in-place link.md; echo $?
0
$ ls -i --full-time
8005586 -rw-r--r--. 1 leynos leynos 20 2026-09-11 15:48:16.884265088 +0200 clean.md
8005588 lrwxrwxrwx. 1 leynos leynos  8 2026-09-11 15:48:16.886265118 +0200 link.md -> clean.md
```

Both the inode and the modification time are unchanged by the clean-file run,
and the clean symlink succeeds. The second line is the behaviour change recorded
in `Surprises & discoveries`: before this change, `--in-place` over any symlink
called `replace_file` unconditionally and was refused.

Two further requests needed no code. `--git --list-files` will be a `Mode`
variant taking a `ReadOnlyDir`, and all five of the named items it depends on
(`ReadOnlyDir`, `Assessment`, `Mode`, `exit_status`, `in_argument_order`) are
still items rather than inlined. No file discovery was added: directory walking,
globbing, and `git ls-files` are pull request #466's scope, and the plan's
`Constraints` place new CLI surface outside this plan. The `clap` flag-to-flag
hazard the request described does not arise, because this CLI has no
flag-to-flag requirement to be unreliable — the `mode`-to-`inputs` requirement is
flag-to-group, and its cells are measured rather than reasoned about. The `AX-4`
citation in `ADR 0009` stays, as asked.

### CodeRabbit review after the `--git` forward-compatibility change

Requested through the gate runner with five of the six deterministic gates green
on this tree — `check-fmt`, `lint`, `typecheck`, `markdownlint` (30 files, 0
errors) and `nixie` — and the sixth, `make test`, red on the recorded
pre-existing #474 counterexample. That is a deviation from the standing rule
that every gate be green before a review is requested, and it is recorded rather
than glossed: the rule exists so that CodeRabbit is not asked to catch what a
deterministic gate could catch first, and #474 *is* deterministically caught, by
`make test`, and is already its own issue. The red is a defect in the
`--headings` transform that predates the branch, not a fault in this change, and
every test this change touches passes. The clean result below should still be
read as "nothing was raised about this diff", not as "the tree is green".

The review ran against the pushed commit `83e6150` on `origin/check-option`,
reviewing the whole branch diff rather than this change alone: 62 files, from
`EP-M0` onward. It reported no rate limit, no refusal, and no findings:

```plaintext
{"type":"review_context","reviewType":"committed","currentBranch":"check-option","baseBranch":"origin/main",…}
{"type":"status","phase":"connecting","status":"connecting_to_review_service"}
{"type":"status","phase":"analyzing","status":"reviewing"}
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":[…62 paths…]}
```

The `reviewedFiles` list was checked against `git diff --name-only
origin/main...HEAD` and is set-equal to it, 62 paths each way: the zero is a
completed review of the whole change surface, with no changed file skipped. (The
gate runner's own summary said 69 files; the log it cites says 62, and the
set-equality check is what the claim rests on.) The four areas this change is
most likely to be questioned on drew no comment, because the review produced no
findings at all: the `clap` `inputs` group, `driver::Inputs` and
`Inputs::resolve`, the `exit_status(mode, false, true)` pre-flight path, and the
`--in-place` clean-file no-op with its symlink-to-a-clean-file consequence. As
with the earlier reviews, the zero means the contracted reviewer raised nothing
rather than that the diff was exhaustively audited — it completed in about 49
seconds over a 9,467-line diff, most of which had passed review once before. The
full JSON-lines log is `/tmp/coderabbit-mdtablefix-check-option.out`.

### `make fmt` measured, and declined

`make fmt` was not run on this change. It is not one of the commit gates, and
it cannot be scoped to a diff: `/home/leynos/.local/bin/mdformat-all` accepts no
path arguments and applies itself to every Markdown file in the repository.

```bash
with_all_md() {
  fd --print0 --type f --extension md --extension markdown --extension mdx . |
    xargs -0 "$@"
}
with_all_md mdtablefix --wrap --renumber --breaks --ellipsis --fences --in-place
with_all_md markdownlint-cli2 --fix
```

The first stage is this project's own binary, and the flag set includes
`--wrap`, so running the target would reformat the documents this milestone
writes with the very tool the milestone documents. The second stage is
`markdownlint-cli2 --fix`, which `make markdownlint` already runs in check mode
over the same tree.

Measured rather than assumed, with this branch's binary and the flag set above.
At `HEAD`, 10 of the 31 tracked Markdown files drift:

| File | Drift at `HEAD` |
| --- | --- |
| `CHANGELOG.md` | `+7 -10` |
| `README.md` | `+2 -2` |
| `docs/adrs/0006-single-pass-idempotence.md` | `+15 -16` |
| `docs/adrs/0007-line-ending-detection.md` | `+2 -3` |
| `docs/architecture.md` | `+13 -13` |
| `docs/developers-guide.md` | `+99 -101` |
| `docs/execplans/check-option.md` | `+1028 -1098` |
| `docs/execplans/issue-373-code-block-pipe-line-trailing-pipe.md` | `+1 -2` |
| `docs/users-guide.md` | `+25 -25` |
| `docs/v0-6-0-migration-guide.md` | `+9 -10` |

Three of those — ADR 0006, ADR 0007, and the #373 plan — are files this change
never touches, which is what makes the drift pre-existing rather than a residue
of this work. It is also the same class in every case: prose re-wrapped at a
slightly different width, and table padding tightened, with no word changed.
Running the target would therefore rewrite ten files this documentation change
has no business touching, in the commit that closes it.

Re-measured at `ec936b2`, the branch's last commit: the tracked count has grown
from 31 to 35 because this branch added documents, the drifting set is the same
ten files, and only the deltas moved, for the documents this branch went on to
edit — `docs/execplans/check-option.md` is now `+1158 -1230` and
`docs/developers-guide.md` is `+129 -132`. Re-measured a third time at
`bb068f1`, the rebased tip, the drifting set is still the same ten files and the
developers' guide is `+127 -131` — *down*, because `main`'s #477 rewrote that
same document and took some of the drift with it. The plan's own figure is the
special case: it was `+1158 -1230` at `ec936b2` and `+1259 -1329` at `bb068f1`,
and it changes whenever this document is edited, so it is quoted as a reading
taken at a named point rather than as a property of the file. Each reading is
dated for the same reason. The conclusion does not depend on any of the counts:
the same ten files drifted before this branch existed and drift still, three of
them documents this change never touches, so declining `make fmt` remains the
decision. The command behind both measurements
is `mdtablefix --check` with the five flags above over `git ls-files '*.md'`,
which reports only the files that would change.

Run over this branch's working tree instead, the same measurement is a check on
the change itself, and it caught one regression: `docs/contents.md` is a fixed
point at `HEAD`, and the index entry added for the two new ADRs was wrapped
differently from the formatter, so it drifted at `+3 -4`. The four new
documents were each measured the same way. The two vendored guides are already
fixed points, and the two new ADRs drifted (`+24 -22` and `+19 -19`).

Those three files were therefore re-authored to the formatter's own output, on
the reasoning that a brand-new document has no history to preserve and should
not enter the repository already drifting; the two ADRs and `docs/contents.md`
were each re-run through the flag set a second time to confirm the result is a
fixed point. The only content-level difference in either ADR is the table
separator padding, checked by comparing the two word streams. After that, 10 of
the 35 Markdown files on disk drift — the same ten as at `HEAD`, none of them
new — and the vendored guides, the two ADRs, and the contents index are all
fixed points.

`make check-fmt`, the gate that does exist for formatting, is
`cargo fmt --all -- --check` and covers Rust sources only; it ran with the other
gates on this change.

### EP-M7 prediction control

`INV-PREDICTS` names its own negative control: make `--check` compare trimmed
strings, and a trailing-newline case must fail. It was run as a real mutation
rather than argued about, twice, because the first run showed the control was
weaker than the obligation assumed.

The mutation went into the single shared change decision,
`Assessment::is_changed` at `src/driver.rs:67`, replacing
`self.original != self.formatted` with a comparison of the two texts trimmed.
That is the sharpest form of the hazard, because `--check` and `--in-place`
consult one predicate: under trimmed comparison a document whose whole drift is
its final terminator reports clean **and** is not written, so the two modes
agree with each other perfectly. A test that compared only those two runs would
pass against a broken formatter. The third run is what catches it — printing
renders `assessment.formatted` without consulting `is_changed` at all, so the
printed bytes carry the terminator while the writer's copy does not.

First run, against the corpus as it then stood:

```plaintext
test result: FAILED. 10 passed; 1 failed
minimal failing input: document = "prose words here", mask = 0
  left:  [112, 114, 111, 115, 101, 32, 119, 111, 114, 100, 115, 32, 104, 101, 114, 101]
 right: [112, 114, 111, 115, 101, 32, 119, 111, 114, 100, 115, 32, 104, 101, 114, 101, 10]
generated: --in-place must write exactly the bytes the printer prints, under []
```

All ten corpus cases passed, which was a defect in the corpus rather than in
the mutation: every fixture that drifted under the terminator rule also drifted
in its body text, so trimmed comparison left it drifting anyway. A case only
fails this control if its *entire* drift is the terminator, so
`unterminated_clean` was added — `tests/cli_check.rs`'s `CLEAN` document with
its final newline removed, byte for byte — and the corpus's own assertion, that
each fixture drifts under the flag it was chosen for, now also covers a
document that drifts under no flag at all. The corpus lives in
`tests/check_prediction/corpus.rs`, a module of its own, because it is measured
data rather than a test and because that keeps both files under `AGENTS.md`'s
line limit.

Second run, against the corrected corpus:

```plaintext
failures:
    check_predicts_in_place_on_generated_documents
    check_predicts_in_place_over_the_corpus::case_01
    ... case_02 through case_10 ...
test result: FAILED. 0 passed; 11 failed
```

Every corpus case fails naming `unterminated_clean` and two byte vectors that
differ in one trailing `10`, and the property shrinks to
`document = "```sh", mask = 0` — a fence, also missing its terminator. The
fixture is load-bearing rather than decorative: the first run's ten-and-one
split became the second run's eleven-and-nothing.

Both runs are logged, as
`/tmp/test-mdtablefix-check-option-negative-control.out` and `-2.out`. Each
time, `src/driver.rs` was restored exactly — verified by
`git status --porcelain` on that file and by reading the unmutated line back
before any behaviour was believed. One trap is worth recording: `cargo test`
builds the binary through `assert_cmd`, so the stale mutated binary remained in
`target/debug` after the source was restored, and a manual run against it
reported "clean" for a document the printer changed. That looked like a real
`INV-PREDICTS` failure for as long as it took to remember to rebuild. Proptest
also wrote `tests/check_prediction.proptest-regressions` on each failing run,
pinning a counterexample to a mutation that no longer exists; it was deleted
both times and is not committed.

## Documentation and skills to consult

Repository documents:

- `AGENTS.md`: binding style, the 400-line cap, testing obligations, the
  abstraction and newtype policy (`:217-231`), the `cap_std`/`camino`
  preference (`:232-234`), dependency policy (`:249-263`), error handling
  (`:262-283`), and observability (`:286-306`).
- `docs/contents.md`: the index to everything else; start here.
- `docs/repository-layout.md`: directory ownership.
- `docs/documentation-style-guide.md`: en-GB-oxendict spelling, sentence-case
  headings, 80-column prose, 120-column code, language identifiers on every
  fence, and the ADR template.
- `docs/architecture.md`: the component narrative, especially "Concurrency
  with `rayon`", where the new modes belong.
- `docs/developers-guide.md`: the `src/main.rs` internal API reference, the
  "callers select the function that matches their intent rather than passing a
  Boolean mode flag" convention at `:111-113`, the CLI matrix harness, and the
  observability conventions.
- `docs/adrs/0004-state-machine-abstractions.md`: the ADR format to imitate.
- `docs/execplans/cli-matrix-testing.md`: inherited constraints on test
  placement, `.dat` fixtures, and snapshot discipline.
- `docs/rust-testing-with-rstest-fixtures.md`: fixture and parameterization
  patterns.
- `docs/rust-doctest-dry-guide.md`: writing the new public API's doctests
  without duplicating test logic.
- `docs/trailing-spaces.md`: trailing-space preservation, distinct from the
  trailing-newline rule this plan touches.

Signposted documents absent here, vendored by `EP-M7`:

- `docs/rstest-bdd-users-guide.md`, canonical copy under
  `github---leynos---repovec-appliance/.../docs/`. Needed because this is the
  first adoption of `rstest-bdd` here.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`, canonical copy
  under `github---leynos---evert/.../docs/`. It prescribes generic
  `&impl Trait` injection over `dyn`, which is the style `assess` follows.

Signposted documents absent here and not applicable:

- `docs/netsuke-design.md`: no Netsuke-specific policy is adopted. Aligning
  this repository with the Netsuke lint baseline is issue #441.
- `docs/ortho-config-users-guide.md`: this repository uses plain `clap`, not
  `ortho-config`, and this plan does not change that.

Skills to load:

- `rust-router` first, then the smallest useful follow-on.
- `hexagonal-architecture` to protect the boundary between pure domain and
  adapters, not to impose a directory layout. Note that this plan deliberately
  chose a newtype capability over a port trait; read the skill's
  "when hexagonal architecture applies" guidance before reversing that.
- `rust-unit-testing` for fixture shape, table tests, and choosing between
  equality, matcher, and snapshot assertions.
- `proptest` for generator design and shrinking discipline.
- `rust-errors` for the error-versus-drift distinction and why the library
  stays infallible.
- `rust-types-and-apis` when shaping `LineDelta`, `FileReport`, `Mode`, and
  `ExitStatus`.
- `arch-decision-records` for the two ADRs.
- `en-gb-oxendict` for all prose.
- `commit-message` when committing.
- `codegraph-mcp` for structural questions about callers and blast radius.

## External references

- GitHub issue #452, the check-mode requirement:
  <https://github.com/leynos/mdtablefix/issues/452>.
- GitHub issue #451, the line-ending requirement:
  <https://github.com/leynos/mdtablefix/issues/451>.
- `similar` crate documentation:
  <https://docs.rs/similar/2.7.0/similar/>.
- Black's `--check` and `--diff` semantics:
  <https://black.readthedocs.io/en/stable/usage_and_configuration/the_basics.html>.
- Ruff's formatter documentation, whose `--diff` is specified to "exit with a
  non-zero status code and the difference between the current file and how the
  formatted file would look":
  <https://docs.astral.sh/ruff/formatter/>.
- `dprint check`, which prints diffs and exits non-zero:
  <https://dprint.dev/ci/>.
- golang/go#46289, which changed `gofmt -d` to exit non-zero when diffs exist:
  <https://github.com/golang/go/issues/46289>.
- GitHub issue #465, atomic in-place writes, split out of this plan:
  <https://github.com/leynos/mdtablefix/issues/465>.
- `rstest-bdd`: <https://github.com/leynos/rstest-bdd>.

## Revision note

### Revision 2, 2026-09-09

After a six-lens design review. The first draft's
architecture was substantially wrong in four ways, all now corrected. The
application service moved from the library to the binary, because the argument
for library placement was factually wrong and the placement would have forced
`anyhow` into public library API against `AGENTS.md:266-270`. The
`DocumentStore` port was replaced by a `ReadOnlyDir` newtype, because the port
had one adapter, no second backend in prospect, and — since the draft passed
the store into every mode — did not actually deliver the read-only guarantee
it existed to provide. Exit status became a function of mode as well as
observation, because the draft would have made a successful `--in-place` over
drifting files exit `1`. Storage key and display path were separated, because
`open_file_parent` yields a bare file name and the draft would have reported
`a.md` for `--check docs/a.md`.

Three correctness gaps were added: byte-order-mark handling, without which
`--check` reports clean on a genuinely ragged file; a formatter-idempotence
obligation, without which the gate could never go green; and a deterministic
bound on diff computation. The verification plan was substantially
strengthened — `INV-PREDICTS` was circular, `LEM-COUNT` was satisfied by an
implementation doing no diff at all, `INV-NOWRITE` was blind to ambient
writes, `INV-ORDER`'s control was probabilistic, and `rayon`'s collection
order turned out not to be a documented guarantee. The Verus milestone was
cut, because its stated goal reduces to an arithmetic identity and proving it
would restate an assumed property.

Two requirements were reaffirmed against contrary evidence and flagged for the
approval gate rather than silently changed, along with one out-of-scope
addition. Revision 3 resolves all three.

### Revision 3, 2026-09-09

Resolving the three items revision 2 raised at the approval gate, on explicit
direction from `@leynos`.

`--diff` now exits `1` on drift rather than `0`, adopting the behaviour of
`ruff format --diff`, `dprint check`, and modern `gofmt -d` on the principle
of least surprise. This also dissolves the mutual-exclusion concern rather
than requiring a second change: the reason Black, ruff, and `terraform fmt`
permit combining check with diff is so that one run can both display the drift
and fail the build, and `--diff` now does both by itself. `--check` and
`--diff` therefore remain mutually exclusive as two renderings of one analysis
with identical exit semantics, and no continuous-integration usage needs a
second invocation, which preserves the requirement that no file is read more
than once. `INV-EXIT` gained a third high-risk cell and a matching negative
control, because a mapping that suppresses drift for every mode except
`--check` would pass the `--in-place` assertion while failing `--diff`.

`EP-M1b`, atomic write-then-rename, is removed from this plan and raised as
GitHub issue #465. The hazard remains recorded in `Risks` with a pointer to
that issue, and the recommendation stands to sequence #465 immediately after
this work so the serialization path is edited once rather than twice.

Whole-file majority line-ending detection is accepted, including its rewriting
of LF-authored snippets inside fenced code blocks in a mostly-CRLF document.
`INV-DOCUMENT` already carries the mixed-endings-inside-a-fence case so the
behaviour is pinned by test, and `EP-M7` documents it in the user's guide.

The five proposed dependencies are accepted.

No implementation has begun; the plan awaits approval.

### Revision 4, 2026-09-09

Approved. `@leynos` directed implementation to proceed, with the standing
instruction that every applicable deterministic gate must pass before each
CodeRabbit review, and that the ExecPlan is to be kept current as work
proceeds. Status moved from `DRAFT` to `IN PROGRESS`.

### Revision 5, 2026-09-11

Rebased onto `origin/main` after three pull requests merged while the plan was
halted at `EP-M2`, and resumed. Recorded in `Artefacts and notes → Rebase onto
origin/main` and in six `Decision log` entries dated 2026-09-11.

What changed in the plan itself:

- `EP-M1`'s module is `src/io/document.rs`, not `src/document.rs`, and it takes
  the line-ending policy from `src/io/line_endings.rs` rather than redefining
  it. `EP-M1` now delivers the byte-order-mark half only; #469 delivered the
  line-ending half.
- `INV-DOCUMENT`'s artefact reference follows the module move.
- `EP-M3` no longer claims a version bump: `0.6.0` is already declared and
  unreleased, so the exit-status change ships inside it.
- The check-and-diff ADR is `0009` and the byte-order mark gets `0008`; `0006`
  and `0007` are held by merged records, and the trace links in `Conformance
  basis` follow.
- The `--in-place` truncation risk is marked discharged by #467.
- `EP-M2` is complete. `INV-IDEMPOTENT` was the blocker and #470 cleared it; the
  red state was re-observed on the rebased tree before any green code was
  written, so the milestone's own tolerance gate was honoured rather than
  assumed.

`EP-M2`'s acceptance was met, including all three `Verification plan` negative
controls; see `Artefacts and notes → EP-M2 green transcripts`. One control
exposed that the conservation property was near-vacuous under the red state's
generator, which is now fixed, and one artefact was rescoped against the two
idempotence suites #470 added.

### Revision 6, 2026-09-11

`EP-M3` implemented; see `Artefacts and notes → EP-M3 red and green
transcripts`.

What changed in the plan itself:

- `Validation and acceptance`'s clean fixture is the formatter's own padded
  bytes. `| A | B |` is not a fixed point, so the block as written would have
  failed the first command a reviewer ran, and its `--diff` transcript carried
  the same unpadded body.
- `EP-M3` step 1 records what the red state actually was: a run-time rejection
  by `clap`, not the compile error the step predicted.
- `EP-M3` step 2 records the deliberate split of `Mode::Diff` into `EP-M4`.
- `Interfaces and dependencies`' `assess` documentation no longer mentions a
  `display_path` parameter the signature does not take.

No requirement, obligation, or acceptance criterion changed. The `EP-M3`
outcome, the three exit statuses, the read-only guarantee, and the argument
order guarantee were all met as specified; the four corrections above are
factual, not scope.

### Revision 7, 2026-09-11

`EP-M4` implemented; see `Artefacts and notes → EP-M4 red and green transcripts`
and `→ INV-DETERMINISTIC negative control`.

What changed in the plan itself:

- `INV-DETERMINISTIC`'s non-vacuity line stated the control's predicted outcome
  as a requirement — "must make the test flaky or fail". The control was applied
  and the outcome is now recorded as measured: it does make the test fail, both
  deterministically on an above-threshold corpus and nondeterministically in the
  transition band, and it also exposed that the ten-run method cannot see a
  budget that is never crossed. The prediction was right about the hazard and
  incomplete about its detection.
- The same block's artefact and evidence lines now name the above-threshold
  case, because the first version of that test was insensitive to the hazard it
  existed to detect: with one changed table at the end of 1200 unchanged lines,
  trimming reduces the work to that table and the degraded render is
  byte-identical to the correct one.
- `Rigour and residual gaps` records that `INV-DETERMINISTIC` is held by
  construction rather than enforced, with the two candidate ways to close it.
- `EP-M4`'s acceptance now says "below and above the line-count threshold", and
  its control outcome is recorded as its own line.

No requirement, obligation, or acceptance criterion changed. `EP-M4`'s outcome,
the exit-status contract, the read-only guarantee, and the deterministic
rendering were all met as specified. The above-threshold test is a strengthening
of `INV-DETERMINISTIC`'s artefact, not a new obligation: it asserts the same
invariant over the corpus shape where the invariant is actually at risk.

### Revision 8, 2026-09-11

`EP-M5` implemented; see `Artefacts and notes → EP-M5 red and green transcripts`.

What changed in the plan itself:

- `EP-M5`'s step 2 said to review every changed `.snap`. That is the wrong test
  for this milestone: the curated subset adds twelve snapshots and changes none,
  so the review covers every *added* file, and the "none changed" claim is
  established by an `md5sum` comparison against a baseline taken before
  regeneration rather than by inspection.
- `EP-M5`'s step 1 is recorded as implemented with its three curated rows named
  and the reason each is in the subset. The no-drift branch exists only because
  `row_010` unwrapped happens to be a fixed point, which was measured across
  all sixteen row-and-wrap combinations rather than assumed from the row's
  transforms.
- The `Constraints` snapshot-churn allowance is spent to twelve of its forty
  lines, and the "full four-mode expansion" that the milestone rejects remains
  rejected: it would add 32 files against this subset's twelve.
- The diff invariant's first form is recorded in `Surprises & discoveries` as a
  near-miss. It reconstructed the printed document from the payload's marker
  lines, which is only correct while every line of the document falls inside
  some hunk's context; the smallest curated fixture already falsifies that, and
  the replacement applies the payload instead.

No requirement, obligation, or acceptance criterion changed. The two modes
remain rendering variants of one assessment, the exit-status contract is
unchanged, and nothing in this milestone touched production code: the diff is
`tests/` and `docs/` only.

### Revision 9, 2026-09-11

`EP-M6` halted before its first command; nothing was implemented in this
revision. The halt is a finding, not a failure to start: the mutation run is
blocked by a genuine counterexample in the `test` gate, and the plan's
`Tolerances` require recording and escalating rather than working around it.

What changed in the plan itself:

- `Progress` now records `EP-M6` as blocked with the reason, and states plainly
  that `make test` is red at `a06bab6`. No earlier milestone's completion claim
  is retracted — every one of them was gated on a run that passed — but the
  claim that the tree is currently green is not made.
- `Concrete steps → EP-M6` gains a step 0 precondition, `Milestones and
  plateaus → EP-M6` gains its blocker paragraph, and three `Decision log`
  entries record why the milestone was deferred rather than the flaky test
  excluded, why the class is escalated as GitHub issue #474, and why no further
  commit claims all gates green.
- `Surprises & discoveries` gains the class itself (with the six-case corpus
  and the `origin/main` byte-identity evidence) and the flakiness finding, whose
  unexplained residual variance is recorded as an open question rather than
  asserted away.
- `Artefacts and notes → EP-M6 baseline blocked` carries the mutants refusal
  transcript, the corpus table, the pass-by-pass bytes, the pre-existence
  measurements, the variance evidence and what would close the gap.

No requirement, obligation, or acceptance criterion changed for `EP-M0` through
`EP-M5` or for `EP-M7`. The reporting feature's own contract — one assessment
rendered three ways, exit `0`/`1`/`2`, drift only in the reporting modes — is
untouched by this class, which lies in the `--headings` transform and predates
the branch. What the discovery does change is the meaning of the `test` gate for
`EP-M6` and `EP-M7`: it is currently a coin flip on this input class, so a green
run is not by itself evidence that `INV-IDEMPOTENT` holds. `EP-M7` can proceed
on its documentation diff, gated by `markdownlint` and `nixie`, but the feature
cannot be called closed while a `--headings` input is not a fixed point.

### Revision 10, 2026-09-11

Forward-compatibility with the `--git` plan (pull request #466): four small
changes, no new CLI surface, and `--git` itself not implemented. The `--git`
plan is sequenced after this one, and its author asked for the changes so that
its second input source would not have to be contorted around this branch's
shape — with the explicit invitation to decline anything that conflicted with
this plan. Nothing did.

What changed:

- The `mode` group now requires an `inputs` group holding `files`, so a mode
  flag still demands an input source while the source is a named thing to
  extend. Behaviour is unchanged; the requesting plan measured both shapes on
  `clap` 4.6.6, and `tests/cli_check.rs` now pins the whole accept/reject matrix
  from this side, including the two-file positional that a group with
  `multiple(false)` could plausibly have broken.
- `driver::Inputs` replaces `main`'s `cli.files.is_empty()` test, recorded as
  `AX-6` in `Verification plan`: no paths named is `Inputs::Stdin`, and a source
  that resolves to no paths is `Inputs::Files(vec![])`, which a future `--git`
  needs in order to exit `0` on an empty match rather than block on a terminal.
- Input resolution now happens before the parallel stage and its failures return
  through `exit_status(mode, false, true)`, so they exit `2` like every other
  operational failure. This is a user-visible change for a non-UTF-8 argument,
  recorded in `Surprises & discoveries`: the run now fails as a whole instead of
  counting that path as one file's error.
- `--in-place` no longer writes a file whose bytes would not change. The bytes
  would be identical but the file would not be: the replacement renames a
  temporary over the target, so the inode and the modification time would move.
  Both properties are now pinned, each beside a positive control that proves a
  drifting file *is* replaced. This is a behaviour change and is declared as one:
  a symlink to a clean file is no longer declined, because there is no
  replacement to decline.

The plan's interfaces were updated to match (`driver::Inputs`, the `write_back`
doc, the `Cli` group attributes and the `run`/`run_stdin`/`run_files` split), the
`Decision log` carries one entry per request plus one recording that the
remaining requests needed no change, and
`Artefacts and notes → Forward-compatibility for --git (#466)` carries the
request-to-evidence table and the inode measurement. The `AX-4` citation stays in
the ADR, as the requesting plan asked.

`EP-M6` stays blocked and `EP-M7` stays pending; this revision neither unblocks
nor blocks them. `make test` is still red for the recorded #474 counterexample,
so no commit in this revision claims a passing test suite.

### Revision 11, 2026-09-11

The forward-compatibility change of revision 10 was reviewed by CodeRabbit and
came back clean: `review_completed`, 0 findings, no rate limit and no refusal.
The review ran against the pushed commit `83e6150` over the whole branch diff —
62 files, set-equal to `git diff --name-only origin/main...HEAD` — and raised
nothing on any of the four areas the change is most exposed on. That is one more
piece of evidence, not proof: as with `EP-M0`, `EP-M3` and `EP-M4`, the review
is a contracted second reader, and its zero means it raised nothing rather than
that the diff was exhaustively audited.

What changed in the plan itself is the record of how that review was requested.
Five of the six deterministic gates were green and `make test` was red on the
recorded pre-existing #474 counterexample, so the request was a deviation from
the standing "all gates green first" rule. The artefact note recording the
CodeRabbit review of the `--git` forward-compatibility change states the
deviation and its reasoning plainly, and asks the reader to take the clean
result as "nothing raised about this diff" rather than as evidence that the tree
is green. The same
section records the `reviewedFiles` set-equality check and notes that the gate
runner's summary reported 69 files where the log it cites carries 62, so the
number in this plan is the measured one.

`EP-M6` remains blocked on issue #474 and `EP-M7` remains pending. No
requirement, obligation, or acceptance criterion changed in this revision, and
no code, test, or configuration file was touched: the revision is
`docs/execplans/check-option.md` alone.

### Revision 12, 2026-09-11

`EP-M7` steps 1–8, the documentation of the whole feature. No Rust source,
test, or configuration file changed: the diff is `README.md`, `CHANGELOG.md`,
`docs/contents.md`, `docs/users-guide.md`, `docs/v0-6-0-migration-guide.md`,
`docs/architecture.md`, `docs/developers-guide.md`,
`docs/execplans/check-option.md`, and the four new documents
(`docs/adrs/0008-byte-order-mark-preservation.md`,
`docs/adrs/0009-check-and-diff-reporting.md`, and the two vendored guides).

What is written, against the step list:

- The user's guide's `Command-line usage` section covers every flag, the three
  file modes, the exit-status table, how to read a report line, line-ending and
  byte-order-mark behaviour (including the fenced-code homogenisation and the
  lone-`\r` limitation), the trailing-newline rule, the empty-glob hazard, and
  the symlink limitation. Each claim was reproduced on the built binary before
  it was written, and the two empty-glob workarounds were run rather than
  quoted.
- `README.md`'s 52-line flag list is now a synopsis linking to that section.
- `docs/architecture.md` gained `Check and diff reporting` with a sequence
  diagram, a `report` class, a `driver` class, a `SourceDocument` entry on
  `io`, and a corrected `## Contents`. The figure numbering was corrected as
  part of the edit: the new section sits between the concurrency and atomic
  figures, so the concurrency figure became `Figure 3` and the new one
  `Figure 4`, keeping reading order ascending.
- `docs/developers-guide.md` gained the CLI driver and reporting architecture
  sections (read-only by type, one formatter built once, explicit argument
  order, the binary's private driver) and the BDD test-infrastructure
  subsection. The two `## Internal API reference` blocks that still described
  `format_to_string` and `rewrite_in_place` — functions deleted in `EP-M3` —
  were replaced with the items that exist, and the `report_line_endings` prose
  now names the boundary in `src/driver.rs` as well as the private one in
  `src/io/replace.rs`.
- Both ADRs are written to the house template. `ADR 0009` keeps the `AX-4`
  citation, as the `--git` plan's author asked.
- Both vendored guides carry a provenance header naming the source repository,
  the commit, and the blob hash, each verified by md5 against the sibling
  checkout rather than asserted.

One decision belongs in the open: `make fmt` was declined, with the
measurement and the reasoning in
`Artefacts and notes → make fmt measured, and declined`. The measurement also
caught a regression this change had introduced — `docs/contents.md` is a fixed
point at `HEAD` and my index entry was not — so the two new ADRs and
`docs/contents.md` were re-authored to the formatter's own output and re-run to
confirm they are fixed points. After that the change adds no new drift: 10 of
the 35 Markdown files on disk drift, the same 10 as at `HEAD`, three of which
this change never touches. The gates for this revision and the issue closure
are recorded in Revision 13; no commit in this revision claims a green
`make test`, which remains red for the recorded #474 counterexample.

### Revision 13, 2026-09-11

`EP-M7` closes, and the plan takes stock without declaring itself finished.

Gate run for the documentation change of Revision 12, through the gate runner:
`make markdownlint` reports 34 files and 0 errors
(`/tmp/markdownlint-mdtablefix-check-option-3.out`), and `make nixie` validates
all 10 Mermaid diagrams in the tree, including the two new and one edited
diagram in `docs/architecture.md`
(`/tmp/nixie-mdtablefix-check-option-2.out`). The first markdownlint run was
**red** — three `MD060` table-alignment errors on the exit-status table this
change adds to `docs/users-guide.md` — and is kept at
`/tmp/markdownlint-mdtablefix-check-option-2.out` rather than overwritten,
because the failure is evidence that the gate ran over the new content rather
than that it was skipped. No Rust gate was run: no Rust source, test, or
configuration file changed in this revision.

The documentation landed as commit `95ec57c` and is pushed to
`origin/check-option`. Issue #452 is closed as not planned, with a comment
recording the `--concise` supersession, the two renderings that replace it, and
the links to `ADR 0009` and this plan; #451 was already closed by an earlier
milestone.

`Outcomes & retrospective` is written, `Progress` records `EP-M7` as complete,
and the plan's status line now states what remains: `EP-M6` is blocked by issue
[#474](https://github.com/leynos/mdtablefix/issues/474) and `make test` is
still red on that counterexample, so the plan stays `IN PROGRESS` rather than
being set `COMPLETE`. Nothing in this revision changes a requirement,
obligation, or acceptance criterion.

### Revision 14, 2026-09-11

Revision 13 closed `EP-M7` and the plan claimed, in three places, that every
obligation was discharged. An audit of those claims found one that was not:
`INV-PREDICTS` was recorded as tested by `tests/check_properties.rs`, which
never runs `--check` or `--in-place` at all — it drives the formatter in print
mode and asserts the delta and idempotence claims. The obligation was therefore
discharged by no test, and the retrospective said otherwise. This revision adds
the missing artefact and corrects the claims. **The obligation is strengthened,
not changed**: no requirement, acceptance criterion, or behaviour moves, and no
implementation file changes.

`tests/check_prediction.rs` (356 lines) and its corpus module
`tests/check_prediction/corpus.rs` (82 lines, split for `AGENTS.md`'s 400-line
limit) run the reporting and writing modes over byte-identical copies of one
document and compare what `--check` said with what `--in-place` did.

The obvious shape — two runs, compared — is vacuous here, and that is the
finding worth recording. Both modes consult one shared change decision, so a
predicate that answered wrongly would move the report and the write in the same
direction and the agreement would hold. A third run is therefore the oracle:
printing renders `assessment.formatted` without consulting `is_changed`, so the
prediction is measured against bytes that decision cannot move. The generator
samples the document boundary — line-ending style, byte-order mark, final
terminator — which is where a comparison made on body text would miss the
difference.

Eleven tests: ten corpus cases (the bare flag set, each of the eight flags
alone, and all eight together) over a sixteen-document corpus, plus one
generated property of 48 cases sampling the flag powerset from a bitmask.
`cargo test --test check_prediction` is green — 11 passed, 0 failed
(`/tmp/test-mdtablefix-check-option-prediction.out`).

The corpus was measured rather than assumed, and measuring it found the
neighbouring corpus's comment wrong: `tests/check_properties.rs` describes its
entries as "Documents that drift under exactly one flag each", but `prose`
never drifts, its `--wrap` entry drifts only because of the Setext heading in
it, and its footnotes entry is a no-op. That comment is left alone — the
idempotence claim it supports does not depend on it — while the new corpus
carries its own measured association: one fixture per flag, each asserted to
drift under that flag, alongside the boundary and clean documents.

The obligation's negative control — make `--check` compare trimmed strings, and
a trailing-newline case must fail — was run as a real mutation of the shared
predicate, twice. The first run failed only the generated property, because
every corpus fixture that drifted under the terminator rule also drifted in its
body and so kept drifting under trimmed comparison. `unterminated_clean` was
added — `tests/cli_check.rs`'s `CLEAN` document minus its final newline — after
which the second run failed all eleven tests: the corpus cases naming that
fixture, and the property shrinking to `document = "```sh"`. Both transcripts
are in `Artefacts and notes → EP-M7 prediction control`, which also records the
stale `target/debug` binary that produced a false alarm after the mutation was
reverted, because that is the kind of evidence a reader would otherwise have to
rediscover.

Two plan errors are corrected rather than overwritten. `Outcomes &
retrospective` now says what `tests/check_properties.rs` actually discharges —
`LEM-COUNT` and `INV-AGREE`, in-process and not over two copies — and keeps the
`INV-PREDICTS` mistake visible with its lesson: a claim about evidence is itself
a claim. The obligation carries the corrected artefact, evidence, non-vacuity
statement, and control result. `Progress` records step 11, added to `EP-M7` for
this work.

Gate run, through the gate runner, over the new test and this plan revision
(including the Revision 13 text, the status line, and the corrected
retrospective and obligation, none of which was committed when the last gate
run was taken): `make check-fmt`,
`make lint`, `make typecheck`, `make markdownlint` (34 files, 0 errors), and
`make nixie` all pass, and the new test files compile under
`--all-targets --all-features` with `-D warnings` contributing no diagnostic.
`make test` is **red**, and `check_prediction` passed 11/11 inside it before the
abort — the failure is
`check_properties::generated_documents_reach_a_fixed_point` on
`document = "|1|2|\n|---|---|\n---", mask = 128`: the recorded issue #474
class, and not this change. Because `cargo test` fail-fast aborted there,
36 integration binaries and the doctests did not execute in that run, so the
whole suite was run again with `--no-fail-fast` rather than left partly
unverified: all 43 binaries ran, exactly one failed — the same
`check_properties` function on the same minimal input, replayed from the
persisted regression file — and the doctests pass, 40 passed and 20 ignored. No
`*.proptest-regressions` file was created or modified by either run. Logs:
`/tmp/check-fmt-mdtablefix-check-option.out`,
`/tmp/lint-mdtablefix-check-option.out`,
`/tmp/typecheck-mdtablefix-check-option.out`,
`/tmp/test-mdtablefix-check-option.out`,
`/tmp/test-no-fail-fast-mdtablefix-check-option.out`,
`/tmp/doctest-mdtablefix-check-option.out`,
`/tmp/markdownlint-mdtablefix-check-option.out`,
`/tmp/nixie-mdtablefix-check-option.out`.

### Revision 15, 2026-09-11

Revision 14 added the missing `INV-PREDICTS` artefact. This revision records
what followed from it: the one sentence in the developer's guide that it left
stale, a hand-check of the committed artefact, a second CodeRabbit run in place
of the one that returned nothing, and the pull request description. No
requirement, acceptance criterion, or code file changes; the diff is two
Markdown files.

`docs/developers-guide.md`'s "One formatter, built once" said that one closure
behind both modes "is what makes `--check` and `--in-place` structurally unable
to disagree". The sentence is true, and it was read as sufficient — which is
exactly the reading Revision 14 refuted, because two modes that consult one
decision agree with each other however wrong that decision is. The section now
separates the two claims: the structure is necessary but not sufficient, and
`tests/check_prediction.rs` is what tests the rest. It names the corpus module,
states why two runs would be vacuous, and says what the third run supplies
instead. That is `EP-M7` step 4's subject matter, so the step list gains a step
12 for it.

The artefact was then checked by hand as well as through the suite, on the
fixture that carries the terminator rule: a padded table with no final newline.
`--check` printed `/tmp/spot2/input.md +1 -1` on stdout and
`1 file would be reformatted.` on stderr, exited 1, and left the file
byte-identical, confirmed by `cmp` against a copy. `--in-place` over that same
file exited 0 and added the terminator, visible under `od -c` as the closing
`\n`, and a second `--check` printed `1 file left unchanged.` and exited 0. The
report line names the path as given rather than the file's base name, the counts
are the rewrite's own delta, and the sequence is the prediction test in
miniature. Doing it by hand also exercises the shipped binary rather than the
test harness, which is the only place `main`'s argument handling, the exit
status, and the printer meet.

`coderabbit review --agent --committed` was run again over the branch tip
`17d242e`, because the attempt taken in Revision 14's wake stopped without a
result — its log held only `review_context` and `connecting_to_review_service`.
The second run completed: exit 0, `review_completed`, 0 findings, 73 seconds,
against a `reviewedFiles` set measured as exactly equal to
`git diff --name-only origin/main...HEAD` — 73 files either way, no file in the
diff unreviewed and none reviewed that is not in the diff, the two new test
files among them.
As with the earlier reviews, that zero means nothing was raised, not that the
diff was exhaustively audited. Log:
`/tmp/coderabbit-mdtablefix-check-option.out`. This revision is documentation
that was committed after that review, so the reviewed tip predates it; a
further run over the new head was requested and its outcome is recorded in the
revision that follows this one.

Pull request #464's description still described the plan as unstarted — it
carried "the Rust gates were not applicable", which was written when the
deliverable was expected to be documentation, and a Sourcery summary of the
plan document rather than of the implementation. It was rewritten to describe
what the branch does: the two reporting modes and the exit-status contract, the
three changes taken beyond the two flags, the obligation-to-artefact table, the
gate behaviour including the #474 deviation and the `--no-fail-fast` run, the
supersession of issue #452, and the design review's four architectural
corrections. The body is a summary of this plan and cites it for the detail;
where the two disagree, the plan is the record.

A gate run over this revision cost one fix, and it is the same defect class this
plan already records twice: a line wrapped so that it begins with `#452` parses
as an ATX heading missing its space, and `make markdownlint` reports MD018. The
prose was reflowed to put the issue number mid-line, changing nothing about
what the revision says. The reminder is that a rewrap is an edit, and a plan
that mentions issue numbers in running prose will trip this whenever a line
break lands on the `#`.

Gate run, through the gate runner, over this revision: `make markdownlint` (34
files, 0 errors) and `make nixie` (all diagrams validated) pass. The first
markdownlint attempt was the red one described above; the re-run after the
reflow is the pass that counts, and it was taken over this text, including this
paragraph. Logs: `/tmp/markdownlint-mdtablefix-check-option.out`,
`/tmp/nixie-mdtablefix-check-option.out`. The Rust gates were not re-run, and
not because they would fail: nothing outside these two Markdown files changed,
so `check-fmt`, `lint`, `typecheck`, and the test suite would report exactly
what Revision 14 recorded — green apart from the pre-existing #474
counterexample in `check_properties`.

### Revision 16, 2026-09-11

Two threads left open by Revision 15 are closed here, and nothing else changes:
the review of that revision's own commit, requested after the commit was pushed
because the previous review's tip predated it, and the `make fmt` measurement,
which had been taken at an earlier `HEAD`. No requirement, acceptance criterion,
or code file changes; the diff is `docs/execplans/check-option.md` alone.

`coderabbit review --agent --committed` over `ec936b2` — the commit that carries
Revision 15 — completed in 55 seconds with exit 0, `review_completed`, 0
findings, and no rate limit, quota message, or refusal anywhere in the log. Its
`reviewedFiles` set is again exactly equal to
`git diff --name-only origin/main...HEAD`, 73 files either way, so the review
covered the documentation revision and the corpus and prediction test files
alike. The log records no commit SHA; that is a limitation of the tool rather
than of the run, and the commit is known by HEAD at launch and by `git log`, not
from the transcript. Log:
`/tmp/coderabbit-rev15-mdtablefix-check-option.out`. Taken with every earlier
run this plan records, from `EP-M0` onward, no CodeRabbit review of this branch
has yet raised a finding; that is a consistent result rather than independent
evidence that the tree is correct, and this plan treats it as the former.

The `make fmt` artefact in `Artefacts and notes` said that 10 of 31 tracked
Markdown files drift. That reading was taken at an earlier `HEAD`; re-measured
at `ec936b2` with the same five flags over `git ls-files '*.md'`, 10 of 35
drift — the same ten files, the tracked count having grown because this branch
added documents, and larger deltas only for the documents the branch itself
edited. The section now carries both readings and the command that produced
them, so the claim behind declining `make fmt` can be re-run rather than
trusted.

Gate run, through the gate runner, over this revision: `make markdownlint` (34
files, 0 errors) passes. `make nixie` and the Rust gates are not re-run, and not
because they would fail: no diagram, Rust source, test, or configuration file
changed. Log: `/tmp/markdownlint-mdtablefix-check-option.out`.

### Revision 17, 2026-09-11

This revision is the rebase onto `origin/main`, and the record of what it took
to get right. The branch is replayed onto `408c76a`, `Refuse Setext conversion
of a table delimiter row (#474) (#477)` — the upstream fix for the defect class
this plan raised as issue #474 and recorded as `EP-M6`'s blocker. That clears
the precondition `cargo mutants` aborted on, so `EP-M6` is unblocked and is now
the only outstanding milestone; the milestone itself is **not** run here, and no
mutant score is claimed. The substance of this branch's own diff is unchanged by
the rebase: 26 commits replayed, no file conflicted in either attempt, and the
difference between the rebased tip and the pre-rebase tip `2a73a58` is exactly
`408c76a`'s own stat — 17 files, 1010 insertions, 551 deletions, the same 17
paths.

**The first attempt rebased cleanly and was still wrong.** `git rebase
origin/main` exited successfully with no conflict, and the Weave merge driver
that the global attributes file selects for Markdown silently corrupted three
documents: `docs/architecture.md`'s footnotes example lost its `Before:`/`After:`
structure — the `Before:` lines were rewritten to the `[^1]:` form and the
`After:` label, its blank line, the opening fence, and the `Text.` line under it
were dropped — and fourteen blank lines appeared across `docs/architecture.md`
(one), `docs/developers-guide.md` (five), and `docs/users-guide.md` (eight).
Nothing in the driver's output said so; it reported auto-resolutions at `high`
and `very_high` confidence, and once at `conflict`.
The blobs were compared three ways to
establish it, the result was preserved under `/tmp/weave-corrupt/` before
anything destructive, and the operation was re-run — after verifying with
`git -c core.attributesFile=/dev/null check-attr merge -- <path>` that the
override really does return the path to Git's built-in machinery — as
`git -c core.attributesFile=/dev/null rebase origin/main`. The re-run produced no
conflict markers, restored the footnotes example byte-for-byte (same MD5 as
`origin/main`'s block), and left a tree whose delta against `2a73a58` is
`main`'s change and nothing else. Measured afterwards: the preserved copies fail
`markdownlint` with 21 errors under this repository's own configuration, against
0 for the merged files, so the damage was real and the gates would eventually
have caught it — after a corrupted rebase had been committed. The full account,
with the integrity checks over all 17 of `main`'s paths, is in
`Surprises & discoveries` and
`Artefacts and notes → Second rebase onto origin/main (#477)`.

`EP-M6`'s blocker is recorded as cleared in the milestone section, in its step 0
precondition, in `Progress`, and in `Outcomes & retrospective`; the status line
now says the milestone is unblocked and unrun rather than blocked. The condition
the blocker paragraph refused to satisfy — green by excluding the failing test
or pinning a passing seed — is untouched, because the fix came from upstream
instead.

Gate run, through the gate runner, over the rebased tip `bb068f1` with a clean
tree, all six gates, sequentially: `make check-fmt` passes (7 s), `make lint`
passes (74 s, cold dependency compile, clippy `--all-targets --all-features`
`-D warnings`), `make typecheck` passes (10 s), `make test` passes (177 s: 45
test binaries plus the doc-test target, 46 `test result:` blocks, 1860 passed,
0 failed, 20 ignored, all ignored being doc-tests), `make markdownlint` passes
(34 files, 0 errors), and `make nixie` passes (all Mermaid diagrams valid).
`generated_documents_reach_a_fixed_point` — the test whose failure blocked the
milestone — is `ok` in that log, so the green is the upstream fix arriving and
not the failure moving. No `*.proptest-regressions` file was created or
modified by the run (both pre-existing files are byte-identical by MD5,
including the ignored `tests/check_properties.proptest-regressions`, which
`git status` cannot show). `make fmt` is deliberately not run, unchanged from
Revisions 15 and 16, and its artefact now carries a third reading taken at this
tip: the same ten files drift, and the developers' guide is at `+127 -131`,
lower than at `ec936b2` because `main`'s #477 rewrote that file too. The plan's
own figure, `+1259 -1329` when that reading was taken, changes whenever this
document is edited and is recorded as a reading rather than a property. Logs:
`/tmp/check-fmt-mdtablefix-check-option.out`, `/tmp/lint-…`, `/tmp/typecheck-…`,
`/tmp/test-…`, `/tmp/markdownlint-…`, `/tmp/nixie-…`, all under `/tmp` with the
`mdtablefix-check-option` slug.

This revision changes no requirement, obligation, acceptance criterion, or code
file. The diff is `docs/execplans/check-option.md` alone; the rebase changed
which commits the branch sits on, not what it contains.

### Revision 18, 2026-09-12

This revision runs `EP-M6` on the tip Revision 17 produced, kills the survivor
it turned up, and discharges the last outstanding milestone. `EP-M0`–`EP-M7`
are complete; the status line, `Progress`, `Outcomes & retrospective`, and the
milestone section all say so, and `Artefacts and notes → EP-M6 run` carries the
command, the transcript, and the per-mutant evidence paths.

The run was the milestone's own command over the two files `EP-M6` names, at
the rebased tip, from a clean tree, with `-j 3`, `TMPDIR=target/mutants-scratch`
and `-o target` so that the mutated copies, the scratch builds, and the results
all stay inside the ignored `target/` directory and `/tmp` holds only logs.
Measured: the unmutated baseline is green (136 s build + 168 s test) — the
precondition the previous two revisions could not satisfy — and of **46
mutants, 38 were caught, 6 were unviable, 2 were missed, none timed out**, in
14 minutes. The delta survivor (`LineDelta::has_changes` mutated from `>` to
`<`) is killed by a new `pure_deletion_counts_only_deletions` test and a
symmetric assertion in `pure_insertion_counts_only_insertions`; re-measured
with `--iterate`, which re-tests only the two survivors, it moves into
`caught.txt` (`1 missed, 1 caught`, baseline green with the new tests). The
driver survivor (`Mode::Diff if is_changed` mutated to `if true`) is accepted as
an equivalent mutation, and its premise is pinned by the new
`equal_texts_render_nothing` test rather than left as an argument, so a future
renderer that emits an unconditional header fails a test instead of silently
changing what that guard costs. **Final state: 39 of the 40 viable mutants
killed, the fortieth recorded as equivalent.**

The mutant count is 46, not the 43 the blocked run enumerated, and that is three
mutants arriving rather than a number being corrected: `4a599ed` ("Keep the
reporting shape open to a second input source") added `Inputs::resolve` and the
`Mode::InPlace if is_changed` guard, and `git merge-base --is-ancestor` confirms
that commit is not an ancestor of the tree the 43 were counted on. The earlier
enumeration is kept in place with a note, and the PR body's figure is corrected
to the measured one. The six unviable mutants are default-value substitutions
for types with no `Default` impl (`ExitStatus`, `Inputs`, `Assessment`,
`FileReport`) or an unconstrained generic (`in_argument_order<T>`): they do not
compile, so they are excluded from the denominator rather than counted as
kills, which is why it is 40 and not 46.

The code change is two files, `src/report/delta.rs` and `src/report/render.rs`,
`+29 -1`: one assertion added to each of the two one-sided delta tests, which
pre-existed and already computed the deltas they now also assert on, and one new
test in `src/report/render.rs` whose doc comment states why the guard it pins
exists. Nothing in `src/driver.rs` changes — the accepted survivor is a
statement about the renderer's output, so the pin belongs at the renderer's
boundary.

Gate run, through the gate runner, over this tip: `make check-fmt` passes (6 s),
`make lint` passes (5 s, clippy `--all-targets --all-features` `-D warnings`),
`make typecheck` passes (1 s), `make test` passes (81 s: 45 test binaries plus
the doc-test target, 1861 passed, 0 failed, 20 ignored, the ignored being
doc-tests), `make markdownlint` passes (34 files, 0 errors), and `make nixie`
passes. The test count is Revision 17's 1860 plus the one new test, which is the
only test either change adds — the delta change adds two assertions to existing
tests. The first lint pass failed on a `doc_markdown` finding this revision
introduced, `ExecPlan` unbackticked in the new test's doc comment
(`src/report/render.rs:292`), which was fixed and re-gated rather than argued
away; `make test` is unchanged by that fix, a doc comment being invisible to the
harness. Logs: `/tmp/check-fmt-epm6-mdtablefix-check-option.out`,
`/tmp/lint-epm6-2-mdtablefix-check-option.out`,
`/tmp/typecheck-epm6-mdtablefix-check-option.out`,
`/tmp/test-epm6-mdtablefix-check-option.out`,
`/tmp/markdownlint-epm6-mdtablefix-check-option.out`,
`/tmp/nixie-epm6-mdtablefix-check-option.out`.

### Revision 19, 2026-09-12

This revision is a review-driven round over the same branch after the plan was
declared `COMPLETE`. It changes no milestone, obligation, or acceptance
criterion, and its two code changes are follow-ups on work the milestones
already delivered rather than new scope.

The first is a unit test that reaches the file-rewriting boundary with nothing
in the way of it. `src/io_tests.rs` now calls `rewrite_with` directly with an
identity transform over three cases — CRLF endings, a leading byte-order mark,
and a body with no final newline — writing the fixture to a temporary file and
comparing the bytes read back. `rewrite_with` becomes `pub(super)` for it, which
is the visibility the test actually needs rather than a convenience: the item is
private to `io::replace` and the test module is `io::tests`, a sibling, so a
private item is not nameable from there at all. `register_metrics` is
`pub(super)` for exactly that reason and is reached through the same
`#[cfg(test)] use replace::{…}` line. This is the only test that can separate
the bytes the boundary restores from the bytes a transform produces, because
both public entry points always transform content.

Two of the three cases assert byte-identity and the third deliberately does not.
A non-empty document whose last line is unterminated gains a terminator on the
way out, which ADR 0007 states as "unterminated non-empty file gains one
terminator" and the users' guide states as "is unterminated gains a terminator
and is reported as drift, as `+1 -1`". Asserting byte-identity for that case
would pin the opposite of the crate's contract, so the case table carries its
expected bytes explicitly and the test's doc comment says why; the test passes
with the terminator restored rather than absent.

The second is `tests/document_properties.rs`'s `rewrite_in_place`, which reached
its fixture through ambient `std::fs` on both sides of the command it runs. It
now opens a `cap_std::fs_utf8::Dir` over the temporary directory — a camino
path, `Dir::open_ambient_dir`, the shape `tests/cli.rs`'s
`capability_directory` uses, on which the local helper is modelled — writes
`fixture.dat` and reads it back through that capability, and keeps the host path
for the one thing that cannot inherit a capability: the argument handed to the
subprocess. `std::fs` is gone from the file, and the twelve document-boundary
cases pass unchanged.

The third finding is the formatting one at `#[case::mixed_in_fence(...)]`, and
it is skipped rather than fixed, because the attribute is already what this
repository's rustfmt produces. Its second argument is a 112-column line, which
is what the finding noticed, but running rustfmt over a copy of the file with
this repository's own `.rustfmt.toml` exits 0 and reports no diff, so the
formatter has nothing to change in that file and a hand-reflow would be a
departure from rustfmt rather than a fix. The check was proved non-vacuous by
appending a
deliberately misformatted function to the same scratch copy, where rustfmt
reported that diff and exited 1. `make fmt` itself was not run, unchanged from
Revisions 15 to 17 and for the same reason: it cannot be scoped to a diff, and
ten Markdown files in the tree drift under it.

Gate run, through the gate runner, over this round: `make check-fmt` passes
(2 s), `make lint` passes (4 s, clippy `--all-targets --all-features` `-D
warnings`), `make typecheck` passes (1 s), and `make test` passes (70 s, 1864
passed, 0 failed, 20 ignored, the ignored being doc-tests). The total is
Revision 18's 1861 plus the three new cases, and the log shows each of them
running and passing by name rather than only the total moving. No test was flaky
in this run. Logs: `/tmp/check-fmt-epm7-mdtablefix-check-option.out`,
`/tmp/lint-epm7-mdtablefix-check-option.out`,
`/tmp/typecheck-epm7-mdtablefix-check-option.out`,
`/tmp/test-epm7-mdtablefix-check-option.out`.

### Revision 20, 2026-09-12

This revision is driven by continuous integration rather than by review, and it
changes no milestone, obligation, or acceptance criterion. The Windows job of
this branch's own CI run failed on `44c718e`: `atomic write contract (windows)`,
run `34654449073`, job `103443680234`, with exit 101 and two failing targets,
`--test cli_check` and `--test cli_diff`. Both failures are the same test,
`a_closed_pipe_is_a_successful_early_exit`, and both are the same panic at the
`spawn` expectation:

```plaintext
spawn mdtablefix: Os { code: 206, kind: InvalidFilename,
                        message: "The filename or extension is too long." }
```

The test builds 500 fixture names of 187 characters and passes them all, which
is a command line of about 93 KiB. `CreateProcess` refuses a command line past
32 767 characters, so on Windows the child is never spawned — the failure is in
the test's fixture, not in the tool. Unix permits the shape (`ARG_MAX` and the
per-argument limit are far above it), which is why the Linux job was green and
the divergence went unnoticed until CI ran the whole suite on Windows.

The two tests now take their fixture size from `CLOSED_PIPE_FILES` and
`CLOSED_PIPE_PADDING`, which are per-platform constants in each file: 500 files
of 180 padding on Unix, unchanged, and 250 files of 80 padding elsewhere. The
arithmetic that couples the two numbers is worth stating, because it is what
makes the Unix pair a *measurement* rather than a guess. A `--check` report is
one line per file, and that line is the path the argument named, so the
arguments *are* the output — the report is the name plus the `+N -M` delta. Unix
holds 64 KiB in a pipe, so the run must write past that, and 500 × (187 + 8) is
about 97 KiB: the child is blocked in `write` when the read end closes, which is
what makes the early exit deterministic rather than a race. The Windows command
line caps the same figure near 32 KiB, so a deterministic block cannot be
guaranteed there at all, and the pipe is in any case created with a size hint of
zero — the system default rather than the Unix 64 KiB. The exit-status assertion
is split accordingly: `cfg(unix)` keeps the strict `Some(0)`, and elsewhere the
test asserts that the run ends in a documented status, `0` or `1`, and never in
a panic or a crash. Both platforms keep the assertion that no `panicked` reached
stderr, which is the defect the test exists to catch.

The Windows arm could not be measured here: this estate has no Windows host, so
the assertion compiles only under `cfg(not(unix))` and its first real reading is
the next CI run on the pushed commit. What *is* measured is that the shape the
fix produces is still the shape Unix needs: through the gate runner, `make
check-fmt` passes (2 s), `make lint` passes (1 s, clippy `--all-targets
--all-features -D warnings`), `make typecheck` passes (under 1 s), and `make
test` passes (50 s, 1864 passed, 0 failed, 20 ignored, the ignored being
doc-tests), with `cli_check::a_closed_pipe_is_a_successful_early_exit` and
`cli_diff::a_closed_pipe_is_a_successful_early_exit` both `ok` by name in the
log. Nothing under `src/` changed, so no obligation's evidence and no milestone
outcome is affected. Logs: `/tmp/check-fmt-ci-pipefix-mdtablefix-check-option.out`,
`/tmp/lint-ci-pipefix-mdtablefix-check-option.out`,
`/tmp/typecheck-ci-pipefix-mdtablefix-check-option.out`,
`/tmp/test-ci-pipefix-mdtablefix-check-option.out`.

### Revision 21, 2026-09-12

Revision 20 left one claim open on purpose: the Windows arm of the split
assertion had no local reading, because this estate has no Windows host, so the
CI run on the pushed commit would be its first measurement. That run has now
happened, on `4cfb2d6`, run `34655247077`, and it is green: `atomic write
contract (windows)` passes in 6m22s, and every other job — `build-test` and the
four `binstall packaging` targets — passes with it. The two failures that
motivated Revision 20, `--test cli_check` and `--test cli_diff` exiting 101 at
the spawn, are gone, which is the reading the fixture-sizing fix needed and
could not get locally. Nothing was changed to obtain it; this entry records the
measurement, not a further edit.

### Revision 22, 2026-09-12

CodeRabbit's third review of pull request #464 returned twelve inline findings
against `e8c5b7a`. Nine were valid and are fixed; three are rebutted on their
threads with evidence. Seven of the nine were one-line corrections (an ADR
hyphen, a shell qualification in the user's guide, a `-ize` spelling, two
`rstest` conversions, a fixture's wrapping, a `driver.rs` intra-doc link), and
are recorded by the commits rather than repeated here. The remaining two were
structural and between them decided the shape of this revision.

Both structural findings asked the same thing from different directions: every
file stays inside the 400-line limit, and measured data lives in a module of
its own. That is the boundary each split follows, so no module exists only to
move lines — a split with no such justification would be the "module boundary
that exists only to move lines" the superseded decision below rejected, and
that rejection still stands as reasoning.

| Before | After | Lines |
| --- | --- | --- |
| `src/driver_tests.rs` 485 | `src/driver_contract_tests.rs`, `src/driver_report_tests.rs`, `src/driver_in_place_tests.rs` | 107, 214, 115 |
| the same file's fixtures | `src/driver_test_support.rs` | 83 |
| `tests/cli_check.rs` 542 | root harness plus `tests/cli_check/{arguments,closed_pipe,exit_status,no_write,ordering}.rs` | 104 + 50, 96, 163, 62, 123 |
| `tests/in_place_atomic.rs` 408 | root plus `tests/in_place_atomic/failure.rs` | 270 + 155 |
| `tests/cli_matrix/support.rs` 557 | harness plus `cases.rs` (catalogue) and `support_tests.rs` (unit tests) | 380 + 125 + 84 |

Three consequences are worth recording, because none is visible from the diff
alone.

The derived-file/type boundary is a privacy boundary, not a file boundary.
`src/driver_test_support.rs` is the only module the three driver test modules
share, so every item in it is `pub(super)`: the modules that use them are
`super`'s descendants, and a wider visibility would publish test fixtures to
the binary. Its `#[cfg(unix)]` on `use std::fs` is likewise load-bearing rather
than stylistic — an ungated import is unused on Windows under `-D warnings`,
because every `fs::` use in that module sits inside a `#[cfg(unix)]` test.

The extraction changed formatting the reviewer did not ask about. The unit
tests formerly sat behind `#[rustfmt::skip]` on an inline `mod tests`, and that
attribute does not survive the move to an out-of-line module: rustfmt rewrote
`tests/cli_matrix/support_tests.rs` into its own layout. The result is
cosmetic, the assertions are untouched, and the attribute is gone rather than
left in place doing nothing.

Verification was by conservation rather than by eye. For each split, the check
is the list of non-blank lines present in the original and absent from every
new file; every such line has to be one of the deliberately changed ones (a
module header, a renamed constant). Line counts are the weak check, since a
slice off by one line still "looks" split.

The round also cost a detour worth recording. Rebuilding
`tests/cli_matrix/support.rs` after a compile probe, a `git checkout --` on the
file — intended to drop the probe — restored the *tracked* version and
discarded the split with it. The recovery was mechanical because the other two
files of the split (`cases.rs`, `support_tests.rs`) were untracked and survived,
and because the original is in Git; but the probe was unnecessary, and the
compile check that preceded it had already read the file it was meant to prove
(it failed on the probe, which is the proof).

### Revision 23, 2026-09-12

This revision closes the pre-merge round Revision 22 opened, and it lands on a
rebased branch. `origin/main` had advanced one commit (`7855bca`, the 0.6
migration guide's line-ending section, landed as #475), so the branch was
replayed onto it before anything was measured: 42 commits, one conflict.

The conflict was in `docs/v0-6-0-migration-guide.md`, where both sides had
written a `Line-ending preservation` section. Main's text is the richer of the
two — it names all five new public API items, and records that an
already-formatted document whose endings are consistent is rewritten
byte-identically — and it already carries the `-ize` spellings the replayed
commit was making, so the resolution keeps main's section and that commit
becomes a no-op for the file. Nothing the branch's own version claimed is
absent from main's, so the "keep both intents" rule is satisfied by keeping
one. The other three entities Weave merged into that file were auto-resolved
and were checked for markers before the rebase continued.

The round had four parts.

**Inline comments.** All sixteen review threads on #464 are resolved, and
CodeRabbit's review at 2026-09-12T00:40:19Z approved the branch, so no thread
needed an answer. The sweep for unresolved threads carrying no reply of ours
returns none.

**Pre-merge checks.** Three of the rows the table reported as failing were
already discharged by work this branch had landed and are stale rather than
outstanding: `Testing (Overall)` by `c832beb` (the unit test and the CLI diff
test over an unterminated original), `Unit Architecture` by `8c43328` (the
counts travel on `Assessment`; only the boundaries report them), and
`Observability` by `6362f1c` (bounded run and per-file metrics at the binary's
boundaries). The three that were genuinely outstanding are discharged by:

- `User-Facing Documentation` — `3a26c0f` adds two sections to the users'
  guide's library API notes. `mdtablefix::report` is documented as the pure
  half of `--check` and `--diff`: what `LineDelta::between` counts and why it
  agrees with the rendered diff, how a report line is parsed, and the unified
  diff's guarantees about headers, timestamps, colourization, and the
  unterminated-original marker. `mdtablefix::io::SourceDocument` is documented
  as the document boundary: mark and ending split off before formatting and
  restored after, counting over the body rather than the whole input, and
  rendering as a method so one document's style cannot be applied to another's
  lines. Both sections carry a Rust example.
- `Developer Documentation` — `996d701` states the build and test
  requirements: `make test` is two invocations, the second not redundant
  because `--doc` is the only way to run doc examples, and both deny warnings
  so a warning in a test target or doctest fails the gate. It also records
  `similar`'s part — the line tokenizer behind the delta counts and the
  unified-diff engine — and that the requirement is the 2.x line, with both
  call sites to re-check before a 3.x widening.
- `Testing (Compile-Time / Ui)` — answered rather than actioned, and the answer
  is worth recording because the row asks for something the tool cannot
  express. A `trybuild` fixture cannot reach the strict scenario validation:
  `#[scenario]` resolves its feature path against `CARGO_MANIFEST_DIR`, while
  trybuild compiles fixtures in a synthetic project under
  `target/tests/trybuild/` holding only `Cargo.toml`, `Cargo.lock`, and
  `main.rs` — there is no feature file anywhere beneath it — and the
  attribute's arguments (`path`, `index`, `name`, `tags`) offer no inline-text
  escape. The validation such a fixture would pin is already a hard compile
  error, which was measured rather than asserted: adding an undefined step to
  `tests/features/check_mode.feature` fails the build with `error: No matching
  step definition found for 'Then a step no one has defined'`, listing the
  definitions that do exist and pointing at the binding that names the
  scenario.

That measurement turned up the round's only code change, and it is a defect
worth the space. `#[scenario]` reads the feature file itself rather than
through `include_str!`, so Cargo fingerprinted the test target by its bindings
alone: editing a feature file on its own left the previously expanded binary in
place, and the probe compiled cleanly until the bindings were touched by hand.
Strict validation could therefore be missed in an incremental build while a
clean build in CI still caught it. `96e4596` adds a `const _` array that
`include_str!`s both specifications, which restores the missing dependency:
after it, the same probe fails with no file other than the feature file
touched, and all fourteen scenarios pass with the feature files restored.

**A gate failure, taken as a finding.** The first gate pass over the round was
red on `make lint`: `src/metrics.rs`'s `record_file` took `FileOutcome` by
value, and Clippy's `needless_pass_by_value` rejected it because the body never
consumes the enum — it matches on it, and for the failing case copies out the
`&Error`. `12c4f7b` takes the outcome by reference, which is what the caller
needs as well, the analysis's result outliving the recording of it. The
`check-fmt` and `test` gates passed in that same pass, so the failure was one
lint and nothing else.

**Gate evidence.** `scrutineer` ran the six gates sequentially over `1fce083`,
capturing each to `/tmp/<gate>-premerge-mdtablefix-check-option.out`. Five were
green:

- `make check-fmt` — `cargo fmt --all -- --check`, no output.
- `make lint` — `cargo clippy --all-targets --all-features -- -D warnings`,
  clean.
- `make typecheck` — `cargo check --all-targets --all-features`, clean.
- `make test` — 46 test binaries, 1884 passed, 0 failed, 20 ignored, then the
  separate `--doc` run. `tests/bdd_reporting.rs` contributes the fourteen
  scenarios.
- `make nixie` — every diagram validated.

Because that run's only red gate was a markdown one that the tree then moved
past, the whole set was run again over the pushed head `cdcec23`, with fresh
logs at `/tmp/<gate>-regated-mdtablefix-check-option.out` so nothing could
overwrite the earlier evidence. All six are green there: `check-fmt` 2s, `lint`
0s (warm), `typecheck` 1s, `test` 46s over both invocations — 45 test binaries
plus the doc-test suite, 1884 passed, 0 failed, 20 ignored, all of the ignored
in the doctest run — `markdownlint` 34 files and 0 errors, and `nixie` with
every Mermaid diagram validated. That is the record the queued review will be
judged against.

**A second gate failure, taken as a finding.** `make markdownlint` was red over
the same tree — 5 errors across 34 files — and both causes were this round's own
documentation work rather than anything pre-existing. The build-and-test table
`996d701` added had one row wider than its separator, so its pipes did not align
with the header (MD060, two errors), and the rebase's conflict resolution in the
migration guide left a second blank line before three headings (MD012, three
errors): removing the weave markers also removed the hint line that had
separated them, and the blank line above it stayed. Main has a single blank in
all three places, so this was repair of our own conflict resolution, not drift
inherited from main. `3eb244e` widens the column to its longest cell and
collapses the three blank-line pairs, with no wording changed; `make
markdownlint` then reports 34 files, 0 errors, and that is the state the gate
evidence above is measured against.

**The reconciliation, posted.** The row-by-row answer went to #464 as an issue
comment
([#issuecomment-5642414743](https://github.com/leynos/mdtablefix/pull/464#issuecomment-5642414743)),
because the pre-merge table lives on the walkthrough issue comment rather than
on a review, so a reply on a thread would not reach it. It names the three stale
rows and the commits that discharged them, the two documentation rows and their
commits, and the `Testing (Compile-Time / Ui)` rebuttal with the manifest-dir
evidence, the measured compile error, and the fingerprinting defect that
measurement found. The `Ignore` checkbox was deliberately left unticked.

**Push.** The rebase rewrote the branch's history, so the push needed
`--force-with-lease`: `52ea246...3eb244e` forced update, and the remote now
matches the local head, which stands 44 commits ahead of `origin/main`. Two
further commits followed — the markdownlint fix and this record — and both went
up on the normal path.

**The pull request description** was brought level with the tree before the next
review was queued, because a review reads it: the rebase bullet now names the
base the branch actually sits on, and the test tally in `Gates` is the measured
`1884 passed, 0 failed, 20 ignored across 46 test binaries` plus the separate
`--doc` run. A `Pre-merge checks` section records the reconciliation above.

**Next review.** One is queued against the current head — `590209f4`, posting
in about 22 minutes — because a new review is the only thing that refreshes the
pre-merge table, and every row has now been either discharged or answered. The
round this revision closes is therefore complete: no inline thread is
outstanding, the table has been reconciled and answered, and the six gates are
green. What the queued review reports decides whether the loop continues or
ends.

### Revision 24, 2026-09-12 — the review round at `db64399`

**Ten findings, one rebutted.** The review queued as `590209f4` arrived as
`5184665530`, `CHANGES_REQUESTED` at 2026-09-12T01:43:18Z, with the inline
count moving from 27 to 37: ten new top-level comments, every one anchored to
the then-head `db64399` and every one with `reply_to=none`. Each was verified
against the tree before anything was changed. Nine are valid and are fixed in
`6238940`; the tenth rests on a premise that does not hold and is rebutted
rather than actioned.

**The display path, and a test that discriminates it.** The strongest finding
was that the line-ending report named `storage_key` — the bare name a
capability reads by — so `docs/a.md` and `examples/a.md` emitted the same
`path="a.md"` field and a subscriber could not correlate them. `analyse` now
reports `display_path`, which is what `main.rs` already carries for the report
line and the diff headers, so all three agree. The pin is
`check_reports_the_path_the_user_wrote` in `src/driver_report_tests.rs`, and it
was checked in both directions: reverting the argument to `storage_key` makes
it fail with "the line-ending report must name the path the user wrote", and
restoring it makes it pass. The fixture is nested precisely so the two names
differ — with equal names the assertion would hold whichever one the report
used, which is the shape that made the old code look right.

**The critical one, which only CI could confirm.** `identity` in
`src/driver_in_place_tests.rs` is used by a single test, and that test is
`#[cfg(unix)]` because it observes the write through the inode. The import was
not, so on Windows it was an unused import under `-D warnings`. The finding's
line numbers were verified against `db64399` before the fix (import at line 18,
use at line 89, `#[cfg(unix)]` at line 77) rather than taken on trust. The group
could not be gated whole: the other five names stay live on Windows through the
ungated `in_place_writes_the_formatted_text`. There is no Windows host here, so
the `atomic write contract (windows)` job is this fix's first reading.

**Four more, all small.** The fixture builder wrote its file with `std::fs`
beside the capability and so gave the test a different view of the file from
the one the modes under test are handed; it now writes through the capability
with `directory.write`, and creates the parent directory that the new nested
fixture needs. `record_file` matched its outcome labels inline and called
`mode_label` twice, once per instrument; both are bound once and the match
moves to `file_outcome_label`, beside the two label helpers it now matches.
`category`'s doc claimed `declined` covers a path that is not UTF-8, which
`Inputs::resolve` rejects at the command line before `run_files` and which
carries no `io::Error` for the chain walk to find in any case — the only
`InvalidInput` a file's analysis produces is the rewrite boundary's symlink
refusal. ADR 0009 said `--check` exits `2` when a file could not be "read or
rewritten", and `--check` never rewrites.

**Spelling, and one deliberate divergence.** `Normalise` becomes `Normalize`
in the three descriptions of the one `--fences` flag — the clap help text, the
users' guide row, and the `Options` doc — because `src/fences.rs` already used
`-ize` and the policy requires it. The `normalise_event_lines` identifier and
its many call sites are pre-existing and untouched, so the change stays
proportionate to this feature's documentation.

On the ADR wording the finding's remedy was not taken: it proposed "could not
be assessed", and the sentence is a paraphrase of the error the mode emits,
which `analyse_one` builds from `mode.verb()` — `"reading"` for `--check`,
`--diff`, and the bare mode. "Assessed" would name the function rather than the
operation the user sees. The reply says so explicitly rather than substituting
silently.

**The rebuttal.** The tenth finding claimed that an `--in-place` failure counts
towards `errored` and is then reported by `render_summary` as "could not be
read". The count is mode-independent, but the rendering is not:
`src/main.rs:227` calls `render_summary` under `if mode.reports()`, and
`Mode::reports` is
`matches!(self, Self::Check | Self::Diff)`, so no summary is printed for
`--in-place` or for a bare invocation. In the two modes that do print it, the
read is the only step that can fail, because `analyse`'s `write_back` call sits
on the `Mode::InPlace` arm alone. The suggested wording would therefore make the
summary less precise for every mode that can display it.

**The tautological test.** `non_wrap_signature_ignores_wrap_variant` asserted
`f(x) == f(x)` over two local booleans that neither operand used, so it could
not fail. It is replaced by `non_wrap_signature_encodes_the_fixture_and_flags`,
which pins the exact string for three flag lists including the empty one. The
wrap invariance its old name promised is not duplicated here: it is already
asserted over the real matrix by
`tests/cli_matrix.rs::matrix_cases_expand_to_wrapped_and_unwrapped`, which
groups `logical_cases()` by this signature and requires every group to hold
both wrap variants — a stronger statement than a hand-made pair of booleans.

**The threads.** Every one of the ten carries a reply tagging `@coderabbitai`,
nine naming `6238940` and the tenth stating the rebuttal with the call-site
evidence. Two of the ten were still unresolved when the replies went out — the
ADR wording, which the reply answers, and `src/main.rs:228`, which is the
rebuttal — and a GraphQL sweep afterwards found all 26 threads on the pull
request resolved, with none left unanswered.

**Gates.** The full set was run through `scrutineer` over `6238940` with a clean
worktree: `check-fmt` 2s, `lint` 4s, `typecheck` 0s, `test` 60s — 46 binaries,
`1885 passed, 0 failed, 20 ignored`, one more than the previous run, which is
the new tracing test — `markdownlint` 34 files and 0 errors, and `nixie` with
every diagram validated. The one advisory is non-fatal: `nixie` logs
`--> line 89: <unknown>` for `docs/architecture.md`, which was already the case
before this round.

### Revision 25, 2026-09-12 — the pre-merge checks at `6238940`

**When the table may be read.** The walkthrough's check table is refreshed only
when a review runs, and the comment carrying it is edited in place rather than
re-posted, so it was read from the live body at
`pre_merge_checks_walkthrough_start` rather than from an earlier round's copy.
It reported `2 errors, 3 warnings` against the then-head `6238940`. Per the
skill's ordering it was left alone until every inline thread had been answered,
and every row was then reconciled against the current tree before anything was
actioned — the table is generated from a commit that work has moved past, and
two of these rows had already been answered once in an earlier round.

**Five rows, three actioned.** Each was fixed in `be41510`, a commit whose only
subject is the reconciliation.

*Developer Documentation* (⚠️ Warning) held. `docs/developers-guide.md` headed a
list "`src/main.rs` file-output functions:" while `format_lines` and
`formatting_closure` had moved to `src/command.rs:131` and `src/command.rs:140`,
a later bullet still placed `formatting_closure` in `src/main.rs`, and the guide
documented the library's `mdtablefix_io_*` instruments but none of the binary's
`mdtablefix_file_*` and `mdtablefix_run_*`. The headings now say what
`src/command.rs` owns and what stays in `src/main.rs`, the line-ending bullet
says the report names the display path, and a `#### Binary metrics` subsection
documents the four instruments, their bounded label sets and error categories,
and the analysis span.

*Testing (Overall)* (❌ Error) held. The induced `--in-place` write failure was
asserted as "any non-zero": `tests/in_place_atomic.rs` used `.failure()`, and
the three Unix-only cases in `tests/in_place_atomic/failure.rs` used
`!output.status.success()`. The three-valued exit contract this branch
introduces was therefore unasserted exactly where a per-file failure is
induced. All four now assert code `2`, and
`in_place_reports_drift_and_a_write_failure_as_an_error` pins the combination
the contract has to order: drift in one file plus a write failure in another
exits `2` rather than `1`, while the drifted file is still rewritten. The
byte-preservation and no-temporary-file assertions are kept.

*Observability* (⚠️ Warning) held, and it was the round's one real gap.
`AGENTS.md` asks for spans around work at the boundaries and the developer guide
states that a target `path` appears only as a tracing span field;
`src/io/replace.rs:163` had such a span and the binary's per-file analysis had
none. `record_analysis` now opens a `debug` span carrying `mode` and the
display `path` on entry, and records `outcome` and `elapsed_seconds` once the
analysis has run, under the same names and values the counters use.

**The span's missing half, and the statuses the row called unguarded.** Two
follow-ups landed in `7455e9b`, both from re-reading a row's own words against
the tree rather than from a new finding.

The Observability row also asks for "a bounded error category and completion
outcome" — and the bounded category existed only on
`mdtablefix_file_error_total`, so a host filtering a trace could see that a file
failed but not under which of the four names. `record_analysis` now emits
`analysis failed` at debug level inside the span, carrying `error_category` from
the same `category` function the counter labels with — derived from the
`io::ErrorKind` in the error's chain, never from its message — so a span filter
and a metric filter select the same failures by the same name. A successful
analysis emits nothing, which is what lets a host filter on the event at all.
`a_failed_analysis_names_its_category_in_the_trace` asserts the event and its
category, and `a_successful_analysis_emits_no_failure` asserts its absence.

The Testing (Overall) row's summary sentence was that "the changed exit-status
contract is not fully guarded", and three per-file failure assertions were
still written as "any non-zero": the declined-symlink case in
`tests/in_place_atomic.rs`, and both missing-file cases in `tests/parallel.rs`.
Each induces a failure the contract reserves `2` for, so each now requires
exactly `2`. The suite's induced per-file failures are uniform — read-only
directory, `ulimit -f 1`, occupied candidate names, declined symlink, and the
two missing-file cases — and the guide records the event, so the metric, the
trace, and the documentation name the same failures.

**Two rows answered rather than actioned.**

*Unit Architecture* (❌ Error) asks for "a fallible read-only assessment
operation that reads through `ReadOnlyDir`, formats the document, and returns an
`Assessment` without logging, metrics, rendering, or writes". That operation
is `driver::assess(&ReadOnlyDir, storage_key, format) ->
anyhow::Result<Assessment>` at `src/driver.rs:218`, which is exactly those
things: it returns the counts on
the assessment, emits nothing, and cannot write because `ReadOnlyDir` has no
write method. `analyse` is the rendering half layered on top of it. Moving
rendering out of `analyse` would contradict the property the design documents —
retained memory is proportional to the rendered payload rather than to twice the
whole input — so the reply points at the seam and asks for a concrete
counter-proposal rather than performing a refactor the design argues against.

*Testing (Compile-Time / Ui)* (⚠️ Warning) is the row the previous round already
answered, and the answer has not changed: a trybuild fixture cannot compile a
`#[scenario]` binding, for two structural reasons. The macro resolves the
feature path against `CARGO_MANIFEST_DIR`, while trybuild compiles fixtures in a
synthetic project under `target/tests/trybuild/` that contains no feature file
anywhere beneath it; and `#[scenario]` accepts only `path`, `index`, `name`, and
`tags`, so the feature text cannot be inlined in the fixture either. The
validation such a fixture would pin is already a compile-time gate, and that was
measured rather than argued: adding an undefined step makes the build fail,
which is how the missing feature-file fingerprint dependency was found and
fixed.

**Gates.** Six gates green at `7455e9b`, run through `scrutineer` with a clean
worktree: `check-fmt` 2s, `lint` 1s, `typecheck` 1s, `test` 50s — 46 result
lines, `1888 passed, 0 failed, 20 ignored`, with `tests/in_place_atomic.rs` 11
of 11 and `tests/parallel.rs` 4 of 4 — `markdownlint` 34 files and 0 errors, and
`nixie` with every diagram validated. The tally moves from 1886 to 1888 because
this round adds the two tracing tests for the failure event; a run over the same
tree before the three assertion tightenings was green at the same tally, since
tightening an assertion adds no test.

**The reconciliation, and the push.** The row-by-row answer is posted as an
issue comment on the pull request (`5642773695`), tagging `@coderabbitai`, with
the `Ignore` checkbox deliberately unticked. `7455e9b` is pushed to
`origin/check-option`, which stands 50 commits ahead of `origin/main`. A thread
sweep before that found 26 threads and none unresolved; seven carry no reply of
mine, of which six are outdated and one — `tests/cli_matrix/invariants.rs:79`,
asking for `rstest` cases over `contains_table_delimiter` — was self-resolved by
the bot's "addressed in commits" annotation, and the pinning test it asked for,
`contains_table_delimiter_needs_a_pipe`, is in the tree at line 155.

**Next review.** `d5e31dc7` is queued and posts in about 22 minutes, at
2026-09-12T02:33Z, so findings are expected from roughly 02:48Z. A review is the
only thing that refreshes the pre-merge table, and every row of this round has
been either discharged or answered, so what it reports decides whether the loop
continues or ends. The pull request description was brought level with the tree
before the review was queued, because a review reads it.

### Revision 26, 2026-09-12 — the refreshed table, and a row it derived anew

**The ask.** "Please ensure that the execplan has been updated with decisions,
findings, observations and progress to reflect the current implementation
status. Please also update the PR description to reflect the implementation."
This revision is the first half of that; the description is the second, and it
is rebuilt from the gate log by the same generator as before rather than by
hand.

**The queued review ran and said almost nothing.** `d5e31dc7` posted at
2026-09-12T02:33:30Z as comment `5642894586` from `wafflecat-df12`. Its two
replies — "Review triggered." at 02:33:36Z and "Review finished." at
02:33:37Z — are one second apart, and the second carries the note that
CodeRabbit is an incremental review system and does not re-review already
reviewed commits. No inline comment has been posted since the ten of
`5184665530` (01:59:21Z–01:59:37Z, ids `3994699930` to `3994701183`), so the
inline phase closed in Revision 24 stays closed: 26 threads, none unresolved,
and nothing new to answer.

**The table refreshed at 02:39:34Z.** The pull request's walkthrough comment
(`5603998327`, identified by its `updated_at`) was edited in place and now
reports `2 errors, 1 warning`, a warning fewer than the `6238940` table.
*Observability* and *Developer Documentation* have moved to the passed table:
the span, its bounded event, the field table's `error_category` row and the
`#### Binary metrics` section are the work they asked for. *Unit Architecture*
and *Testing (Compile-Time / Ui)* are unchanged, and are the two rows the round
at `7455e9b` answered rather than actioned.

**A row that was re-derived, not inherited.** *Testing (Overall)* still fails,
but it no longer says what it said. Its `6238940` explanation was that "the
changed exit-status contract is not fully guarded"; the refreshed one is that
`write_unified_diff` "documents that it returns output-writer errors, but no
test supplies a failing `io::Write` implementation or asserts error
propagation". The exit-status complaint is gone precisely because every
induced per-file failure now requires code `2` — a change made in `7455e9b`,
after the table that raised it. So the refresh was generated from the current
tree rather than carried over, which settles how the two surviving rows should
be read: they are the bot's position after seeing the fixes, not staleness.
The answers posted to them at 02:10:40Z (`5642773695`) predate the refresh, so
they cannot have moved it either way.

**One row actioned, in `530bdbd`.** The renderer's writer-error row is valid
and was genuinely unaddressed. `write_unified_diff`'s `# Errors` section
promises the writer's failure is returned, and both direct renderer tests wrote
into a `Vec`, which cannot fail.
`a_failing_writer_error_reaches_the_caller` supplies a writer with a byte
budget and asserts the returned error's kind. Two cases: a writer that is
already closed (`budget = 0`) and one that fails inside the body, after the
headers have been accepted (`budget = 16`). Both pass, which is itself the
measurement — `similar`'s `to_writer` propagates the writer's own error
unchanged rather than wrapping it. Asserting on the error's `kind` rather than
on "some error" is what makes that a result: an implementation that replaced
the writer's error with one of its own would fail the test.

`src/report/render.rs` stood at 354 lines and the new test took it to 409, past
the 400-line limit in `AGENTS.md`. The whole test module therefore moved to
`src/report/render_tests.rs`, the sibling-file convention `src/metrics.rs`,
`src/driver.rs` and `src/io.rs` already follow, leaving `render.rs` at 201
lines and the test file at 209. The module path is unchanged, so the insta
snapshot's directory and name are unchanged, and the snapshot test passing
after the move is what shows it.

**A correction to the previous segment's reading.** A CodeRabbit comment at
04:22:47Z reading "Review skipped — Bot user detected. To trigger a single
review, invoke the `@coderabbitai review` command" was taken to mean this pull
request's automatic review had been skipped. It had not: resolving
`5643426668`'s `issue_url` shows issue **#495**, a different pull request in
the same repository. The mistake came from reading a repository-wide
`issues/comments` query, which mixes every pull request's comments together.
The walkthrough query has the same hazard in the other direction — several
later comments in that list carry the walkthrough marker for other pull
requests, so `last` alone picks the newest walkthrough in the repository rather
than the one for this pull request. Resolve `issue_url`, or use the PR-scoped
endpoint, before acting on a comment's body. No review was skipped here, and
none of this round's work is a remedy for one.

**The response, and the next review.** The row-by-row answer to the refreshed
table is posted as an issue comment tagging `@coderabbitai`, with the `Ignore`
checkbox left unticked, and `530bdbd` is pushed to `origin/check-option` once
the gates below are green — the queue is only asked for a review after that,
per the ordering the skill sets out. Every inline thread is answered, one row
of the table is actioned and two are answered with reasons, so what the next
review reports on those two decides whether the loop continues or ends.

**Gates.** All six deterministic gates are green at `530bdbd`, run sequentially
through `scrutineer` over a clean worktree: `check-fmt` 4s, `lint` 7s with the
`check-static-regexes` prerequisite met, `typecheck` 2s, `test` 98s over its two
cargo invocations, `markdownlint` 34 files and 0 errors, and `nixie` with every
diagram validated. The tally is `1890 passed, 0 failed, 20 ignored` across 46
result lines — two more than Revision 25's 1888, which is exactly the new
test's two rstest cases, and the snapshot test's green after the module moved
is what shows the insta path survived the split.

**The push, the description, and the reply.** `530bdbd` is pushed to
`origin/check-option` before the queue is asked for anything. The pull request
description is rebuilt from that same gate log by the generator the previous
rounds used, which refuses to publish a tally carrying a failure or fewer than
1800 passes; it gains a "Third round" section recording the refreshed table,
the row actioned, and the two rows left answered, and it no longer points a
reader at `src/report/render.rs` for the renderer's tests. The row-by-row
answer to the refreshed table is posted as an issue comment tagging
`@coderabbitai`, with the `Ignore` checkbox left unticked.

**The queue.** `4966887b` is queued for `leynos/mdtablefix#464`, posting at
2026-09-12T11:05:41Z (it reported 22m03s at 10:43:38Z), so findings are expected
from roughly 11:20Z and the next check-in is set for 11:25Z. The queue was
empty when it was added, and the two entries before it — `590209f4` and
`d5e31dc7` — are the rounds this revision and Revision 25 record.

### Revision 27, 2026-09-12 — the Windows job's dead imports

**The finding, and it was real.** A blocking defect was reported against
`tests/cli_check/arguments.rs`: lines 3–7 import `std::fs`, `tempfile::tempdir`
and `super::{CLEAN, run_in_os, status_of, stderr_of, stdout_of}` without a
platform gate, while the only consumer, `a_non_utf8_path_argument_exits_error`,
is `#[cfg(unix)]`. That makes every one of them dead on a non-Unix target, and
the repository's Windows job sets `RUSTFLAGS: "-D warnings"`, so dead imports
are errors there. The report also quoted GitHub's own state: `build-test`
success, `atomic write contract (windows)` failure, merge state `UNSTABLE`.

**Verified against the tree and against CI, not taken on trust.** The CI log
was fetched rather than inferred. Run `34667046126` failed in the Windows job
with exactly those three errors — `tests\cli_check\arguments.rs:3:5`, `:5:5`
and `:7:13`, the third naming all five helpers — and `Process completed with
exit code 101`. That run also shows why the failure is easy to misread: the
job's first step runs the library tests and two named suites and passes with
936 + 2 + 2 tests green, and the failure lands in the second step, when the
whole suite is compiled and `cli_check` is built for the first time.

**The local instrument.** There is no Windows host here, and this class of
defect is invisible to the Linux gates by construction: on Unix the test runs,
so its imports are used, and only a build *for another target* can see them as
dead. The Windows target is installed in this environment, so the job's own
configuration can be reproduced locally except for the running of the tests:
`RUSTFLAGS="-D warnings" cargo check --target x86_64-pc-windows-msvc
--all-targets --all-features`. It fails with those same three errors before the
change and exits 0 with no warnings after it. `--all-targets` is the part that
matters — the defect lives in a test binary, which a default `cargo check`
would not build.

**The fix, in `3737276`.** The three imports are gated with `#[cfg(unix)]`
rather than the module declaration being gated in `tests/cli_check.rs`. The
sibling module is the precedent: `tests/cli_check/closed_pipe.rs` also gates
inside the file, keeping a `#[cfg(not(unix))]` arm beside the Unix one, so a
platform's cases can live next to a portable neighbour. Gating the declaration
would make the whole file vanish on other platforms, and a later portable case
added to it would silently not compile there.

**This is the second instance of one pattern, which is the part worth
recording.** Revision 24 fixed an identical defect: `identity` in
`src/driver_in_place_tests.rs` was imported ungated and used by a single
`#[cfg(unix)]` test, so it was dead under `-D warnings` on Windows. Both were
introduced by a *refactor* — that one by a test split, this one by `ba84de9`,
which carves four test files out of one and moved the non-UTF-8 case into a
file whose header imports what only it needs. Neither the Linux gate nor the
review could see it; only a build for the other target can. If a third
instance appears, the cross-check above belongs in the Makefile as a gate
rather than in a plan revision as a command, because the failure mode is
silent everywhere else. It is not added here: the CI job already runs it on
every push, it needs a target that a fresh checkout has not installed, and this
round's request was to fix the defect and re-run the gates.

**Gates.** All six are green at `3737276` over a clean worktree, run
sequentially through `scrutineer`: `check-fmt` 2s, `lint` 1s, `typecheck`
under a second, `test` 51s — 46 result lines, `1890 passed, 0 failed,
20 ignored` — `markdownlint` 34 files and 0 errors, and `nixie` with every
diagram validated. The tally is unchanged from `530bdbd`, which is what this
commit should do: it gates three imports and adds no test.

**What confirms it, and what cannot.** The cross-check above is the compile
half of the Windows job and nothing more; no test can be run for another
target from here. The push starts the real job, and its conclusion is the
next reading this revision will carry.

### Revision 28, 2026-09-12 — the callsite that went silent

**The compile fix worked; the job failed somewhere else.** Run `34689423901` on
`64117e4` gets past compilation — the dead-import errors of Revision 27 are
gone, and every job in the matrix but one is green, `build-test` included. The
`atomic write contract (windows)` job fails in its **first** step, "Test the
atomic write contract", with:

```text
src\wrap\tests\fence_tracker_logging.rs:37:5:
assertion failed: logs_contain("transition=\"matching_close\"")
test result: FAILED. 937 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

`Test the whole suite` is skipped, and the two named suites after the library
binary pass (2 + 2). So one test assertion is the whole of the remaining
failure, and it is a *missing log line*, not a wrong one.

**Three of its siblings passed, which is what identifies the cause.** The four
tests in `fence_tracker_logging` are the same shape, run in the same binary,
within the same second, and share one production function: `FenceTracker::
observe_parsed`, which holds five separate `debug!`/`trace!` invocations —
`implicit_close`, `matching_close`, `unchanged` twice, and `open`
(`src/wrap/fence.rs:177,209,224,235,246`). The run reads:

```text
10:51:04.015  fence_opening_logs_content_free_transition ... ok
10:51:04.093  depth_decrease_logs_content_free_implicit_closure ... ok
10:51:04.164  incompatible_marker_logs_content_free_unchanged_transition ... ok
10:51:04.318  matching_fence_closure_logs_content_free_transition ... FAILED
```

`fence_opening` asserts on the `open` callsite, `depth_decrease` on
`implicit_close`, `incompatible_marker` on the second `unchanged`, and all three
passed. A span, buffer, or thread-locality fault would affect these four
identically — same macro, same span name mechanism, same global mutex buffer.
The only thing that distinguishes the failing test is *which callsite* it
asserts on. That is what an interest cache keyed per callsite does, and it is
why the mechanism below is the one worth measuring.

**The mechanism, from the primary sources.** `tracing-test` installs its
subscriber lazily, in the first traced test the harness reaches
(`INITIALIZED.call_once` in `tracing-test-0.2.6/src/internal.rs`). `tracing`
decides once, when a callsite is first used, whether that callsite can ever be
dispatched, and caches the answer in the callsite's own static. The dangerous
answer is `Interest::never()`, and `tracing-core-0.1.36` produces it whenever
the dispatcher set cannot be consulted: `DISPATCHERS.rebuilder()` yields
`JustOne`, which calls `dispatcher::get_default()`, which returns `&NONE` while
`GLOBAL_INIT` is not `INITIALIZED`. `dispatcher::set_global_default`
(`dispatcher.rs:299`) never recomputes the cache: it sets `INITIALIZING`,
swaps in `GLOBAL_DISPATCH`, stores `INITIALIZED`, and returns. `Interest::and`
does not rescue it either — `never` combined with anything is `never`. So a
callsite that is first used while no global dispatcher is installable caches
`never` permanently, and `event!`'s gate — `level_enabled! && { let interest =
__CALLSITE.interest(); !interest.is_never() && … }` — silently drops every
event from that site thereafter, for the life of the process.

**Measured, not inferred.** Before any test code was touched, the hypothesis was
handed to `alchemist` for falsification, with a minimal probe at
`/home/leynos/scratch/callsite-probe/`. Its one test registers a callsite with
no dispatcher in place, installs one, and prints the subscriber's event count at
each step: `before_install=0 after_install=0 after_rebuild=1`. Verdict:
**not falsified**. The install does not heal the callsite; the documented remedy
`tracing_core::callsite::rebuild_interest_cache()` (`callsite.rs:222`) does.
The probe uses the same `tracing` 0.1.44 / `tracing-core` 0.1.36 pair this
crate resolves to, and `tracing-test` 0.2.6 is the newest release, so there is
no upstream fix to take.

**The fix, in `9834fcb`.** A `traced_test` attribute in the existing
`test-macros` dev-dependency prepends
`::tracing::callsite::rebuild_interest_cache();` to the function body and
re-emits `#[::tracing_test::traced_test]`. `tracing-test` prepends its own
initialization to whatever body it is handed, so the rebuild always runs *after*
the install — the ordering is structural, not a matter of which statement the
test author writes first. All 16 traced sites use it: 13 `use
tracing_test::traced_test;` imports swapped for `use test_macros::traced_test;`,
and the three fully qualified sites (`src/ellipsis.rs:324`,
`src/main_tests.rs:247`, `:274`) rewritten as `#[test_macros::traced_test]`.
Each site carries a one-line pointer to `test_macros` rather than the full
rationale, which lives in the macro's doc comment and in the `§2.3
test-macros` section of the developer's guide.

**Alternatives considered and dropped.** Bumping `tracing-test` is not
available (0.2.6 is current). A `#[ctor]`-style pre-main install would add a
dependency and reach for `doc(hidden)` internals for the same effect. Writing
the heal call into each test body by hand is what the wrapper exists to avoid,
since it puts the ordering back in the author's hands. `--test-threads=1` hides
the race without addressing it, and would slow every gate. The wrapper adds no
dependency, changes no production code, and is scoped to test infrastructure.

**Gates.** All six are green at `9834fcb` over the worktree, run sequentially
through `scrutineer`: `check-fmt` 2s, `lint` 5s with `check-static-regexes`
met, `typecheck` 4s, `test` 67s — 46 result lines, `1890 passed, 0 failed,
20 ignored`, the same tally as `530bdbd` because this commit adds no test —
`markdownlint` 34 files and 0 errors, and `nixie` with every diagram validated.
Two extras were run because the change reaches past the root package:
`cargo fmt --manifest-path test-macros/Cargo.toml -- --check` exits 0, since
`cargo fmt --all` does not cover a crate the root package has no `[workspace]`
for, and the Windows cross-check
`RUSTFLAGS="-D warnings" cargo check --target x86_64-pc-windows-msvc
--all-targets --all-features` exits 0 with no warnings.

**The attribution this revision does not yet have.** The mechanism is measured
in isolation, and the per-callsite symptom points at it; that the *specific*
Windows interleaving reaches the window is an inference, and the honest test of
it is the job itself, which is running on `9834fcb` as this is written. If the
job goes green, the reading is that the rebuild heals whatever poisoned that
callsite — the remedy is a superset of the diagnosis, since every traced test
now recomputes the whole cache before emitting anything. If the job fails again
on the same assertion, the inference was wrong and the next revision records
what the failing interleaving actually does.

**The review, meanwhile, was paused rather than answered.** Review `4966887b`
posted at 11:05:41Z and then reported `Review paused`: "It looks like this
branch is under active development", which is CodeRabbit's own throttle on
commit volume, not a finding. It posted no inline comment — the newest is still
`3994701183` at 01:59:37Z — and it did not regenerate the pre-merge table, whose
`updated_at` moved to 11:06:02Z only because the pause notice was appended to
the same comment. The table's three rows are the ones Revision 26 recorded, and
each was reconciled against the tree again: *Testing (Overall)* is the row
`530bdbd` actioned, and the work is present
(`src/report/render_tests.rs:15` defines `FailingWriter`, `:175` is
`a_failing_writer_error_reaches_the_caller`, asserting the returned error's kind
so a substituted error fails the test); *Unit Architecture* and *Testing
(Compile-Time / Ui)* are the two rows answered with reasons at `7455e9b`, and
nothing in this commit changes either position. No thread is unresolved and
none is unanswered. `@coderabbitai resume` is the documented way out of the
pause, and it is worth asking for once the branch is quiet rather than now,
while commits are still landing.
