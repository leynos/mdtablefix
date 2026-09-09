# Add `--check` and `--diff` reporting modes to the `mdtablefix` CLI

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Today `mdtablefix` can only print reformatted Markdown to standard output or
rewrite files in place. There is no way to ask "would this command change
anything?" without either discarding the answer or mutating the working tree.
That makes `mdtablefix` unusable as a continuous-integration formatting gate.

After this change a user gains two read-only reporting modes.

`mdtablefix --check FILE...` reads every supplied file, computes what the
formatter would write, and reports each file that would change together with
the number of lines that would be inserted and deleted. It writes nothing to
disk. It exits with status `1` when at least one file would change, and `0`
when none would.

`mdtablefix --diff FILE...` does the same analysis but prints a standard
unified diff for each file that would change. It also writes nothing to disk,
and exits `0` whether or not anything would change, matching the exit-status
behaviour of a bare invocation and of `--in-place`.

Observable success looks like this. Given a file `broken.md` whose table
columns are ragged and a file `clean.md` that is already formatted:

```console
$ mdtablefix --check broken.md clean.md
broken.md +3 -3
1 file would be reformatted, 1 file left unchanged.
$ echo $?
1
$ mdtablefix --check clean.md
1 file left unchanged.
$ echo $?
0
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
0
$ cmp --silent broken.md broken.md.backup && echo "unmodified"
unmodified
```

Delivering this honestly requires one further change. The formatter currently
normalizes every input to LF line endings, so on a CRLF file `--check` would
report that every single line changes even when no Markdown content changes at
all. That would make the gate useless in Windows-oriented repositories. This
plan therefore also implements line-ending preservation, closing the second
half of the observable behaviour: a CRLF file that needs no Markdown changes
reports clean.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- `--check` and `--diff` must never write to, create, truncate, or change the
  modification time of any input file. This is the defining property of both
  modes.
- `--check` must report drift for a file if and only if `--in-place` would
  alter that file's bytes. The two code paths must derive their answer from a
  single computed value so this cannot drift apart. See the `Assessment` type
  in `Interfaces and dependencies`.
- `--check`, `--diff`, and `--in-place` are mutually exclusive. Supplying more
  than one must be rejected by argument parsing before any file is read.
- Existing behaviour of a bare invocation, of `--in-place`, and of standard
  input mode must be preserved apart from the two deliberate changes recorded
  in `Decision log`: the operational-error exit status moves from `1` to `2`,
  and output line endings now follow the input rather than always being LF.
- Standard input mode currently prints a single newline for empty input.
  `tests/parallel.rs` asserts this (`.stdout("\n")`). Preserve it exactly; do
  not "fix" it as part of this work.
- All existing formatting transforms must remain untouched. This plan adds
  reporting and serialization behaviour only. No change to `src/process.rs`
  transform logic, `src/table.rs`, `src/wrap/`, `src/footnotes/`, or any other
  transform module is authorized.
- Every body transform must continue to route through
  `mdtablefix::process::process_with_frontmatter`, the canonical frontmatter
  split and rejoin boundary documented in `docs/developers-guide.md`. The
  reports must be derived from that same formatted output, never from a
  re-derivation of formatting logic.
- No Rust source file may exceed 400 lines, per `AGENTS.md`. `tests/cli.rs` is
  already at exactly 400 lines and `docs/execplans/cli-matrix-testing.md`
  explicitly forbids growing it. New tests go in new files.
- `src/process.rs` is 375 lines and has only 25 lines of headroom. Do not add
  check or diff logic to it.
- Report output goes to standard output; diagnostics go to standard error.
  This follows the constraint already established in
  `docs/execplans/cli-matrix-testing.md`.
- Multi-file report order must match the order files were supplied on the
  command line, deterministically, despite parallel processing via `rayon`.
- Diff output must be deterministic. No timestamps, no locale-dependent or
  environment-dependent content, because the output is snapshot-tested.
- Clippy warnings must be denied. Lints must not be silenced except as a last
  resort, and any `#[expect(...)]` must carry a `reason`.
- Every new module begins with a `//!` module-level comment. Every public item
  carries a `///` Rustdoc comment. Function attributes go after doc comments.
- All prose uses en-GB-oxendict spelling per `docs/documentation-style-guide.md`.

## Tolerances (exception triggers)

- Scope: if the implementation needs to touch more than 30 files or roughly
  1600 net lines across source, tests, and documentation, stop and escalate.
- Dependencies: one new runtime dependency (`similar`) and four new
  development dependencies (`rstest-bdd`, `rstest-bdd-macros`, `googletest`,
  `pretty_assertions`) are proposed and must be approved at the approval gate.
  If any further dependency proves necessary, stop and escalate.
- Interface: if any existing public library function signature must change
  incompatibly, stop and escalate. Adding new public modules is expected and
  does not trigger this.
- Behaviour: if line-ending preservation turns out to change the output of any
  existing test fixture in a way not explained by CRLF or trailing-newline
  handling, stop and escalate; that indicates a transform regression.
- Verus: if the optional proof milestone `EP-M6` consumes more than four hours
  of toolchain integration effort without a verified lemma, stop, record the
  residual gap, and fall back to the property test alone.
- Iterations: if a gate still fails after three focused fix cycles, stop and
  record the failing command, the `/tmp` log path, and the likely cause in
  `Decision log`.
- Snapshot churn: if a single change rewrites more than 30 `insta` snapshots,
  stop and reconsider whether the change is as narrow as intended.
- Ambiguity: if any requirement here admits two readings that would produce
  materially different user-visible behaviour, stop and present the options.

## Risks

- Risk: line-ending preservation is a cross-cutting change to the read and
  write path used by every mode, so a mistake regresses all existing output.
  Severity: high. Likelihood: medium. Mitigation: land it as its own milestone
  (`EP-M1`) with no CLI surface change, so the entire existing test suite acts
  as the regression oracle before any new feature is built on top.

- Risk: `--check` and `--diff` could diverge from `--in-place`, reporting drift
  that in-place would not produce or missing drift that it would. Severity:
  high. Likelihood: medium. Mitigation: make all three modes consume one
  `Assessment` value produced by one function, and prove the equivalence with a
  property test rather than relying on code review.

- Risk: promoting `similar` from a transitive development dependency to a
  direct runtime dependency could duplicate the crate in the build graph if the
  version does not unify with the one `insta` resolves. Severity: low.
  Likelihood: medium. Mitigation: request `similar = "2.7"`, which unifies with
  the already-resolved `similar 2.7.0` in `Cargo.lock`; verify with
  `cargo tree --duplicates` after adding it.

- Risk: `rstest-bdd` has never been used in this repository, so first adoption
  carries unknown integration cost (step registration linkage, compile-time
  validation features, feature-file discovery). Severity: medium. Likelihood:
  medium. Mitigation: `EP-M0` includes a throwaway canary scenario proving the
  wiring works before any real behavioural test depends on it.

- Risk: the `similar` unified-diff output shape may not match expectations
  (context radius, missing-newline hint, header format), causing snapshot
  churn after the fact. Severity: low. Likelihood: medium. Mitigation: `EP-M0`
  spikes the output against a real fixture and pins the configuration before
  snapshots are written.

- Risk: changing the operational-error exit status from `1` to `2` is a
  user-visible behaviour change for existing scripted callers. Severity:
  medium. Likelihood: low. Mitigation: record it in an ADR and `CHANGELOG.md`,
  and add explicit tests asserting each of the three statuses.

- Risk: a file with no trailing newline will always report drift, because the
  formatter appends one. Severity: low. Likelihood: high. Mitigation: this is
  correct and intended (in-place genuinely would add the newline); document it
  in `docs/users-guide.md` so it does not surprise users.

- Risk: `similar`'s Apache-2.0 licence differs from this repository's ISC
  licence. Severity: low. Likelihood: low. Mitigation: Apache-2.0 is permissive
  and compatible with distributing an ISC-licensed binary; record the
  assessment in the ADR.

## Progress

- [ ] EP-M0 Prototyping spike: `similar` output shape and `rstest-bdd` wiring.
- [ ] EP-M1 Line-ending preservation and the shared document serialization
      boundary (`src/document.rs`), closing issue #451.
- [ ] EP-M2 Pure reporting domain (`src/check/`): `LineDelta` and rendering.
- [ ] EP-M3 Application service, ports, `--check` flag, and the three-valued
      exit-status contract.
- [ ] EP-M4 `--diff` flag.
- [ ] EP-M5 CLI matrix integration and snapshot coverage.
- [ ] EP-M6 (optional, go/no-go) Verus proof of the change-counting lemma.
- [ ] EP-M7 Documentation, ADRs, changelog, and issue closure.

## Surprises & discoveries

- Observation: `docs/users-guide.md` contains no command-line flag reference at
  all. The canonical flag list lives in `README.md` lines 52 to 106, which
  contradicts `docs/documentation-style-guide.md`, whose rule places
  command-line usage reference material in the user's guide.
  Evidence: `README.md:52-106`; `docs/users-guide.md` has no match for
  "in-place"; `docs/documentation-style-guide.md:103-129`.
  Impact: this plan adds a proper command-line interface section to
  `docs/users-guide.md` covering all flags, and reduces `README.md` to a
  synopsis that links to it. Resolving the inconsistency is in scope because
  the task explicitly requires `docs/users-guide.md` to be updated.

- Observation: `docs/state-machine-abstractions-roadmap.md` exists on disk but
  is absent from `docs/contents.md`, contrary to the style guide's rule that
  the contents file is updated whenever a document is added.
  Evidence: file present via glob; no matching entry in `docs/contents.md`.
  Impact: a one-line index fix is included in `EP-M7` so this plan does not
  repeat the omission for its own new documents.

- Observation: `src/io.rs` (`rewrite`, `rewrite_no_wrap`) duplicates the read,
  format, and write pattern that `src/main.rs` implements independently over
  `cap_std`, including the trailing-newline rule, but is never called by the
  binary. It remains public API documented in `README.md`.
  Evidence: `src/io.rs:16-41` versus `src/main.rs:122-142`; repository-wide
  grep finds no call from `src/main.rs`.
  Impact: `EP-M1` removes the duplication by routing both through
  `mdtablefix::document`, so line-ending preservation reaches library consumers
  as well as the binary. `src/io.rs`'s public signatures are unchanged.

- Observation: `similar::DiffableStr::tokenize_lines` retains line terminators,
  including `\r\n`, in each token.
  Evidence: `similar-2.7.0/src/text/abstraction.rs:101-115` builds slices with
  inclusive ranges covering the terminator.
  Impact: line tokenization is a partition of the input string, so two texts
  have equal token sequences exactly when they are byte-equal. This makes the
  `LineDelta` agreement property in `Verification plan` meaningful rather than
  vacuous, and means a trailing-newline-only or CRLF-only difference is
  correctly counted rather than silently ignored.

- Observation: `similar 2.7.0` is already present in `Cargo.lock` as a
  transitive development dependency of `insta 1.47.2`, and is licensed
  Apache-2.0.
  Evidence: `Cargo.lock:449-458` and `Cargo.lock:1108-1111`;
  `similar-2.7.0/Cargo.toml:38`.
  Impact: requesting `similar = "2.7"` as a direct runtime dependency adds no
  new crate to the build graph, only a new edge.

## Decision log

- Decision: implement `--check` as a concise report (`path +nnn -nnn`) and
  `--diff` as a separate unified-diff mode, and do not implement the
  `--concise` flag described in GitHub issue #452.
  Rationale: issue #452 proposed `--check` emitting diffs with `--concise`
  reducing it to filenames. The requested design inverts this: `--check` is
  already the concise form, so a third flag would be redundant, and `--diff`
  needs exit status `0` whereas #452's `--check` needs non-zero. Two flags with
  one behaviour each is simpler than one flag with a modifier. The user
  confirmed this supersession.
  Date/Author: 2026-09-09, planning agent, confirmed by `@leynos`.

- Decision: adopt a three-valued exit-status contract: `0` clean, `1` drift
  detected under `--check`, `2` operational error. `fn main` returns
  `std::process::ExitCode` instead of `anyhow::Result<()>`.
  Rationale: a formatting gate must distinguish "the code needs formatting"
  from "the tool could not run". Today both exit `1`, so a mistyped path in CI
  is indistinguishable from genuine drift. `1` for drift and `2` for error
  matches `ruff format --check` and aligns with clap's existing use of `2` for
  usage errors. The cost is a user-visible change to the error status in every
  mode, recorded in an ADR and the changelog.
  Date/Author: 2026-09-09, planning agent, confirmed by `@leynos`.

- Decision: `--check` reports drift if and only if the formatted bytes differ
  from the file's current bytes, rather than comparing normalized line
  sequences.
  Rationale: the whole value of `--check` is that it predicts `--in-place`. Any
  comparison that ignores differences `--in-place` would actually write breaks
  that prediction and produces a gate that passes while the tree is still
  dirty. The consequence is that line-ending and trailing-newline differences
  count as drift, which is why issue #451 is brought into scope rather than
  deferred.
  Date/Author: 2026-09-09, planning agent, confirmed by `@leynos`.

- Decision: implement line-ending preservation (issue #451) within this plan
  as milestone `EP-M1`, before any reporting feature is built.
  Rationale: with byte-exact comparison and unconditional LF normalization,
  `--check` would report whole-file drift for every CRLF file, making the
  feature actively misleading on Windows-oriented repositories. Landing it
  first means the existing test suite validates the serialization change before
  new behaviour depends on it.
  Date/Author: 2026-09-09, planning agent, confirmed by `@leynos`.

- Decision: the `--check` report line is `<path> +<insertions> -<deletions>`,
  with counts as plain decimal integers, not zero-padded.
  Rationale: path-first sorts and greps cleanly and is unambiguous to parse in
  continuous integration. It stays close to `git diff --numstat`. The `nnn`
  notation in the requirement denotes a decimal count, not a fixed width.
  Date/Author: 2026-09-09, planning agent, confirmed by `@leynos`.

- Decision: count a changed line as one insertion plus one deletion, matching
  `git diff --numstat` semantics.
  Rationale: this is the universally understood meaning of `+n -n` in
  formatting and version-control tooling. Any other convention would surprise
  users. Recorded because it is not self-evident from the requirement text.
  Date/Author: 2026-09-09, planning agent.

- Decision: use the `similar` crate at version `2.7` for both change counting
  and unified-diff rendering, rather than hand-rolling a diff algorithm.
  Rationale: producing a correct minimal line diff and a specification-conformant
  unified-diff rendering is a solved problem with real subtleties (hunk
  coalescing, context radius, the missing-newline marker). `similar` already
  exists in `Cargo.lock` at exactly this version via `insta`, is Apache-2.0
  licensed, has no mandatory dependencies, and exposes both the change iterator
  and the unified-diff formatter this plan needs. Requesting `"2.7"` rather
  than the newer `"3"` keeps a single copy in the build graph.
  Date/Author: 2026-09-09, planning agent. Requires approval at the gate.

- Decision: `--check` and `--diff` require file arguments, mirroring
  `--in-place`'s existing `requires = "files"`.
  Rationale: both modes report per-file results with per-file paths, which has
  no meaning for standard input. Consistency with `--in-place` gives users one
  rule to remember. Supporting a `-` pseudo-path was considered and rejected as
  scope not requested.
  Date/Author: 2026-09-09, planning agent.

- Decision: when no line-ending style holds a strict majority, or the file
  contains no line endings at all, emit LF.
  Rationale: issue #451 requires deterministic documented behaviour in the
  no-majority case. LF preserves today's behaviour and is the ecosystem
  default, so the tie-break is the least surprising choice.
  Date/Author: 2026-09-09, planning agent.

- Decision: unified-diff headers use the path as supplied on the command line
  for both the `---` and `+++` sides, with no timestamps.
  Rationale: `similar` accepts header strings verbatim and the unified-diff
  format permits a tab-separated timestamp. Including one would make output
  non-deterministic and unsnapshot-able, and would defeat `--diff | patch`
  round-tripping in continuous integration. Black includes timestamps; this
  plan deliberately does not.
  Date/Author: 2026-09-09, planning agent.

- Decision: place the application service (mode dispatch, the document-store
  port, exit-status aggregation) in the library at `src/app.rs`, keeping
  `src/main.rs` as a thin adapter.
  Rationale: `src/main.rs` has only 98 lines of headroom under the 400-line cap
  and already holds argument parsing plus filesystem access. Putting the
  service in the library lets integration tests and unit tests exercise mode
  dispatch through an in-memory store, which is the only practical way to prove
  "check and diff never write" structurally rather than by observation. The
  alternative considered was a binary-private `mod driver;` resolving to
  `src/driver.rs`, which avoids growing the public library surface but is a
  known footgun because the file sits beside library modules without being one.
  Date/Author: 2026-09-09, planning agent. Open to challenge at design review.

- Decision: an operational error dominates drift when aggregating the exit
  status. If one file fails to read and another would be reformatted,
  `--check` exits `2`, not `1`.
  Rationale: an incomplete analysis must not be reported as a clean or merely
  drifted result, because the failed file's true state is unknown. Every error
  is still printed to standard error so no failure is hidden.
  Date/Author: 2026-09-09, planning agent.

- Decision: verify the change-counting lemma primarily with `proptest`, and
  treat a Kani harness as inappropriate here.
  Rationale: the lemma's real risk lies in how repository code aggregates
  `similar`'s output. A bounded model check would have to run against a
  synthetic `ChangeTag` sequence rather than the real diff engine, so it would
  verify a re-implementation and be vacuous with respect to the actual failure
  mode. A property test exercising the genuine `similar` interface can fail
  when the implementation is wrong; the model check cannot. Verus is planned
  separately in `EP-M6` for the sequence-arithmetic lemma itself, where an
  inductive argument does add value over sampling.
  Date/Author: 2026-09-09, planning agent.

- Decision: copy `docs/rstest-bdd-users-guide.md` and
  `docs/reliable-testing-in-rust-via-dependency-injection.md` into this
  repository and register them in `docs/contents.md`.
  Rationale: the task signposts both, but neither exists here; they live in
  sibling repositories. This plan is the first to introduce `rstest-bdd` and
  port-based injection to `mdtablefix`, so contributors need the references
  locally. `docs/netsuke-design.md` and `docs/ortho-config-users-guide.md` are
  also absent but are not applicable: this repository uses plain `clap` rather
  than `ortho-config`, and no Netsuke-specific policy is adopted here.
  Date/Author: 2026-09-09, planning agent.

## Outcomes & retrospective

Not started. Complete this section at each milestone boundary and before
setting the plan to `COMPLETE`, reconciling every discovery against the
artefacts named in `Conformance basis`.

## Context and orientation

`mdtablefix` is a Rust command-line tool that repairs and reflows Markdown
tables, and optionally applies other transforms such as paragraph wrapping and
footnote conversion. The repository is a single crate plus a small
`test-macros` helper crate.

The reader needs to understand five existing pieces before starting.

First, the transform pipeline. `src/process.rs` exposes pure functions with the
shape `&[String] -> Vec<String>`; they take lines and return lines and perform
no input or output. The relevant ones are `process_stream_inner(lines, opts)`
at `src/process.rs:95` and `process_with_frontmatter(lines, body_fn)` at
`src/process.rs:275`. The latter is the canonical boundary that splits leading
YAML frontmatter off, applies the body function, and rejoins. `Options` at
`src/process.rs:42` is a `Copy` struct of six booleans selecting transforms.

Second, the binary. `src/main.rs` (302 lines) defines a `clap` `Cli` struct
with an `in_place` flag, a flattened `FormatOpts` struct of transform flags,
and a `Vec<PathBuf>` of files. `fn process_lines` at `src/main.rs:84` composes
`process_with_frontmatter`, `process_stream_inner`, `renumber_lists`, and
`format_breaks`. `fn open_file_parent` at `src/main.rs:100` is described in its
own doc comment as "the only ambient filesystem boundary for CLI file
processing": it opens the file's parent directory as a `cap_std::fs_utf8::Dir`
capability and returns that plus the bare file name, so all subsequent input
and output is confined to that directory. `fn format_to_string` at
`src/main.rs:122` reads through the capability and returns formatted text;
`fn rewrite_in_place` at `src/main.rs:136` writes that text back. Files are
processed in parallel with `rayon`'s `par_iter`, and `fn report_results` at
`src/main.rs:144` prints every error to standard error but propagates only the
first, so `main` exits `1` on any failure.

A "capability" here means a handle that grants access to one directory and
nothing outside it, as opposed to "ambient" access where any path in the
filesystem can be opened. This matters because the new modes must read files
without ever gaining the ability to write them.

Third, serialization. Both `src/main.rs:129-133` and `src/io.rs:21-25`
independently implement the same rule: an empty result produces an empty file,
and any non-empty result is joined with `"\n"` and given exactly one trailing
newline. Both read with `str::lines()`, which strips `\n` and a preceding `\r`.
The combination means CRLF input is silently converted to LF output, and a file
without a trailing newline gains one. `src/io.rs`'s `rewrite` and
`rewrite_no_wrap` are public library API documented in `README.md` but are not
called by the binary, which uses the `cap_std` path instead.

Fourth, testing. Each `.rs` file directly under `tests/` compiles to its own
test binary, so shared helpers are re-declared per binary with `#[path = ...]
mod ...;` rather than through any central registration. `tests/support/`
provides `run_cli_with_args` and `run_cli_with_stdin`, both returning an
`assert_cmd::assert::Assert`. `tests/cli_matrix.rs` and
`tests/cli_matrix/support.rs` implement a pairwise option matrix that expands
curated base rows into wrap and no-wrap variants and then into standard-output
and `--in-place` runs, snapshotting each with `insta`; its
`RunResult::envelope` at `tests/cli_matrix/support.rs:218` builds a labelled
block containing the case identifier, mode, arguments, exit status, standard
output, standard error, and resulting file content. Snapshots live flat under
`tests/snapshots/`. `tests/cli.rs` is at exactly the 400-line cap and must not
grow.

Fifth, the tracked requirement. GitHub issue #452, "Add check-only formatting
mode with diff and concise output", is the upstream request. GitHub issue #451,
"Preserve the majority input line-ending style in formatter output", is its
prerequisite; its own rationale states that line-ending normalization
"prevents check-only formatting gates from comparing formatter output directly
with valid CRLF source files". There is no `docs/roadmap.md` in this
repository; these two issues are the roadmap entries this plan discharges.

Terms used throughout this plan:

- Drift: a file whose formatted bytes differ from its current bytes.
- Assessment: the pair of a file's current text and its formatted text, from
  which every mode derives its behaviour.
- Line delta: the count of inserted and deleted lines between two texts, using
  `git diff --numstat` semantics where a changed line is one of each.
- Port: an interface the application defines and depends upon. Adapter: an
  implementation of a port that talks to a real external system.

## Conformance basis

There is no Terms of Reference document and no technical design document in
this repository, and none should be invented. The upstream artefacts are:

- `AGENTS.md` (repository root, at commit `c792270`): code style, file-size
  cap, testing obligations, documentation-maintenance duties.
- `docs/documentation-style-guide.md`: prose, Markdown, ADR, and roadmap
  conventions.
- `docs/architecture.md`: current component narrative and Mermaid diagrams.
- `docs/developers-guide.md`: internal API reference and testing conventions.
- `docs/adrs/0004-state-machine-abstractions.md`: the ADR whose header format
  this plan's new ADRs follow, and whose guidance governs whether new stateful
  logic should be explicit or delegated to a crate.
- `docs/execplans/cli-matrix-testing.md`: constraints inherited for CLI test
  placement, fixture format, and snapshot discipline.
- GitHub issue #452: the check-mode requirement, partially superseded, see
  `Decision log`.
- GitHub issue #451: the line-ending requirement, discharged in full.

Two new ADRs are created by this plan and become part of the basis once
accepted: `docs/adrs/0006-check-and-diff-reporting.md` and
`docs/adrs/0007-line-ending-preservation.md`.

Trace links:

```plaintext
ISSUE-451 -> ADR-0007 -> EP-M1 -> tests::document::crlf_round_trips
ISSUE-452-check -> ADR-0006 -> EP-M3 -> tests::cli_check::reports_drift_and_exits_one
ISSUE-452-diff -> ADR-0006 -> EP-M4 -> tests::cli_diff::emits_unified_diff_and_exits_zero
ISSUE-452-no-write -> ADR-0006 -> EP-M3 -> tests::app::check_never_writes
ISSUE-452-exit -> ADR-0006 -> EP-M3 -> tests::cli_check::exit_status_contract
ISSUE-452-multifile -> ADR-0006 -> EP-M3 -> tests::cli_check::reports_every_file_in_order
```

Issue #452's `--concise` acceptance criteria are deliberately not traced; see
`Decision log` for the supersession and its approval.

## Verification plan

This change introduces genuine invariants, so this section is substantive
rather than a formality. Third-party internals are not verified: `similar`'s
diff algorithm, `clap`'s parsing, `rayon`'s scheduling, and `cap_std`'s
capability enforcement are treated as axioms. Repository-owned logic that
builds on those interfaces is verified against the real interface.

### Axioms

- AX-1: `similar::DiffableStr::tokenize_lines` partitions its input. The
  concatenation of the tokens equals the input, and terminators including
  `\r\n` are retained in the tokens. Evidence:
  `similar-2.7.0/src/text/abstraction.rs:101-115`. Consequence: two texts have
  equal token sequences exactly when they are byte-equal.
- AX-2: for `similar::TextDiff::iter_all_changes`, the subsequence of changes
  tagged `Equal` or `Delete` reproduces the old token sequence in order, and
  the subsequence tagged `Equal` or `Insert` reproduces the new one. This is
  the documented contract of a diff.
- AX-3: `cap_std::fs_utf8::Dir` confines all operations to the opened
  directory, and a handle that is never asked to write does not write.
- AX-4: `rayon`'s `par_iter().map(..).collect::<Vec<_>>()` yields results in
  the order of the source iterator regardless of completion order.
- AX-5: `str::lines()` splits on `\n` and strips one preceding `\r`, and does
  not treat a lone `\r` as a separator.

### Obligations

- Obligation INV-PREDICTS: for every input file and every combination of
  transform options, `--check` reports drift if and only if `--in-place` would
  write bytes different from the file's current bytes.
  Method: property test, plus a structural argument.
  Rationale: this is the load-bearing user-facing guarantee. The structural
  argument is that both modes read one `Assessment` and `--in-place` writes
  exactly `assessment.formatted` while `--check` compares exactly
  `assessment.original` against `assessment.formatted`; the property test
  checks that the structure was not subverted.
  Domain: generated Markdown documents mixing tables, prose, lists, fenced
  code, and frontmatter; both LF and CRLF; with and without a trailing
  newline; across the `Options` powerset sampled uniformly.
  Artefact: `tests/check_properties.rs`.
  Evidence: `cargo test --test check_properties`. Before `EP-M3` the test does
  not compile because `--check` does not exist; after, it passes.
  Non-vacuity: the generator must produce both drifting and clean documents.
  Classify cases with `proptest::prop_assert!` plus explicit counters and
  assert that at least one of each class was seen across the run. Negative
  control: temporarily change the check path to compare trimmed strings; the
  property must fail on a trailing-newline case.

- Obligation INV-NOWRITE: `--check` and `--diff` never write, create, or
  truncate any file, and never change a file's modification time.
  Method: parameterized unit test with a fault-injecting in-memory adapter,
  plus an end-to-end filesystem check.
  Rationale: an observational end-to-end test alone could pass by luck on the
  fixtures chosen. Injecting a store whose `write` panics turns any write into
  a hard failure for every case the unit tests cover, while the end-to-end test
  confirms the real adapter behaves the same.
  Domain: clean files, drifting files, empty files, unreadable files, and
  multi-file batches mixing all of these.
  Artefact: `src/app.rs` unit tests plus `tests/cli_check.rs` and
  `tests/cli_diff.rs`.
  Evidence: `cargo test --lib app` and `cargo test --test cli_check`.
  Non-vacuity: the same fault-injecting store must be used in an `--in-place`
  case where the panic does fire, proving the injection is wired up and the
  guard is not silently inert.

- Obligation LEM-COUNT: for any two texts, if `insertions` and `deletions` are
  the counts of `ChangeTag::Insert` and `ChangeTag::Delete` in the change
  sequence, then the formatted text's line count equals the original's line
  count plus `insertions` minus `deletions`.
  Method: property test against the real `similar` interface, with an optional
  Verus proof of the underlying sequence arithmetic in `EP-M6`.
  Rationale: this is the correctness condition for the `+nnn -nnn` report. It
  follows from AX-2 by induction over the change sequence, so it is a genuine
  lemma rather than a restatement. The property test can fail if the
  repository's counting mis-attributes a tag; a Kani harness over a synthetic
  tag sequence could not, because it would not exercise the real diff, which is
  why one is not planned.
  Domain: as INV-PREDICTS, plus adversarial pairs: empty against non-empty,
  pure insertion, pure deletion, pure replacement, and identical texts.
  Artefact: `tests/check_properties.rs`.
  Evidence: `cargo test --test check_properties`.
  Non-vacuity: assert coverage of all five adversarial classes. Negative
  control: a mutant that counts `Equal` as `Insert` must be rejected.

- Obligation INV-AGREE: the computed line delta is zero in both components if
  and only if the two texts are byte-equal.
  Method: property test.
  Rationale: `is_changed` is computed by direct byte comparison and the delta
  is computed by `similar`. They are two independent routes to the same fact,
  so cross-checking them detects a mistake in either. By AX-1 the biconditional
  must hold; if it fails, either the axiom is wrong for some input or the code
  is.
  Domain: as LEM-COUNT, with explicit CRLF-only and trailing-newline-only
  differences, which are precisely the cases a naive implementation gets wrong.
  Artefact: `tests/check_properties.rs`.
  Evidence: `cargo test --test check_properties`.
  Non-vacuity: assert that the generator produces at least one pair differing
  only in line-ending style and at least one differing only in the presence of
  a trailing newline. Negative control: compute the delta from
  `str::lines()`-split text instead of `similar`'s tokens; the CRLF-only case
  must then fail.

- Obligation INV-ENDINGS: the serialized output uses CRLF when CRLF is a strict
  majority of the input's line endings, and LF otherwise; and re-serializing an
  already-serialized document is a fixed point.
  Method: parameterized tests for the partition plus a property test for the
  fixed point.
  Rationale: the majority rule has three boundary classes (CRLF majority, LF
  majority, exact tie) that parameterized cases enumerate exhaustively, while
  idempotence is a statement over all inputs and needs generation.
  Domain: pure LF, pure CRLF, mixed with each majority, exact tie, no line
  endings at all, empty input, lone `\r`.
  Artefact: `src/document.rs` unit tests plus `tests/document_properties.rs`.
  Evidence: `cargo test --lib document` and
  `cargo test --test document_properties`.
  Non-vacuity: the exact-tie case must be present and must assert LF, since
  that is the arbitrary choice most likely to be implemented inconsistently.
  Negative control: change the comparison from `>` to `>=`; the tie case must
  fail.

- Obligation INV-ORDER: multi-file report output lists files in the order they
  were supplied on the command line, regardless of parallel completion order.
  Method: end-to-end behavioural test.
  Rationale: AX-4 makes this true by construction, but a future refactor to
  `for_each` or an unordered collection would break it silently, and the failure
  would be intermittent and hard to diagnose.
  Domain: batches of eight files alternating clean and drifting, with the
  drifting ones deliberately ordered so that alphabetical, size, and
  completion-time orderings all differ from argument order.
  Artefact: `tests/cli_check.rs`.
  Evidence: `cargo test --test cli_check reports_every_file_in_order`.
  Non-vacuity: the fixtures must have differing sizes so a completion-ordered
  implementation would very likely produce a different order; assert on the
  exact ordered sequence, not on set membership.

- Obligation INV-EXIT: the process exit status is `0` when no file drifts and
  no error occurs, `1` when at least one file drifts under `--check` and no
  error occurs, and `2` whenever any operational error occurs, in any mode.
  Method: parameterized unit test over the aggregation function plus
  end-to-end assertions.
  Rationale: the aggregation is a small total function over a finite lattice,
  so its cases can be enumerated exhaustively; the end-to-end tests confirm the
  wiring from that function to the process status.
  Domain: the full cross product of {no files, all clean, some drift, all
  drift} with {no error, some error} for each of the four modes.
  Artefact: `src/app.rs` unit tests plus `tests/cli_check.rs` and
  `tests/cli_diff.rs`.
  Evidence: `cargo test --lib app::exit` and `cargo test --test cli_check`.
  Non-vacuity: the "some drift and some error" case must assert `2`, not `1`,
  which is the precedence decision most likely to be got wrong. Negative
  control: swap the precedence; that case must fail.

### Residual gaps

If `EP-M6` is abandoned under its tolerance, LEM-COUNT rests on sampling rather
than proof. Record that explicitly in `Outcomes & retrospective` rather than
leaving it implied. A lone `\r` used as a line separator, as on pre-OS X Mac,
is out of scope in all obligations; `str::lines()` does not treat it as a
separator (AX-5) and neither will this code. Document that limitation in
`docs/users-guide.md`.

## Interfaces and dependencies

### Dependencies to add

In `Cargo.toml`:

```toml
[dependencies]
similar = "2.7"

[dev-dependencies]
googletest = "0.14"
pretty_assertions = "1"
rstest-bdd = "0.5.0"
rstest-bdd-macros = { version = "0.5.0", features = ["strict-compile-time-validation"] }
```

`similar = "2.7"` unifies with the `similar 2.7.0` already resolved through
`insta`. Verify with `cargo tree --duplicates` that no second copy appears.
`rstest-bdd-macros`'s `strict-compile-time-validation` feature turns a missing
step definition into a compile error rather than a runtime skip, which is the
setting used in sibling repositories that have adopted the crate. Both
`rstest-bdd` crates require Rust 1.85 or newer; this repository pins 1.89.

### `src/document.rs` (new, library)

The shared serialization boundary. Replaces the duplicated join-and-newline
logic in `src/main.rs` and `src/io.rs`.

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
    /// Returns the literal characters this style writes between lines.
    pub fn as_str(self) -> &'static str;

    /// Selects the style holding a strict majority of `content`'s line
    /// endings, defaulting to [`LineEnding::Lf`] on a tie or when `content`
    /// contains no line endings.
    pub fn detect(content: &str) -> Self;
}

/// A document split into lines together with the line-ending style to restore
/// when it is written back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    lines: Vec<String>,
    line_ending: LineEnding,
}

impl SourceDocument {
    /// Splits `content` into lines and records its majority line-ending style.
    pub fn parse(content: &str) -> Self;

    /// Borrows the document's lines, with line endings removed.
    pub fn lines(&self) -> &[String];

    /// Returns the line-ending style detected when parsing.
    pub fn line_ending(&self) -> LineEnding;
}

/// Serializes `lines` using `line_ending`.
///
/// An empty slice produces an empty string. A non-empty slice is joined with
/// `line_ending` and terminated with one further `line_ending`, matching the
/// trailing-newline convention every existing writer already applies.
pub fn render(lines: &[String], line_ending: LineEnding) -> String;
```

### `src/check.rs` and `src/check/` (new, library)

Pure reporting domain. No input or output, no paths opened, no process state.

```rust
// src/check.rs
pub use crate::check::delta::LineDelta;
pub use crate::check::render::{render_report_line, render_summary, render_unified_diff};

/// A file's current text paired with the text the formatter would write.
///
/// Every mode derives its behaviour from this one value, so `--check`,
/// `--diff`, and `--in-place` cannot disagree about whether a file changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    original: String,
    formatted: String,
}

impl Assessment {
    /// Pairs a document's current text with its formatted text.
    pub fn new(original: String, formatted: String) -> Self;

    /// The text currently on disk.
    pub fn original(&self) -> &str;

    /// The text the formatter would write.
    pub fn formatted(&self) -> &str;

    /// Whether writing the formatted text would change the document's bytes.
    ///
    /// This is a direct byte comparison and is the authoritative answer;
    /// [`Assessment::delta`] is reported alongside it but never consulted to
    /// decide whether a document changed.
    pub fn is_changed(&self) -> bool;

    /// Counts the lines that would be inserted and deleted.
    pub fn delta(&self) -> LineDelta;
}
```

```rust
// src/check/delta.rs

/// Counts of lines inserted and deleted between two texts.
///
/// A line that is modified counts as one insertion and one deletion, matching
/// `git diff --numstat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LineDelta {
    insertions: usize,
    deletions: usize,
}

impl LineDelta {
    /// Counts the line-level changes turning `original` into `formatted`.
    pub fn between(original: &str, formatted: &str) -> Self;

    /// The number of lines that would be added.
    pub fn insertions(self) -> usize;

    /// The number of lines that would be removed.
    pub fn deletions(self) -> usize;

    /// Whether either count is non-zero.
    pub fn is_empty(self) -> bool;
}
```

```rust
// src/check/render.rs

/// Renders one `--check` report line, for example `docs/a.md +12 -8`.
pub fn render_report_line(path: &camino::Utf8Path, delta: LineDelta) -> String;

/// Renders the `--check` trailing summary, for example
/// `2 files would be reformatted, 1 file left unchanged.`
pub fn render_summary(changed: usize, unchanged: usize) -> String;

/// Renders a unified diff with `path` on both header lines and no timestamps.
///
/// Uses a context radius of three lines and retains the
/// `\ No newline at end of file` marker.
pub fn render_unified_diff(
    path: &camino::Utf8Path,
    original: &str,
    formatted: &str,
) -> String;
```

### `src/app.rs` (new, library)

The application service: the driven port, mode dispatch, and exit-status
policy. It depends on the port, never on `cap_std`, `clap`, or `std::io`
directly.

```rust
/// Reads and writes document text on behalf of the application.
///
/// This is the single driven port through which every mode touches storage.
/// `--check` and `--diff` never call [`DocumentStore::write`], which is what
/// makes their read-only guarantee structural rather than incidental.
pub trait DocumentStore {
    /// Reads the document at `path` as UTF-8 text.
    ///
    /// # Errors
    /// Returns an error if the document cannot be read or is not valid UTF-8.
    fn read(&self, path: &camino::Utf8Path) -> anyhow::Result<String>;

    /// Replaces the document at `path` with `contents`.
    ///
    /// # Errors
    /// Returns an error if the document cannot be written.
    fn write(&self, path: &camino::Utf8Path, contents: &str) -> anyhow::Result<()>;
}

/// What the caller asked the tool to do with each document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Print formatted text to standard output.
    Print,
    /// Rewrite each document in place.
    InPlace,
    /// Report which documents would change, and by how many lines.
    Check,
    /// Print a unified diff for each document that would change.
    Diff,
}

/// The process outcome for one document or for the whole run.
///
/// Ordered by severity: [`Outcome::Error`] dominates [`Outcome::Drift`], which
/// dominates [`Outcome::Clean`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    /// Nothing would change and nothing failed.
    Clean,
    /// At least one document would be reformatted.
    Drift,
    /// At least one document could not be processed.
    Error,
}

impl Outcome {
    /// Maps the outcome onto the documented process exit status.
    ///
    /// `Clean` is `0`, `Drift` is `1`, and `Error` is `2`.
    pub fn exit_code(self) -> u8;
}

/// Reads `path` through `store` and pairs its text with the formatted result.
///
/// # Errors
/// Returns an error if the document cannot be read.
pub fn assess(
    store: &impl DocumentStore,
    path: &camino::Utf8Path,
    format: impl Fn(&SourceDocument) -> String,
) -> anyhow::Result<Assessment>;

/// Applies `mode` to one assessed document, writing any report to `out`.
///
/// # Errors
/// Returns an error if the document must be written and the write fails.
pub fn apply(
    store: &impl DocumentStore,
    path: &camino::Utf8Path,
    assessment: &Assessment,
    mode: Mode,
    out: &mut impl std::io::Write,
) -> anyhow::Result<Outcome>;

/// Reduces per-document outcomes to the run's outcome, most severe wins.
pub fn aggregate(outcomes: impl IntoIterator<Item = Outcome>) -> Outcome;
```

### `src/main.rs` (modified, binary)

`main.rs` becomes the adapter layer only: `clap` inbound, `cap_std` and
standard output outbound.

```rust
/// Reads and writes documents through a `cap_std` directory capability.
struct CapabilityStore {
    directory: cap_std::fs_utf8::Dir,
}

impl mdtablefix::app::DocumentStore for CapabilityStore { /* ... */ }
```

The `Cli` struct gains a mutually exclusive argument group:

```rust
#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
#[command(group(
    clap::ArgGroup::new("mode").multiple(false).requires("files")
))]
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

`fn main` changes signature from `anyhow::Result<()>` to
`std::process::ExitCode`, printing errors to standard error itself and
returning `ExitCode::from(outcome.exit_code())`.

### `src/io.rs` (modified, library)

`rewrite` and `rewrite_no_wrap` keep their exact public signatures. Only
`rewrite_with`'s body changes, to route through `document::parse` and
`document::render` so library consumers also gain line-ending preservation.

## Plan of work

### Stage A: understand and propose

No code changes. This document is Stage A. It ends at the approval gate.

### Stage B: red tests and feature specifications

Write the failing tests and feature files before any production code, per
milestone. Each milestone's red stage must be observed and recorded before its
green stage begins.

### Stage C: implementation and verification together

Implement each milestone's production code and its verification artefacts in
the same milestone, never deferring the property tests to the end.

### Stage D: refactor, documentation, and wider validation

Documentation, ADRs, changelog, and the full gate suite.

## Milestones and plateaus

### EP-M0: prototyping spike

Identifier and outcome: a throwaway branch-local spike that answers two
questions and is then deleted. First, what exactly does
`similar::TextDiff::from_lines(a, b).unified_diff().context_radius(3)
.header(path, path).to_string()` produce for a small ragged-table fixture, and
does the missing-newline marker appear where expected? Second, does a minimal
`rstest-bdd` scenario compile, link, and run in this crate with
`strict-compile-time-validation` enabled?

Requirements and gaps: de-risks the two unknowns identified in `Risks`.

Acceptance evidence: a transcript of the spike's diff output pasted into
`Artefacts and notes`, and a passing canary scenario. Both artefacts are then
either deleted or promoted; nothing half-finished is left behind.

Conformance check: no public interface, dependency beyond those already
approved, trust boundary, or persisted format is introduced.

Recovery: `git restore` the spike files; nothing else was touched.

Remaining gaps: everything.

Compatibility decision: none required.

### EP-M1: line-ending preservation

Identifier and outcome: `src/document.rs` exists, `src/main.rs` and
`src/io.rs` both use it, and the tool preserves each input's majority
line-ending style in every mode. Issue #451 is discharged. No new command-line
flag exists yet.

Requirements and gaps: `ISSUE-451`, `INV-ENDINGS`.

Acceptance evidence: `tests/document_properties.rs` and the `src/document.rs`
unit tests pass. A CRLF fixture processed with `--in-place` retains CRLF. The
entire pre-existing test suite passes unchanged, which is the point of
sequencing this first.

Conformance check: `src/io.rs`'s public signatures are unchanged; the new
`mdtablefix::document` module is a public addition; no persisted format
changes, though written bytes change for CRLF inputs, which is the requested
behaviour and is recorded in `CHANGELOG.md`.

Recovery: revert the milestone's commits; `src/main.rs` and `src/io.rs` return
to their duplicated join logic.

Remaining gaps: no reporting modes yet.

Compatibility decision: none required. `rewrite` and `rewrite_no_wrap` are
pre-existing public API and keep their signatures, so no shim is needed.

### EP-M2: pure reporting domain

Identifier and outcome: `src/check.rs`, `src/check/delta.rs`, and
`src/check/render.rs` exist and are fully unit- and property-tested. Nothing
in the binary calls them yet.

Requirements and gaps: `LEM-COUNT`, `INV-AGREE`.

Acceptance evidence: `cargo test --lib check` and
`cargo test --test check_properties` pass, including the adversarial classes
and negative controls named in `Verification plan`.

Conformance check: `similar` is now a direct runtime dependency;
`cargo tree --duplicates` shows one copy.

Recovery: the modules are additive and unreferenced by the binary, so they can
be deleted without touching anything else.

Remaining gaps: no command-line surface.

Compatibility decision: none required; these are new pre-existing-consumer-free
modules.

### EP-M3: application service, ports, and `--check`

Identifier and outcome: `src/app.rs` exists with the `DocumentStore` port,
`Mode`, `Outcome`, `assess`, `apply`, and `aggregate`. `src/main.rs`
implements `CapabilityStore`, returns `ExitCode`, and supports `--check`. All
four modes route through one `Assessment`.

Requirements and gaps: `ISSUE-452-check`, `ISSUE-452-no-write`,
`ISSUE-452-exit`, `ISSUE-452-multifile`, `INV-PREDICTS`, `INV-NOWRITE`,
`INV-ORDER`, `INV-EXIT`.

Acceptance evidence: `tests/cli_check.rs` and the `src/app.rs` unit tests pass;
`tests/features/check_mode.feature` scenarios pass; the exit-status contract is
asserted for all three values.

Conformance check: `main`'s return type changed, which is a user-visible
exit-status change requiring ADR 0006; `--check` is a new public command-line
interface, approved at the gate; no trust boundary changes, because
`CapabilityStore` still obtains access only through `open_file_parent`.

Recovery: revert the milestone; `EP-M2`'s modules become unreferenced again but
remain correct.

Remaining gaps: `--diff` and matrix coverage.

Compatibility decision: none required. The exit-status change is a deliberate
behaviour change to a pre-1.0 command-line tool with no compatibility
commitment, documented rather than shimmed.

### EP-M4: `--diff`

Identifier and outcome: `--diff` is implemented, exits `0` regardless of
drift, and emits a deterministic unified diff per changed file.

Requirements and gaps: `ISSUE-452-diff`.

Acceptance evidence: `tests/cli_diff.rs` passes, including an assertion that
`--diff` output applied with `patch` reproduces the formatted file, and
`tests/features/diff_mode.feature` scenarios pass.

Conformance check: `--diff` is a new public command-line interface, approved at
the gate; output is deterministic, so no snapshot instability is introduced.

Recovery: revert the milestone; `--check` remains fully functional.

Remaining gaps: matrix coverage and documentation.

Compatibility decision: none required.

### EP-M5: CLI matrix integration

Identifier and outcome: `--check` and `--diff` are execution modes in the
option matrix alongside standard output and `--in-place`, so every curated
transform combination is exercised under both new modes and snapshotted.

Requirements and gaps: regression protection for every combination of transform
flags with the new modes.

Acceptance evidence: `cargo test --test cli_matrix` passes with the harness's
own self-tests confirming that mode expansion now covers four modes and that
every logical case has a snapshot.

Conformance check: `docs/developers-guide.md`'s CLI matrix section is updated
in the same milestone so the documented harness matches the implemented one.

Recovery: revert the matrix expansion; the standalone `tests/cli_check.rs` and
`tests/cli_diff.rs` still cover the features.

Remaining gaps: documentation and issue closure.

Compatibility decision: none required; the matrix is a test-only surface.

### EP-M6: Verus proof of the counting lemma (optional, go/no-go)

Identifier and outcome: either a Verus proof of `LEM-COUNT`'s sequence
arithmetic exists and is checked in the gates, or the milestone is abandoned
under its tolerance and the residual gap is recorded.

Requirements and gaps: `LEM-COUNT`, strengthening it from sampled to proved.

Acceptance evidence: the Verus proof establishes, by induction over a sequence
of `Equal`, `Insert`, and `Delete` tags, that the count of `Equal` plus
`Insert` equals the count of `Equal` plus `Delete` plus insertions minus
deletions, with the antecedent shown inhabited by an explicit witness sequence.
The proof must not assume its conclusion nor introduce an axiom solely to
discharge it. Inspect the trusted assumptions before accepting it.

Go/no-go: proceed only if the Verus toolchain integrates without modifying
`rust-toolchain.toml` for the main build and without adding a required gate
that contributors cannot run. Abandon on the four-hour tolerance.

Conformance check: if kept, `docs/developers-guide.md` gains a section on how
to run the proof; if abandoned, `Outcomes & retrospective` records the gap.

Recovery: the proof is additive and isolated; delete it.

Remaining gaps: none introduced.

Compatibility decision: none required.

### EP-M7: documentation and closure

Identifier and outcome: all user-facing, architectural, and developer-facing
documentation is current; both ADRs are written; `CHANGELOG.md` records the
two behaviour changes; `docs/contents.md` indexes every new document; and
issues #451 and #452 are closed with an explanatory comment.

Requirements and gaps: `AGENTS.md`'s documentation-maintenance duties, and the
roadmap-closure requirement.

Acceptance evidence: `make markdownlint` and `make nixie` pass; every new file
is reachable from `docs/contents.md`; both issues are closed.

Conformance check: reconcile every discovery in this plan against the upstream
artefacts before setting the status to `COMPLETE`.

Recovery: documentation-only; revert freely.

Remaining gaps: none.

Compatibility decision: none required.

## Behaviour specifications

The behavioural tests are driven by two Gherkin feature files. Create them
before the corresponding implementation, and keep them synchronized with the
milestones they belong to. Feature-file paths in `#[scenario(path = "...")]`
are relative to the crate root.

### `tests/features/check_mode.feature` (EP-M3)

```gherkin
Feature: Report which Markdown files would be reformatted

  Scenario: A clean file reports no drift and succeeds
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--check" against those files
    Then the exit status is 0
    And the report lists no files
    And the summary reads "1 file left unchanged."
    And no input file was modified

  Scenario: A drifting file is reported with its line counts
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--check" against those files
    Then the exit status is 1
    And the report line for "ragged.md" is "ragged.md +3 -3"
    And no input file was modified

  Scenario: Every supplied file is reported before the tool exits
    Given a Markdown file "clean.md" that is already formatted
    And a Markdown file "ragged.md" with an unaligned table
    And a Markdown file "also-ragged.md" with an unaligned table
    When mdtablefix runs with "--check" against those files
    Then the exit status is 1
    And the report lists "ragged.md" and "also-ragged.md" in argument order
    And the summary reads "2 files would be reformatted, 1 file left unchanged."

  Scenario: An unreadable file yields the error status, not the drift status
    Given a Markdown file "ragged.md" with an unaligned table
    And a path "missing.md" that does not exist
    When mdtablefix runs with "--check" against those files
    Then the exit status is 2
    And standard error mentions "missing.md"

  Scenario: A CRLF file needing no Markdown changes reports clean
    Given a Markdown file "windows.md" that is already formatted with CRLF endings
    When mdtablefix runs with "--check" against those files
    Then the exit status is 0
    And the report lists no files

  Scenario: Check mode rejects being combined with in-place mode
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--check --in-place" against those files
    Then the exit status is 2
    And standard error mentions "cannot be used with"
```

### `tests/features/diff_mode.feature` (EP-M4)

```gherkin
Feature: Show what would change in Markdown files

  Scenario: A drifting file produces a unified diff and still succeeds
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--diff" against those files
    Then the exit status is 0
    And the diff header names "ragged.md" on both sides
    And the diff contains a hunk header
    And no input file was modified

  Scenario: A clean file produces no diff
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--diff" against those files
    Then the exit status is 0
    And standard output is empty

  Scenario: The emitted diff reconstructs the formatted file
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--diff" against those files
    And the diff is applied to a copy of "ragged.md"
    Then the patched copy matches the output of "--in-place"

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
files, with the scenario bindings in `tests/bdd_reporting.rs`. Scenario state
is held in an `rstest` fixture rather than a global world, following the
`rstest-bdd` guidance that "fixtures are the world". A `#[derive(ScenarioState)]`
struct carrying `Slot<TempDir>`, `Slot<Vec<Utf8PathBuf>>`, and
`Slot<std::process::Output>` is sufficient. The `When` steps run the real
binary through `assert_cmd::Command::cargo_bin("mdtablefix")` and capture
`Output` so both streams and the status can be asserted; note that the
`rstest-bdd` user's guide does not cover subprocess testing, so this harness is
this repository's own convention and must be documented in
`docs/developers-guide.md`.

## Concrete steps

Run every command from the repository root,
`/home/leynos/.lody/repos/github---leynos---mdtablefix/worktrees/cfdaa2f9-abd0-4c67-9f5f-0531a93ac8e6`.

Log every gate to `/tmp` so truncated console output can be reviewed:

```bash
make test 2>&1 | tee "/tmp/test-mdtablefix-$(git branch --show-current).out"
```

Substitute `check-fmt`, `typecheck`, `lint`, `markdownlint`, or `nixie` for
`test` and change the log prefix to match. Do not run gates in parallel; this
environment relies on build caching and sequential runs are fastest.

### EP-M0

1. `cargo add similar@2.7` and confirm `cargo tree --duplicates | grep similar`
   prints nothing.
2. Write a scratch test that prints the unified diff for a two-line ragged
   table; run `cargo test --lib -- --nocapture spike` and paste the output into
   `Artefacts and notes`.
3. Add the four development dependencies, create
   `tests/features/canary.feature` with one trivial scenario and a matching
   step file, and run `cargo test --test bdd_reporting`. Confirm that deleting
   a step definition produces a compile error, proving
   `strict-compile-time-validation` is active.
4. Delete the scratch test and the canary feature. Commit the dependency
   additions alone.

### EP-M1

1. Red: add `src/document.rs` with the module doc, the type signatures, and
   `todo!()` bodies, plus its unit tests and `tests/document_properties.rs`.
   Run `cargo test --lib document` and expect panics from `todo!()`, which is
   the intended red failure.
2. Green: implement `LineEnding::detect`, `SourceDocument::parse`, and
   `render`. Re-run until green.
3. Refactor: replace the join logic in `src/main.rs:129-133` and
   `src/io.rs:21-25` with calls into `document`. Run the full suite; every
   pre-existing test must still pass.
4. Add a CRLF fixture under `tests/data/` and an end-to-end `--in-place` test
   asserting CRLF survives.
5. Run all five gates and commit.

### EP-M2

1. Red: create `src/check.rs`, `src/check/delta.rs`, `src/check/render.rs`
   with signatures and `todo!()` bodies; write `tests/check_properties.rs` and
   the unit tests. Observe the red failure.
2. Green: implement `LineDelta::between` over
   `TextDiff::from_lines(original, formatted).iter_all_changes()`, counting
   `ChangeTag::Insert` and `ChangeTag::Delete`; implement the three rendering
   functions.
3. Add the negative controls named in `Verification plan` as temporary local
   mutations, confirm each property fails for the intended reason, then revert
   the mutations. Record the observed failure messages in
   `Artefacts and notes`.
4. Run all gates and commit.

### EP-M3

1. Red: write `tests/features/check_mode.feature`, `tests/steps/reporting.rs`,
   `tests/bdd_reporting.rs`, and `tests/cli_check.rs`. They will fail to
   compile because `--check` does not exist; that is the red state. Record the
   exact compiler error.
2. Green: add `src/app.rs`; add the `mode` argument group and `--check` to
   `Cli`; implement `CapabilityStore`; change `fn main` to return `ExitCode`.
3. Add the `src/app.rs` unit tests using an in-memory store and a
   panic-on-write store.
4. Confirm `cargo test --test cli_check` passes and the exit statuses are 0, 1,
   and 2 in the three intended situations.
5. Run all gates and commit.

### EP-M4

1. Red: write `tests/features/diff_mode.feature` and `tests/cli_diff.rs`;
   observe the failure.
2. Green: add `--diff` to the `mode` group and the `Mode::Diff` arm of `apply`.
3. Run all gates and commit.

### EP-M5

1. Extend `tests/cli_matrix/support.rs`'s execution-mode expansion from two
   modes to four, and update the harness self-tests that assert the expansion
   is complete.
2. Regenerate snapshots with
   `INSTA_UPDATE=always cargo test --test cli_matrix cli_matrix_snapshots`,
   then review every changed `.snap` file before staging it. Do not accept
   snapshots mechanically.
3. Update `docs/developers-guide.md`'s CLI matrix section in the same commit.
4. Run all gates and commit.

### EP-M6

1. Evaluate the Verus toolchain against the go/no-go criteria before writing
   any proof.
2. If proceeding, write the proof, inspect its trusted assumptions, and
   document how to run it.
3. If abandoning, record the decision and the residual gap.

### EP-M7

1. Add a command-line interface section to `docs/users-guide.md` covering every
   flag, the exit-status contract, line-ending behaviour, the trailing-newline
   rule, and the lone-carriage-return limitation.
2. Reduce `README.md`'s flag list to a synopsis plus a link to the user's
   guide, updating the usage line to include `--check` and `--diff`.
3. Add a "Check and diff reporting" section to `docs/architecture.md` near
   "Concurrency with `rayon`", update the Module Relationships Mermaid diagram
   to include `document`, `check`, and `app`, and update the `## Contents`
   index. Note that the existing diagram already names functions that no longer
   match `src/main.rs`; correct those while editing it.
4. Add sections to `docs/developers-guide.md` covering the `DocumentStore`
   port and its re-use policy, the `Assessment` single-source-of-truth rule,
   the `rstest-bdd` conventions adopted here, and the subprocess step harness.
5. Write `docs/adrs/0006-check-and-diff-reporting.md` and
   `docs/adrs/0007-line-ending-preservation.md`, following ADR 0004's header
   format: `# Architectural decision record (ADR) 000N: <title>`, then
   `## Status`, `## Date`, `## Context and problem statement`.
6. Copy `docs/rstest-bdd-users-guide.md` and
   `docs/reliable-testing-in-rust-via-dependency-injection.md` in from their
   canonical locations in the sibling repositories.
7. Add `CHANGELOG.md` entries for `--check`, `--diff`, line-ending
   preservation, and the exit-status change.
8. Update `docs/contents.md` to index every new document, and add the missing
   entry for `docs/state-machine-abstractions-roadmap.md`.
9. Run `make markdownlint` and `make nixie`, then all Rust gates, and commit.
10. Close issues #451 and #452 with a comment linking this plan and explaining
    the `--concise` supersession.

## Validation and acceptance

Acceptance is behavioural. A reviewer should be able to reproduce each of the
following without reading any source code.

Build the binary once:

```bash
cargo build --bin mdtablefix
export MDT=./target/debug/mdtablefix
```

Prepare fixtures:

```bash
printf '| A | B |\n| --- | --- |\n| 1 | 2 |\n' > clean.md
printf '|A|B|\n|---|---|\n|1|2|\n' > ragged.md
cp ragged.md ragged.md.orig
```

A clean file reports nothing and succeeds:

```console
$ $MDT --check clean.md; echo "status=$?"
1 file left unchanged.
status=0
```

A drifting file is reported and fails:

```console
$ $MDT --check ragged.md; echo "status=$?"
ragged.md +3 -3
1 file would be reformatted.
status=1
```

Neither mode touches the file:

```console
$ cmp ragged.md ragged.md.orig && echo "unmodified"
unmodified
```

A missing path yields the error status, distinct from drift:

```console
$ $MDT --check missing.md; echo "status=$?"
status=2
```

`--diff` prints a unified diff and succeeds regardless of drift:

```console
$ $MDT --diff ragged.md; echo "status=$?"
--- ragged.md
+++ ragged.md
@@ -1,3 +1,3 @@
-|A|B|
-|---|---|
-|1|2|
+| A | B |
+| --- | --- |
+| 1 | 2 |
status=0
```

The modes are mutually exclusive:

```console
$ $MDT --check --in-place ragged.md; echo "status=$?"
error: the argument '--check' cannot be used with '--in-place'
status=2
```

CRLF input survives in-place formatting:

```console
$ printf '|A|B|\r\n|---|---|\r\n|1|2|\r\n' > windows.md
$ $MDT --in-place windows.md
$ file windows.md
windows.md: ASCII text, with CRLF line terminators
```

### Red, green, refactor evidence

For each milestone, record in `Artefacts and notes`:

- Red: the exact command and its failure, including the compiler error for
  tests that reference a flag that does not yet exist, or the `todo!()` panic
  for functions not yet implemented. A test that passes before the change is
  not a red test and must be strengthened.
- Green: the same command passing after the minimal implementation.
- Refactor: the command sequence and passing result after cleanup.

### Verification evidence

For each obligation in `Verification plan`, record the command, the initial
failure or counterexample, the passing result, and the negative control's
observed rejection. An implementation change that requires a new invariant,
lemma, or axiom must return to `Verification plan` before continuing.

### Quality criteria

- Tests: `make test` passes with no warnings; `RUSTFLAGS="-D warnings"` is
  already set by that target.
- Verification: `INV-PREDICTS`, `INV-NOWRITE`, `LEM-COUNT`, `INV-AGREE`,
  `INV-ENDINGS`, `INV-ORDER`, and `INV-EXIT` are each discharged with the
  evidence and non-vacuity check named against them.
- Lint and typecheck: `make check-fmt`, `make typecheck`, and `make lint` all
  pass. `make lint` includes the static-regex check.
- Documentation: `make markdownlint` and `make nixie` pass.
- Performance: no benchmark threshold applies. `--check` and `--diff` add one
  diff computation per changed file and must not read any file more than once.
- Security: `--check` and `--diff` must not widen the filesystem capability.
  All access continues to flow through `open_file_parent`.

### Quality method

Run the gates sequentially after each milestone:

```bash
make check-fmt && make typecheck && make lint && make test
```

Delegate full gate runs to the `scrutineer` subagent so bulky output stays out
of the planning context, and read the cited `/tmp` log rather than re-running a
gate to diagnose a failure.

## Idempotence and recovery

Every step in this plan is safe to repeat. `cargo add` is idempotent for an
already-present dependency. Test and gate commands are read-only apart from
`target/` and `insta`'s pending-snapshot files. Snapshot regeneration under
`INSTA_UPDATE=always` overwrites `.snap` files, so review `git diff` before
staging and use `git restore tests/snapshots/` to undo an unwanted
regeneration.

Commit after each milestone so any milestone can be reverted with
`git revert` without disturbing its predecessors. The milestones are ordered so
that reverting a later one leaves the repository in a coherent state: reverting
`EP-M4` leaves `--check` working, and reverting `EP-M3` leaves the pure domain
modules unreferenced but correct.

The only step that changes files outside the repository is none; the manual
validation commands above create fixtures in the working directory, so run them
in a scratch directory or delete `clean.md`, `ragged.md`, `ragged.md.orig`, and
`windows.md` afterwards.

## Artefacts and notes

Populate this section during implementation with the spike transcript from
`EP-M0`, the red and green transcripts for each milestone, and the observed
failure messages from each negative control. Keep each excerpt short and
focused on what proves success.

## Documentation and skills to consult

Read these before starting; they are the reason several decisions above are
shaped as they are.

Repository documents:

- `AGENTS.md`: the binding style, testing, and commit rules for this
  repository, including the 400-line file cap and the abstraction, port, and
  helper policy that governs introducing `DocumentStore`.
- `docs/contents.md`: the index to every other document; start here.
- `docs/repository-layout.md`: which directory owns what.
- `docs/documentation-style-guide.md`: en-GB-oxendict spelling, sentence-case
  headings, 80-column prose, language identifiers on every fence, and the ADR
  template.
- `docs/architecture.md`: the existing component narrative, especially
  "Concurrency with `rayon`", which is where the new reporting modes belong.
- `docs/developers-guide.md`: the internal API reference for `src/main.rs`, the
  "callers select the function that matches their intent rather than passing a
  Boolean mode flag" convention, the CLI matrix harness, and the observability
  conventions.
- `docs/adrs/0004-state-machine-abstractions.md`: the ADR format to imitate,
  and the guidance on when to keep logic explicit rather than delegating it.
- `docs/execplans/cli-matrix-testing.md`: the inherited constraints on test
  placement, `.dat` fixtures, and snapshot discipline.
- `docs/rust-testing-with-rstest-fixtures.md`: fixture and parameterization
  patterns for the unit tests.
- `docs/rust-doctest-dry-guide.md`: how to write the doctests for the new
  public API without duplicating test logic.
- `docs/trailing-spaces.md`: background on trailing-space preservation, which
  is distinct from the trailing-newline rule this plan touches.

Documents referenced by the task that are absent here and are copied in by
`EP-M7`:

- `docs/rstest-bdd-users-guide.md`, canonical copy at
  `github---leynos---repovec-appliance/.../docs/rstest-bdd-users-guide.md`.
  Needed because this plan is the first adoption of `rstest-bdd` in this
  repository.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`, canonical copy
  at `github---leynos---evert/.../docs/`. Prescribes generic `&impl Trait`
  injection rather than `dyn`, which is the style `assess` and `apply` follow.

Documents referenced by the task that are absent and are not applicable:

- `docs/netsuke-design.md`: no Netsuke-specific policy is adopted by this plan.
  Issue #441 tracks aligning this repository with the Netsuke lint baseline
  separately.
- `docs/ortho-config-users-guide.md`: this repository parses arguments with
  plain `clap`, not `ortho-config`, and this plan does not change that.

Skills to load:

- `rust-router` first, then the smallest useful follow-on skill for the
  question at hand.
- `hexagonal-architecture` for the port and adapter boundary, used to protect
  the domain from `cap_std`, `clap`, and `std::io`, not to impose a directory
  layout.
- `rust-unit-testing` for fixture shape, table tests, and choosing between
  equality, matcher, and snapshot assertions.
- `proptest` for the property tests, especially generator design and shrinking
  discipline.
- `rust-errors` for the `Result` shape of the port and the error-versus-drift
  distinction.
- `arch-decision-records` for the two ADRs.
- `rust-types-and-apis` when shaping `Assessment`, `Mode`, and `Outcome`.
- `verus` only if `EP-M6` proceeds.
- `en-gb-oxendict` for all prose.
- `commit-message` when committing.
- `codegraph-mcp` for structural questions about callers and blast radius
  before editing.

## External references

- GitHub issue #452, the check-mode requirement:
  <https://github.com/leynos/mdtablefix/issues/452>.
- GitHub issue #451, the line-ending requirement:
  <https://github.com/leynos/mdtablefix/issues/451>.
- `similar` crate documentation, for `TextDiff::from_lines`,
  `iter_all_changes`, `ChangeTag`, and `UnifiedDiff`:
  <https://docs.rs/similar/2.7.0/similar/>.
- Black's documented `--check` and `--diff` semantics, the closest prior art
  for the exit-status and reporting design:
  <https://black.readthedocs.io/en/stable/usage_and_configuration/the_basics.html>.
- `rstest-bdd`, the behavioural test framework adopted here:
  <https://github.com/leynos/rstest-bdd>.

## Revision note

Initial draft, 2026-09-09. Establishes the two reporting modes, brings
line-ending preservation into scope because byte-exact comparison makes it a
prerequisite rather than an enhancement, and records the supersession of the
`--concise` flag proposed in issue #452. No implementation has begun; the plan
awaits approval.
