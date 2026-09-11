# Add `--check` and `--diff` reporting modes to the `mdtablefix` CLI

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: IN PROGRESS

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
- [ ] EP-M4 `--diff`, sharing `--check`'s exit semantics.
- [ ] EP-M5 Curated CLI matrix coverage for the two new modes.
- [ ] EP-M6 Targeted mutation testing of the counting and aggregation
      functions.
- [ ] EP-M7 Documentation, ADRs, changelog, and issue closure.

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

## Outcomes & retrospective

Not started. Complete at each milestone boundary and before setting the plan
to `COMPLETE`, reconciling every discovery against `Conformance basis`.

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
`insta`; `RunResult::envelope` at `tests/cli_matrix/support.rs:218` builds a
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
- `docs/developers-guide.md`: internal API reference, the "callers select the
  function that matches their intent rather than passing a Boolean mode flag"
  convention at `:111-113`, the CLI matrix harness, and observability.
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

### Obligations

- **INV-PREDICTS**: for every input and every combination of the CLI's eight
  transform flags, `--check` exits `1` if and only if running `--in-place`
  over an identical copy changes that copy's bytes, and the reported counts
  equal the delta between the copy's before and after bytes.
  Method: property test that actually runs both paths over two copies.
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
  combining-mark content, since table padding is width-sensitive.
  Artefact: `tests/check_properties.rs`, extending the shape of
  `src/main.rs:252-301` but not its weak generator (six fixed words, one table
  shape, all options false).
  Evidence: `cargo test --test check_properties`.
  Non-vacuity: assert the generator produced both drifting and clean cases and
  reached each of the eight flags. Negative control: make `--check` compare
  trimmed strings; a trailing-newline case must fail.

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
  Artefact: `tests/cli_diff.rs`.
  Evidence: `cargo test --test cli_diff deterministic`.
  Non-vacuity: negative control is enabling `TextDiffConfig::timeout`, which
  must make the test flaky or fail.

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
report format and are rejected as operational errors.

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

/// Writes the formatted text back. Only reachable from [`Mode::InPlace`].
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
#[command(group(clap::ArgGroup::new("mode").multiple(false).requires("files")))]
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
the determinism test passes over ten runs; `--diff` over a drifting file exits
`1` and over a clean file exits `0`. The first draft's criterion that `patch`
reproduce the file is dropped, per `Decision log`, which also removes an
undeclared external tool dependency from the suite.

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
9. Run `make fmt`, then `make markdownlint`, `make nixie`, and all Rust gates.
   Commit.
10. Close issues #451 and #452 with a comment linking this plan and explaining
    the `--concise` supersession.

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
