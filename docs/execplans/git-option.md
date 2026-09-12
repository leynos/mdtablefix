# Add a `--git` option that selects repository files to format

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: COMPLETE — 2026-09-12, at commit `6384543`, since rebased onto
`origin/check-option` four times the same day as the base kept moving — onto
`64117e4`, then `49a2d0a`, then `e462b97`, and finally `6be1a76`, each recorded
in `Progress` with the measurement of what it moved — on branch `git-option`,
stacked on pull request #464 and carried by pull request #466, which was marked
ready for review the same day while its base #464 remained open. The
prerequisite extraction of `Cli` and `FormatOpts` landed, in the base's
`src/command.rs` rather than in a `src/cli.rs` of this branch's own, and Stage A
is discharged.
All four milestones — EP-M0's grammar measurement, EP-M1's selection tree,
EP-M2's command-line surface, and EP-M3's documentation — are delivered. Every
deterministic gate is green on the milestone tree, the six CodeRabbit rounds
recorded under `Artefacts and notes` returned zero findings, all four CI runs
since the branch's conflict with its base was cleared are green on Linux and
Windows, and `Outcomes & retrospective` states what was delivered against what
was planned and which pinned interfaces the implementation superseded.

One thing this plan still owes, recorded rather than implied away. The branch's
first CI run was red on both jobs over five defects that are all in test code —
a fixture identity the helper never supplied, exit-status and path text that
holds only where the platform spells them the way Linux does, and a snapshot
carrying the executable suffix — and `6384543` answers all five. Those answers
are now measured on a second platform rather than promised: the first two runs
the branch could obtain after its conflict was cleared, `34691011897` and then
`34691436697` on the pushed tip, each concluded `success` with all six jobs
green, so the CI verdict that this plan spent the longest waiting on is no longer
outstanding, and the claim that carries it is about the shipped tree rather
than the head — every file this branch ships is CI-verified on Linux and Windows.
What remains
outstanding is the one
verification no machine here can perform — the macOS and Windows half of the
`canonicalize` case question behind INV-DEDUP — which is carried forward rather
than discharged, with ADR 0010's known-risks section holding it and the fallback
named in this plan's `Verification plan`. That one is not a gap in the work: it
is a limit of the platform, and the fallback is named so a reader meeting it
knows what to do rather than only what is unproven.

The plan's remaining work is not this plan's: pull request #464 must merge
before this one can. See `Progress`, `Outcomes & retrospective`, and
`Conformance basis`, "Related work".

## Purpose / big picture

Today `mdtablefix` only formats the files a user names on the command line, so
tidying a repository means writing a shell pipeline and remembering to exclude
`target/`, `node_modules/`, and anything else `.gitignore` covers. Get that
pipeline wrong and the tool rewrites build output or, worse, source code.

After this change a user standing anywhere inside a Git working tree can run:

```console
mdtablefix --git --in-place --wrap
```

and every Markdown file Git tracks beneath the current directory is reflowed
in place. Adding `--include-untracked` extends the selection to untracked files
Git does not ignore, making the pair exactly equivalent to
`git ls-files --cached --others --exclude-standard`.

Success is observable without reading any code:

- `mdtablefix --git --list-files` prints the paths that would be acted on, one
  per line, and touches nothing.
- `mdtablefix --git --in-place` reformats tracked Markdown and leaves ignored
  Markdown, untracked Markdown, and `.rs` sources byte-identical.
- `mdtablefix --git --include-untracked --in-place` also reformats untracked
  Markdown, still leaving ignored files alone.
- `mdtablefix --git --md-exts mdc,markdown --in-place` acts only on files with
  those extensions.
- `mdtablefix --git` outside a Git repository exits non-zero with a diagnostic
  and modifies nothing.
- `mdtablefix --git some-file.md` is rejected at argument parsing, because the
  two ways of choosing files are mutually exclusive.

## Context and orientation

`mdtablefix` is one Rust package: a library crate rooted at `src/lib.rs`, a
binary crate rooted at `src/main.rs`, and a proc-macro helper at
`test-macros/`. The package version is `0.5.1`, so it is pre-1.0 and carries no
source-compatibility obligations.

Read these before starting, in this order:

- `AGENTS.md` — binding style, error-handling, testing, and commit rules. In
  particular lines 232 (use `cap_std`/`camino` in place of `std::fs`/
  `std::path`) and 264 to 269 (prefer `thiserror` domain enums; never export an
  opaque error type from a library).
- `docs/contents.md` — the documentation index.
- `docs/architecture.md` — the processing pipeline and the `rayon` concurrency
  model.
- `docs/developers-guide.md` — the capability-scoped filesystem boundary and
  the observability rules.
- `docs/documentation-style-guide.md` — British English with Oxford spelling,
  prose at 80 columns, a language identifier on every fenced block, captioned
  figures.
- `docs/rust-testing-with-rstest-fixtures.md` — the fixture-first `rstest`
  style. Note it says nothing about `googletest` or `pretty_assertions`; the
  guidance for those is in the `rust-unit-testing` agent skill.

Load these agent skills: `rust-router`, then `rust-unit-testing` for test
shapes, `rust-errors` for the error enums, and `arch-crate-design` for the
module boundary; `hexagonal-architecture` for the port split; `proptest` for
the property tests; `arch-decision-records` for the ADR; `en-gb-oxendict` for
prose.

### How file selection works today

`src/main.rs` is 303 lines. Its `Cli` struct has one positional field and one
cross-field constraint:

```rust
#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", requires = "files")]
    in_place: bool,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}
```

`main` branches on `cli.files.is_empty()`. If empty, it reads standard input.
Otherwise it maps `cli.files.par_iter()` through `open_file_parent`, which
`src/main.rs` documents as "the only ambient filesystem boundary for CLI file
processing": it opens the file's parent directory as a `cap_std::fs_utf8::Dir`
and returns a relative `camino::Utf8PathBuf`. Every read and write goes through
that capability. `report_results` prints per-file errors to stderr and returns
the first error, so one bad file does not abort the others.

There is no glob expansion, no directory walking, and no extension filtering
anywhere in the crate. `walkdir`, `glob`, `ignore`, `git2`, and `gix` are all
absent. Nothing in the repository shells out to `git`.

### Terms used in this plan

- **Tracked file** — a path in Git's index. `git ls-files --cached` lists
  these, including paths whose working-tree copy has been deleted.
- **Untracked file** — present in the working tree, absent from the index.
  `git ls-files --others` lists these.
- **Standard exclusions** — `--exclude-standard` adds `.git/info/exclude`,
  every directory's `.gitignore`, and the user's global excludes file.
- **Gitlink** — an index entry of mode `160000` recording a submodule commit.
  It appears in `ls-files` output as a directory path.
- **Candidate** — a path emitted by the file source before policy is applied.
- **Port** — a trait the domain defines and depends on. **Adapter** — an
  implementation that touches the outside world.

### Measured behaviour of the reference command

These transcripts were captured on Git 2.52.0 while drafting. Treat them as the
specification of "semantically equivalent"; the end-to-end tests re-establish
them against the `git` on the machine.

Given a repository containing committed `tracked.md`, `sub/subtracked.md`,
`sub/deep/deeptracked.md` and `.gitignore`; untracked `untracked.md` and
`sub/subuntracked.md`; an ignored `ignoredfile`; and with `sub/subtracked.md`
deleted from the working tree:

```console
$ git ls-files --cached --others --exclude-standard
sub/subuntracked.md
untracked.md
.gitignore
sub/deep/deeptracked.md
sub/subtracked.md
tracked.md
```

Four facts follow, each driving a design decision.

1. Output is **not globally sorted**. The untracked pass runs first, the cached
   pass second, each sorted within itself. Selection must impose its own order
   to be reproducible.
2. `sub/subtracked.md` is listed although it no longer exists on disk, because
   it is still in the index. Selection must tolerate absent candidates.
3. Run from `sub/`, the command scopes to that subtree and prints paths
   relative to the current directory. `--full-name` would print
   repository-root-relative paths instead. This plan does **not** pass
   `--full-name`, so `--git` is subtree-scoped exactly as the reference command
   is.
4. Without `-z`, a path containing a space, a quote, or a non-ASCII byte is
   C-quoted, for example `"we ird\303\251\"q.md"`. With `-z` the bytes are
   emitted verbatim and NUL-terminated. `-z` is mandatory, not an
   optimization.

During an unresolved merge the same path is emitted once per index stage:

```console
$ git ls-files --cached --others --exclude-standard
c.md
c.md
c.md
```

`--deduplicate` collapses that to one line. This plan passes it, and also
deduplicates in the domain, for the reason given under INV-DEDUP.

Outside a repository the command fails loudly:

```console
$ git ls-files -z --cached
fatal: not a git repository (or any of the parent directories): .git
exit=128
```

`git ls-files -t` is **not** a usable substitute for a filesystem check: a
tracked file deleted from the working tree reported `H`, not `R`, because the
`R` tag requires `--deleted`. See Surprises & discoveries.

## Constraints

Hard invariants. If satisfying the objective would violate one, stop and
escalate rather than working around it.

- **CON-SAFE-001** — `--git` must never cause bytes to be written to any inode
  other than a regular file that the configured `git ls-files` invocation
  reported and whose extension is in the configured set. This is stated in
  terms of **inodes written**, not paths selected, because a symlink is a
  reported path that writes to a different inode. Rewriting a `.rs` file, an
  ignored file, or a file outside the selection is a defect of the highest
  severity.
- **CON-CAP-001** — All file **content** reads and writes continue to flow
  through the capability boundary that `docs/developers-guide.md` mandates:
  `open_file_parent` returns a `cap_std::fs_utf8::Dir` and a relative
  `camino::Utf8Path`. Selection code may call `std::fs::symlink_metadata` to
  classify a candidate; that is a deliberate, narrow carve-out from
  `AGENTS.md:232`, recorded in the Decision log, and it must not extend to
  reading or writing contents.
- **CON-DEP-001** — No new dependency that performs Git operations or
  filesystem walking. `git2`, `gix`, `ignore`, `walkdir`, and `glob` are all
  excluded; the selected mechanism spawns `git` through `std::process::Command`
  from the standard library. `thiserror` is permitted and expected, because
  `AGENTS.md:264` mandates it for domain error enums.
- **CON-API-001** — This change adds **no** public library API. The selection
  module tree is private to the binary crate. `src/lib.rs` is not modified.
  This satisfies `AGENTS.md:268` ("never export the opaque type from a
  library") without argument, and avoids a permanent semver commitment for a
  single in-package consumer.
- **CON-SIZE-001** — No source file may exceed 400 lines. `src/main.rs` is
  already 303 lines and `src/process.rs` is 376.
- **CON-LINT-001** — `cargo clippy --workspace --all-targets --all-features
  -- -D warnings` must pass. Any `#[expect]` must carry a `reason`.
- **CON-OBS-001** — Selection code emits `debug!` and `trace!` only, and
  tracing fields must never contain document or path content. Note the
  corollary: tracing structurally cannot answer "why did it pick *that* file";
  that is what `--list-files` is for. Do not try to satisfy it with `trace!`.
- **CON-DOC-001** — Documentation follows `docs/documentation-style-guide.md`.

## Tolerances (exception triggers)

Stop and escalate when any of these is reached.

- **Scope** — more than 24 files touched, or more than 1600 net added lines
  across the whole plan. (These numbers are counted against the file list in
  "Interfaces and dependencies" plus five documentation files, an ADR, and this
  plan. An earlier draft set 14 and 900, which was arithmetically
  unsatisfiable.)
- **Dependencies** — any dependency beyond the five named in "Interfaces and
  dependencies". Anything matching CON-DEP-001's exclusion list is a hard stop,
  not a tolerance.
- **Interface** — any change to an existing public library item, or to the
  meaning of an existing CLI flag other than the documented `--in-place`
  relaxation in REQ-GIT-005.
- **File size** — `src/main.rs` reaching 380 lines. Contingency under Risks;
  take it and continue.
- **Iterations** — a gate still failing after four fix attempts.
- **Ambiguity** — any point where two readings of this plan would produce
  materially different user-visible behaviour.
- **Semantics** — any divergence between the implemented selection and the
  reference command that this plan does not record as deliberate.

## Risks

- Risk: `git` is absent from `PATH`, so `--git` fails at runtime even though
  `mdtablefix` installed cleanly.
  Severity: medium. Likelihood: low.
  Mitigation: map `io::ErrorKind::NotFound` to a dedicated message naming
  `git` and `PATH`, covered by a test injecting a non-existent program through
  `GitLsFiles::with_program`. `--git` is opt-in and its premise is a Git
  working tree, so the requirement is reasonable.

- Risk: `rstest-bdd` was new to this repository.
  Severity: low now. Likelihood: low.
  Mitigation: **largely discharged.** Pull request #464 already adds
  `rstest-bdd = "0.5.0"` and
  `rstest-bdd-macros = { version = "0.5.0", features =
  ["strict-compile-time-validation"] }`, and ships passing feature files, so
  the toolchain question is answered. Note the explicit macros crate and the
  feature: match them rather than re-deriving them. EP-M0's BDD half is
  therefore reduced to confirming a scenario in this plan's own feature file
  runs.

- Risk: **`--code-emphasis` reaches a fixed point on the second pass, not the
  first**, so a single `--in-place` run does not settle it. Measured on this
  branch at `408c76a`: formatting `tests/data/cli-matrix/table-prose.dat` with
  `--code-emphasis --in-place` shortens emphasis markers on pass one without
  re-padding the table, and pass two narrows the two affected column rules by
  two characters. Pass three is identical to pass two, so it converges — it
  does not drift forever.

  This matters for `--git --check` as a gate: after one `--in-place` pass over
  a repository, a `--check` run would still report drift on such a file, and a
  check-fix-check loop needs two fixes rather than one.

  Severity: medium, and only for `--code-emphasis` users. Likelihood: low —
  one of 110 fixtures swept, and `make fmt`'s flag set excludes the flag.
  Mitigation: **not this plan's defect and not this plan's to fix.** Tracked
  as issue #478, raised from this plan's measurement, which also records the
  root cause and the `--ellipsis` precedent for the fix.
  `tests/idempotence_drift.rs` gates the `make fmt` flag set and that set plus
  `--headings`; `--code-emphasis` is deliberately outside both. Do not add it
  to a corpus-wide drift gate as part of this work. Document `--git --check`
  as a gate for the gated flag sets, and do not claim one-pass convergence for
  `--code-emphasis`.

- Risk: `src/main.rs` on `check-option` is already **386 lines** against the
  400-line cap, before this plan adds a single field.
  Severity: high. Likelihood: certain.
  Mitigation: this is no longer a contingency but a required first step. The
  `Cli` and `FormatOpts` declarations occupy roughly lines 38 to 120 of that
  file; extract them into a binary-private `src/cli.rs` declared by
  `main.rs`, which returns it to roughly 300 lines and leaves room for the
  five new fields and the composition wiring. As delivered the module is the
  base's `src/command.rs`, which does the same job and more — see
  `EX-REBASE-STRUCTURE`. Do the extraction as its own
  commit, with no behaviour change, before EP-M2 adds anything. `src/driver.rs`
  at 369 lines has the same little headroom, so `--git` wiring must go in
  `src/select/`, not there.

- Risk: a repository contains a path that is not valid UTF-8.
  Severity: low. Likelihood: low.
  Mitigation: `driver::Inputs::resolve` now fails the whole run on a non-UTF-8
  argument, so selection must drop such candidates **before** constructing
  `Inputs`, counting them and emitting one content-free stderr warning. This
  keeps the disproportionate outcome — a whole-repository operation aborted by
  one stray filename elsewhere in the tree — off the `--git` path without
  disturbing the positional-argument contract #464 established.

- Risk: the extension filter is the only thing between `--git` and rewriting
  source files, so a defect there is destructive.
  Severity: high. Likelihood: low.
  Mitigation: verify the filter in both directions rather than only checking
  that selected paths look right. See INV-EXT-SOUND and INV-EXT-COMPLETE.

## Conformance basis

There is **no Terms of Reference document and no separate technical design
document** for this feature. Do not invent one. The governing upstream
artefacts that exist are `AGENTS.md` (at commit `c792270`),
`docs/developers-guide.md`, `docs/documentation-style-guide.md`, and the
accepted ADRs (none of which constrains file selection). This plan creates the
missing design record as **ADR 0010**.

### Related work

The sequencing question that previously blocked this plan is settled: this work
comes **after** pull request #464. Three of its dependencies have merged, and
pull request #464 itself is still open, so implementation proceeds by
**branching from `check-option`** rather than waiting for the merge. See the
Decision log, "stack this work on `check-option`", and Surprises &
discoveries.

**Merged to `main`, and now load-bearing for this plan:**

- **#467** (issue #465), atomic in-place writes. `mdtablefix::io::replace_file`
  writes a temporary file beside the target, applies the target's permissions,
  and renames over it. It reads `symlink_metadata` and **declines a symbolic
  link** with `io::ErrorKind::InvalidInput`. Both the CLI's `rewrite_in_place`
  and the library's `rewrite_with` route through it. Documented in
  `docs/architecture.md`, "Atomic in-place writes", with Figure 6.
- **#469** (issue #451), line-ending preservation. `LineEnding`,
  `LineEndingCounts`, `count_line_endings`, `detect_line_ending`, and
  `serialize_lines` are now **public library API**, re-exported at the crate
  root. ADR 0007 records the rationale. This plan must not reimplement any of
  it.
- **#470** (issue #468), single-pass idempotence, with ADR 0006.

Those merges took the ADR numbers 0006 and 0007, and the numbers have moved
twice since this plan was drafted: **0008** is now byte-order-mark preservation
(delivered on `check-option`) and **0009** is check and diff reporting (pull
request #464). This plan's decision record is therefore **ADR 0010**. The
earlier drafts of this plan said 0008; every reference in this document has
been renumbered, and EP-M3 must create `docs/adrs/0010-git-file-selection.md`.

**In review, and a prerequisite for this plan:** pull request #464 adds
`--check` and `--diff`. Its commit `83e6150`, "Keep the reporting shape open to
a second input source", implements the four forward-compatibility requests this
plan made, so the interfaces this plan consumes now exist:

| Interface on `check-option` | How this plan uses it |
| --- | --- |
| `ArgGroup "inputs"` holding `files`, with `mode` requiring `inputs` | `git` joins `inputs`; `--git --check`, `--git --diff`, `--git --in-place` all parse |
| `driver::Inputs { Stdin, Files(Vec<Utf8PathBuf>) }` | `--git` becomes a second source producing `Inputs::Files`; an empty selection exits 0 without reading stdin |
| `driver::Mode { Print, InPlace, Check, Diff }` | `--list-files` becomes a fifth variant |
| `driver::ReadOnlyDir` | every mode reads through it, because `analyse` clones the caller's capability and wraps the clone; `--list-files` returns one statement earlier still, before the clone, so it holds no read capability at all. See the Decision log for what that does and does not guarantee |
| `driver::Assessment::is_changed`, and `write_back` skipping unchanged files | discharges what this plan called INV-NOWRITE-UNCHANGED; this plan no longer implements it |
| `driver::{ExitStatus, exit_status}` | `--git` failures map to `ExitStatus::Error`, exit code **2**, not 1 |
| `driver::in_argument_order` | re-establishes ordering without relying on the withdrawn `AX-RAYON-ORDER` |

*Table 1: interfaces this plan consumes from pull request #464.*

Pull request #464 also adds `googletest`, `pretty_assertions`, `rstest-bdd`,
and `rstest-bdd-macros` as development dependencies, adds
`cargo test --doc --all-features` to the `test` target, and bumps the crate to
`0.6.0`. All four were items this plan intended to introduce; none of them is
this plan's work any more.

**Two consequences of `83e6150` that this plan must accommodate**, both
recorded by its author rather than left to be discovered:

1. `Inputs::resolve` now fails the **whole run** when any argument is not valid
   UTF-8, rather than counting it as one file's error. This plan's
   drop-and-count posture for non-UTF-8 paths therefore has to happen during
   selection, before a path reaches `Inputs`. See the note under
   "Interfaces and dependencies".
2. A symlink pointing at a file that is already clean is no longer declined,
   because `write_back` attempts no replacement. The refusal still fires for a
   drifting target. That makes the symlink error **intermittent**, which is a
   further argument for excluding symlinks at selection time rather than
   relying on the write boundary to reject them.

**Roadmap**: this repository has no general-purpose roadmap. The two roadmap
documents that exist are feature-scoped and neither mentions `--git`,
`git ls-files`, or file selection. The instruction to mark a roadmap entry as
done is therefore **not applicable**; do not create a roadmap entry to satisfy
it. ADR 0010 is the durable record.

Requirements, traced through milestones to evidence:

```plaintext
REQ-GIT-001 -> ADR-0010 -> EP-M1 -> select::git_ls_files::tests::lists_tracked_only
REQ-GIT-002 -> ADR-0010 -> EP-M1 -> select::policy::tests::selects_only_configured_extensions
REQ-GIT-003 -> ADR-0010 -> EP-M2 -> cli_git.rs::md_exts_replaces_the_default_set
REQ-GIT-004 -> ADR-0010 -> EP-M2 -> cli_git.rs::rejects_git_with_explicit_files
REQ-GIT-005 -> ADR-0010 -> EP-M2 -> cli_git.rs::in_place_is_satisfied_by_git
REQ-GIT-006 -> ADR-0010 -> EP-M1 -> select::policy::tests::excludes_missing_other_symlink
REQ-GIT-007 -> ADR-0010 -> EP-M1 -> select::git_ls_files::tests::maps_spawn_and_exit_failures
REQ-GIT-008 -> ADR-0010 -> EP-M2 -> feature::"Exit successfully when nothing is selected"
REQ-GIT-009 -> ADR-0010 -> EP-M2 -> feature::"Refuse to rewrite a conflicted file"
REQ-GIT-010 -> ADR-0010 -> EP-M2 -> feature::"List the selection without acting"
CON-SAFE-001 -> EP-M2 -> feature::"Never write through a symlink"
```

- **REQ-GIT-001** — With `--git`, the candidate set is exactly what
  `git ls-files --cached`, run in the current working directory, reports. With
  `--git --include-untracked` it is exactly what
  `git ls-files --cached --others --exclude-standard` reports.
- **REQ-GIT-002** — Candidates are narrowed to Markdown files. The default
  extension set is `md`, `mdc`, and `markdown`, matched ASCII
  case-insensitively.
- **REQ-GIT-003** — `--md-exts` replaces the default set. A leading dot is
  optional and surrounding whitespace is ignored.
- **REQ-GIT-004** — `--git` and positional file arguments are mutually
  exclusive.
- **REQ-GIT-005** — `--in-place` is satisfied by either positional files or
  `--git`. `--in-place` alone remains an error.
- **REQ-GIT-006** — A candidate that is absent, is not a regular file, or is a
  symlink is skipped silently rather than reported as an error.
- **REQ-GIT-007** — When `git` cannot be spawned, or exits non-zero, the tool
  exits non-zero with a diagnostic naming the cause and relaying `git`'s
  stderr.
- **REQ-GIT-008** — Selecting zero files is success: exit 0, no output, and
  **standard input is not read**.
- **REQ-GIT-009** — When the repository is mid-merge, mid-rebase, or
  mid-cherry-pick, any selected file containing conflict markers is excluded
  from rewriting and named on stderr. `--allow-conflicted` overrides.
- **REQ-GIT-010** — `--list-files` prints the resolved selection, one path per
  line, to stdout and exits 0 without reading or writing any file content.

## Architectural boundaries

Apply `hexagonal-architecture` to protect one boundary, not to restructure the
crate. The rest of `mdtablefix` is a pure text pipeline with no infrastructure
to isolate and stays exactly as it is.

The boundary worth protecting is between **what to select** — which extensions
count, how aliases collapse, what order results come back in, what to do about
a candidate that is not a regular file — and **how candidates are discovered**:
spawning a subprocess, decoding a NUL-delimited byte stream, stat-ing paths.
Without the split, every policy rule can only be tested by building a real Git
repository on disk, which makes the awkward cases (an unresolved merge, a
non-UTF-8 path, a staged deletion, a symlink) tedious to reach.

**One driven port, not two.** An earlier draft also defined a
`RepositoryFileSource` trait. It had one implementor, one call site, and no
test fake anywhere in the verification plan, so it inverted no dependency — it
was the pattern transplant this section exists to avoid. `GitLsFiles` now
exposes `list_candidates` as an inherent method, with `with_program` as the
test seam. The surviving port is:

```rust
/// Reports what a candidate path actually is in the working tree.
pub trait PathProbe {
    fn probe(&self, root: &Utf8Path, path: &Utf8Path) -> PathKind;
}
```

`PathProbe` earns its place: it makes INV-PROBE-EXCLUSIONS testable across all
four `PathKind` variants without a filesystem, and it carries the file identity
that INV-DEDUP needs.

The dependency direction is one sentence: **policy depends on `PathProbe` and
on nothing else; the adapters and `main` depend on policy.**

Every module's `//!` header must state its side of that sentence explicitly —
for example, "This module is the selection domain. It depends on `PathProbe`
and performs no I/O." A maintainer must be able to derive the direction from
the modules alone, without reading ADR 0010.

**Directories are parameters, never ambient state.** Both `list_candidates` and
`probe` take the working-tree root explicitly. Nothing in the selection code
calls `std::env::set_current_dir`, and no test may either: Cargo runs
integration tests as threads in one process, so a chdir is a data race — in the
very suite whose job is proving CON-SAFE-001 does not rewrite the wrong files.
The composition root resolves the current directory once, at the boundary,
exactly as `open_file_parent` does today.

## Verification plan

### Non-trivial axioms

Assumed, not verified. Do not test third-party internals; exercise this
repository's logic against the real interface at the boundary.

- **AX-GIT-LSFILES** — `git ls-files -z --deduplicate --cached
  [--others --exclude-standard]` writes to stdout exactly the index paths
  (optionally unioned with non-ignored untracked paths), each NUL-terminated,
  unquoted, verbatim, relative to the process working directory. Basis: the
  `git-ls-files` manual page and the transcripts above. Boundary evidence: a
  real-`git` test in `src/select/git_ls_files.rs`.
- **AX-GIT-EXIT** — `git` exits 0 on success, including on an empty selection,
  and non-zero with a stderr diagnostic otherwise (128 outside a repository).
- **AX-GIT-NLS** — `git`'s diagnostics are **localised** when built with NLS
  and vary across versions. Therefore no test and no document may assert on
  git's own message text. Assert on our wrapper's wording and the exit status;
  relay git's stderr without depending on it.
- **AX-CAPSTD** — `cap_std::fs_utf8::Dir` behaves as documented. Already
  relied upon.
- **AX-RAYON-ORDER** — **withdrawn; this is not an axiom.** An earlier draft
  asserted that `par_iter().map(...).collect::<Vec<_>>()` preserves input
  order, on the grounds that the existing code already relies on it. The
  concurrent plan for `--check` and `--diff` (pull request #464) researched the
  same question and refuted it: `rayon` does not document order preservation
  for `collect`, and `rayon-1.12.0/src/iter/from_par_iter.rs:24-34` routes
  through `par_extend` with no ordering statement. The existing reliance is
  therefore a latent defect, not a contract. Selection must not depend on it:
  each unit of work carries its index and results are ordered on that index.
  See the note on ordering under EP-M2.
- **AX-CLAP-GRAMMAR** — partially **falsified during planning**, and the
  surviving parts are now **measured rather than assumed**. Verified against
  clap 4.6.6: adding `git` to the `inputs` group with `multiple(false)` still
  accepts `mdtablefix a.md b.md` and `--in-place a.md b.md`, rejects
  `--git a.md`, and accepts `--git --check`, `--git --diff`, and
  `--git --in-place`; and `mode` requiring `inputs` rejects a mode flag used
  alone. **Disproved**: `requires = "git"` on a bool flag, which admits
  `--list-files a.md`. Every such dependency now uses a post-parse check; see
  "Command-line surface". The `inputs` group itself is no longer this plan's to
  create — pull request #464 added it at this plan's request in commit
  `83e6150`.

### Obligations

Full detail is given only where the obligation is subtle. The rest are
one-liners on purpose.

**INV-NUL-SPLIT** — splitting the adapter's output is a faithful inverse of
Git's framing: for any list of non-empty byte strings containing no NUL,
splitting their NUL-terminated concatenation returns that list; splitting empty
input returns an **empty list**, not a list holding one empty path.

- Method: `proptest` round-trip, plus `rstest` cases for empty input, a single
  entry, a missing trailing NUL, and a non-UTF-8 entry.
- Rationale: the one place a byte-level framing mistake silently produces a
  wrong file set. The empty-input case is the classic off-by-one that a naive
  `split(b'\0')` gets wrong by yielding `[""]`. CON-SAFE-001 depends on it.
- Domain: 0 to 20 byte strings of length 1 to 40 excluding NUL, including
  non-ASCII bytes.
- Artefact: `src/select/git_ls_files.rs` `mod tests`.
- Non-vacuity: the empty-list case is generated and asserted explicitly, and
  the generator produces non-UTF-8 sequences so the drop-and-count path is
  reached. Negative control: a plain `bytes.split(|b| *b == 0)` without the
  trailing-empty guard must fail the empty-input case.

**INV-EXT-SOUND** — every selected path's ASCII-lowercased extension is in the
configured set. **INV-EXT-COMPLETE** — every candidate whose lowercased
extension is in the set and which probes as `RegularFile` appears in the
output.

- Method: one `proptest` asserting both directions, with a fake `PathProbe`.
- Rationale: these are the two halves of the selection contract. Stating only
  the first admits an implementation that selects nothing; stating only the
  second admits one that selects everything. CON-SAFE-001 needs both.
- Domain: 0 to 30 candidates drawn from a pool mixing matching extensions,
  non-matching extensions (`rs`, `toml`, `png`), extensionless names, dotfiles,
  and nested directories; probe verdicts drawn from all four `PathKind`
  variants.
- Artefact: `src/select/policy.rs` `mod tests`.
- Non-vacuity: the property classifies each case and the run must observe
  non-empty selections, empty selections, and at least one case where a
  matching extension is excluded solely because of the probe verdict.

**INV-DEDUP** — no two selected paths denote the same directory entry.

- Method: `rstest` cases over a fake probe returning colliding
  `FileIdentity` values, covering the three-stage merge listing and a pair of
  paths differing only in ASCII case.
- Rationale, **substantially revised** now that #467 has merged. An earlier
  draft justified this invariant with a data-loss scenario: two entries naming
  one file on a case-insensitive filesystem, formatted concurrently by `rayon`,
  where one thread truncates while the other reads and the file ends up zero
  bytes with the run exiting 0. **That scenario is now closed at the write
  boundary.** `replace_file` writes a temporary file and renames it over the
  target, so there is no truncation window and no reader can observe an empty
  file. The worst remaining outcome is a redundant format and a last-writer-wins
  result, both of which are correct content.

  The invariant survives at much lower severity, for three honest reasons:
  `git ls-files` genuinely emits a conflicted path once per index stage, so
  duplicates are real; formatting the same file twice is wasted work
  proportional to repository size; and `--list-files` printing a path twice
  would be a visible defect. Do not justify it with the data-loss story.

- **`FileIdentity` must be the canonicalized path, not `(st_dev, st_ino)`.**
  This is the reverse of an earlier draft and follows directly from #467.
  Consider two hard links to one inode. Before atomic replacement they aliased,
  so collapsing them was right. After it, replacing the first creates a new
  inode that takes over the first name, and the second link still refers to the
  original, unformatted content — the migration guide states this outright:
  "handles, hard links, and watches tied to the previous file keep the old
  contents and do not follow the replacement". Collapsing them on inode would
  therefore format one and silently leave the other stale, which is worse than
  formatting both. A case-insensitive alias is one directory entry and
  canonicalizes to one path; two hard links are two entries and canonicalize to
  two. `std::fs::canonicalize` draws exactly the line this invariant needs.
- Domain: the merge-stage listing; `README.md` against `Readme.md`; and a
  hard-link pair that must **not** collapse.
- Artefact: `src/select/policy.rs` `mod tests`.
- Non-vacuity: each case asserts the collision was present in the input. The
  hard-link case is the negative control for the identity choice: an
  implementation keyed on `(st_dev, st_ino)` must fail it.
- Residual gap — **carried forward, not discharged** (2026-09-12): whether
  `std::fs::canonicalize` normalizes case on macOS APFS and on Windows is
  asserted here from documentation, not measured — this machine is Linux. EP-M1
  was to confirm it on the release matrix, both of which are release targets as
  of commit `1f64236`, and no machine in this environment can: the branch's CI
  runs the suites on Linux and Windows, its Windows job has no
  case-insensitive-filesystem case to appeal to, and it has no macOS job at
  all. The check therefore stands open. ADR 0010 records it among its known
  risks rather than closing it, and this bullet stays as the plan's own record
  that the milestone was completed with one verification unmet. If it does not
  hold, fall back to comparing `(st_dev, st_ino)` **plus** parent-directory
  identity, and record the change here. The consequence of being wrong is
  bounded and is stated in the bullet above: a missed dedup formats one file
  twice and the last write wins, which is correct content either way — never
  data loss, and never a file reported clean that is not.

**INV-ORDER-DET** — selection is a deterministic function of the candidate
multiset: permuting the input does not change the output, and the output is
sorted byte-wise on the UTF-8 path.

- Method: `proptest` comparing a generated list against a shuffled copy.
- Rationale: `git ls-files` output is not globally sorted, and the order is
  user-visible because `--list-files` and stdout concatenation both follow it.
  Only the permutation-invariance half carries information; "sorted" checked
  with the same `Ord` the implementation sorts by proves nothing, so the
  ordering is stated explicitly instead.
- Artefact: `src/select/policy.rs` `mod tests`.
- Non-vacuity: the property asserts the permutation actually differed and that
  at least one output had two or more entries.

**INV-PROBE-EXCLUSIONS** — a candidate is selected only if the probe reports
`RegularFile`; `Missing`, `Other`, and `Symlink` are excluded.

- Method: `rstest` parameterized over all four variants, each with a matching
  extension so exclusion is attributable to the verdict alone.
- Rationale, **revised** now that #467 has merged. An earlier draft justified
  the `Symlink` variant as preventing a destructive write: `std::fs::metadata`
  follows symlinks, so a tracked `src/notes.md -> lib.rs` would probe as
  `RegularFile`, pass the extension filter on the *link* name, and have
  Markdown written over `src/lib.rs` from inside the cap-std sandbox, whose
  root is the file's parent directory rather than the repository. **That write
  is now impossible**: `replace_file` reads `symlink_metadata` and declines a
  symlink with `io::ErrorKind::InvalidInput`, so CON-SAFE-001 is defended at
  the write boundary regardless of what selection does.

  The variant is still required, for a different reason. `--git` selects files
  the user never named, so a repository containing a tracked Markdown symlink
  would emit a per-file error on every run for a file the user did not ask
  about — and REQ-GIT-006 promises such candidates are skipped silently.
  Commit `83e6150` makes it worse rather than better: with `write_back`
  skipping unchanged files, the refusal fires only when the symlink's target
  drifts, so the error is **intermittent**. Excluding symlinks at selection
  time is what makes the behaviour predictable.
- Artefact: `src/select/policy.rs` `mod tests`, plus an end-to-end scenario.

**INV-NOWRITE-UNCHANGED** — **discharged by pull request #464; not this
plan's work.** `driver::write_back` is documented as reachable "only for a file
whose bytes would change", and commit `83e6150` pins two properties for it,
each beside a positive control proving a drifting file is still replaced, so
neither can pass by never writing. Its rationale is now stronger than this plan
anticipated: because replacement renames a temporary over the target, an
unconditional write would move the inode and the modification time of a
byte-identical file, and a downstream staleness check would see a rebuild where
there was nothing to rebuild.

This plan's only obligation here is not to regress it: the `--git` path must
route writes through `write_back` rather than calling `replace_file` directly.
Verify by inspection at the EP-M2 conformance check.

**INV-CONFLICT-GUARD** — when the repository is mid-merge, mid-rebase, or
mid-cherry-pick, a selected file containing all three conflict-marker forms is
excluded from rewriting unless `--allow-conflicted` is given.

- Method: `rstest` over the marker-detection predicate, plus a scenario
  asserting a conflicted file is **byte-identical** after `--git --in-place`.
- Rationale: reflowing across the `<<<<<<< HEAD`, `=======`, and `>>>>>>>`
  markers restructures text on both sides of the boundary. The user then
  resolves against corrupted content and commits it into a rewritten history,
  where `git rebase --abort` is gone. Requiring all three marker forms, each at
  line start with the exact seven-character run, keeps the false-positive rate
  low for documents that discuss conflict markers; gating the scan on repository
  state narrows it further; `--allow-conflicted` is the escape.
- Artefact: `src/select/conflict.rs` `mod tests` and the feature file.
- Non-vacuity: include a document that mentions `<<<<<<< HEAD` inside a fenced
  block but has no `=======` or `>>>>>>>`, and assert it is **not** excluded.

**LEM-SELECT-SETEQ** — the set of inodes `--git` writes equals
`{ inode(p) : p reported by the configured ls-files invocation, lowercased
extension of p is in the configured set, p is a regular file, and the formatted
output of p differs from its contents }`.

- Method: composition of the invariants above, discharged against the real
  interface by the behavioural scenarios in EP-M2.
- Rationale: this connects the unit-level invariants to the user-visible
  promise. Its left-hand side is only observable end to end.
- Non-vacuity: the scenario fixture contains a file in each equivalence class —
  tracked Markdown, untracked Markdown, ignored Markdown, tracked
  non-Markdown, tracked-but-deleted Markdown, and a Markdown symlink — and
  asserts both that the right ones changed and that the rest did not. A
  selection that is too narrow fails the first assertions; one that is too wide
  fails the second.

**Not verified by test: CON-CAP-001.** Whether writes flow through a
`cap_std::Dir` cannot be observed by a black-box `assert_cmd` test. It is
discharged by code review at the EP-M2 conformance check. An earlier draft
traced it to a test name that could not have delivered it.

### Rigour not used, and why

`proptest` is proportionate for every obligation here. Kani is not: the only
plausible candidate is INV-NUL-SPLIT over small byte arrays, which a byte-level
property covers at a fraction of the cost, and `src/fences_properties.rs`
already records the same judgement for an analogous parsing obligation. Verus
is not: LEM-SELECT-SETEQ is a set comprehension over a finite filter with no
recursion, no unbounded arithmetic, and no inductive structure; it is
discharged by the bidirectional properties, whose negative controls demonstrate
they can fail; and Verus needs its own toolchain, conflicting with this
repository's `rust-toolchain.toml` pin of `nightly-2026-03-26`. Revisit if
selection later becomes recursive. Full reasoning belongs in ADR 0010, not
repeated here.

### Mutation testing replaces hand-applied negative controls

Each obligation above names a negative control. Do **not** discharge them by
hand-editing the source, observing a failure, and reverting: that leaves no
diff, no CI signal, and nothing a reviewer can re-run, and under context
pressure it will be recorded rather than performed.

Use `cargo-mutants`, which is a cargo subcommand binary and therefore not a
manifest entry, so CON-DEP-001 is untouched. `make mutants` runs it, and
`.cargo/mutants.toml` — the tool's own default path for the config — holds the
scope and the test selection, so that a bare `cargo mutants` in this checkout
behaves as the target does. Keep the named controls in this document as a
specification of what must die; let the tool prove it.

Three properties of that config are load-bearing rather than incidental:

- `examine_globs = ["src/select/**"]` is the scope, one definition of it rather
  than one on the command line and another in the file. `--file` would be the
  same glob in a second place, free to drift from this one.
- **No `additional_cargo_test_args`**: the whole suite is the test set, exactly
  as `make test` runs it. The setting was there while
  `tests/git_file_selection.rs` was red — the tool checks the unmutated baseline
  first, and a suite that is already red makes every mutant look caught — and
  EP-M2 dropped it once that suite went green. The file records the history
  where the setting used to be, because a narrowed run and a widened one report
  the same number for a mutant no test in either set exercises.
- `TMPDIR` is set by the target, absolutely, to
  `$(HOME)/.cache/mdtablefix/mutants/$(notdir $(CURDIR))`. Absolute, because the
  tool's own child processes run inside the scratch copy of the tree, where a
  relative path does not exist. Outside the worktree, because the children
  inherit `TMPDIR` and a test suite whose temporary directories landed inside
  this repository would fail the `--git` scenarios that assert on being outside
  one. `$HOME/.cache` is neither `/tmp`, which is not a build target on this
  machine, nor inside the tree under test.

Zero survivors is the acceptance criterion. `src/select/policy.rs` and
`src/select/git_ls_files.rs` are the two files it names, and the scope is the
whole selection module, so the criterion is discharged for every file in it —
a survivor anywhere in `src/select/` fails the run, and the run exits non-zero.

## Milestones and plateaus

No compatibility machinery appears anywhere in this plan: the package is
pre-1.0, `--git` is new, CON-API-001 keeps the module tree private to the
binary, and the one existing-interface change (REQ-GIT-005) strictly widens
what is accepted.

### EP-M0 — prototyping spike, reduced scope

Both halves of this milestone have shrunk since the plan was first written.

- Outcome: confirm on the merged `check-option` tree what was measured here in
  isolation, then delete the spike.
- The grammar half is largely done. Adding `git` to the `inputs` group was
  measured against clap 4.6.6 and the results are recorded under
  AX-CLAP-GRAMMAR. What remains is to reproduce them against the **real**
  `Cli`, which carries `mode`, `--check`, and `--diff` as well, and to capture
  clap's verbatim diagnostics for the documentation.
- The BDD half is largely done: pull request #464 ships `rstest-bdd` and
  `rstest-bdd-macros` with passing feature files, so the toolchain question is
  answered. Confirm one scenario in this plan's own feature file runs.
- Acceptance evidence: `EV-M0-GRAMMAR`, a transcript covering `--git a.md`
  (rejected), `--in-place` alone (rejected), `mdtablefix a.md b.md` (**still
  accepted** — the regression case), `--git --in-place`, `--git --check`,
  `--git --diff`, `--git --list-files`, and `--include-untracked` without
  `--git` (rejected by the post-parse check, exit 2), each with its verbatim
  diagnostic; and `EV-M0-BDD`, one passing scenario.
- Go/no-go: if the measured grammar does not reproduce on the real `Cli`,
  escalate rather than choosing a different shape unilaterally — the
  diagnostics are user-visible.
- Recovery: `git checkout -- .`; the spike is additive and discardable.
- Compatibility decision: none required.

### EP-M1 — selection module, private to the binary

- Outcome: `src/select/` implements extension filtering, identity-based
  deduplication, probing, conflict detection, and the `git ls-files` adapter,
  fully tested without a repository except for one real-`git` boundary test.
  The CLI is unchanged.
- Requirements: REQ-GIT-001, REQ-GIT-002, REQ-GIT-006, REQ-GIT-007.
- Acceptance evidence: `EV-M1-SELECT` — `cargo test --bin mdtablefix select`
  passes with every obligation above discharged and observed red first; and
  `EV-M1-MUTANTS` — `make mutants` reports **40 mutants, 34 caught, 6 unviable,
  0 missed**, which is zero survivors across the whole `src/select/**` scope,
  not merely in the two files named above.
- Conformance check: `src/lib.rs` is unmodified (CON-API-001); `policy.rs`
  imports nothing from `std::process`, `std::fs`, or `cap_std`; `probe` and
  `list_candidates` both take a root parameter and no test calls
  `set_current_dir`; every module `//!` states its dependency direction.
- Recovery: revert the commit.
- Remaining gaps: no CLI flag.
- Compatibility decision: none required.

Merging what an earlier draft split into two milestones is deliberate: a
milestone whose outcome is "an additive module the binary does not reference"
is dead code with a checkbox, and two of them in a row is bookkeeping rather
than two plateaus. One milestone that ends with a fully tested, reviewable
selection module is a real, revert-safe state.

### EP-M2 — command-line surface and end-to-end behaviour

- Outcome: `--git`, `--include-untracked`, `--md-exts`, `--list-files`, and
  `--allow-conflicted` exist, appear in `--help`, obey the grammar, and drive
  the existing pipeline. The feature is fully usable.
- Requirements: REQ-GIT-003, REQ-GIT-004, REQ-GIT-005, REQ-GIT-008,
  REQ-GIT-009, REQ-GIT-010, and end-to-end discharge of LEM-SELECT-SETEQ,
  INV-NOWRITE-UNCHANGED, and CON-SAFE-001.
- Additional obligation, from the memory profile: **`Mode::Print` must not
  retain the whole formatted corpus.** Pull request #464 improved this for the
  reporting modes — `driver::analyse` drops each `Assessment` before returning,
  "so retained memory is proportional to the rendered payload rather than to
  twice the whole input" — but under `Mode::Print` the rendered payload *is*
  every file's formatted text, and the `collect()` before printing is still a
  hard barrier. Measured on this repository, 28 Markdown files average 17.7 KB,
  so a 20,000-file documentation monorepo would hold roughly 354 MB: fine on a
  workstation, fatal in a 2 GB container, and previously bounded only by
  `ARG_MAX`. `--git` removes that bound. Iterate `chunks(256)` sequentially,
  `par_iter()` within each chunk, and drain each chunk to a `BufWriter` over a
  single `stdout().lock()` before the next. Peak becomes a constant ~4.5 MB,
  and the `BufWriter` also collapses the present one `write(2)` per output line
  — `Stdout` is `LineWriter`-backed unconditionally, roughly 8.6 million
  syscalls at that scale. Ordering within a chunk comes from
  `driver::in_argument_order`, not from `collect`, because `AX-RAYON-ORDER` is
  withdrawn.

  Note this is a pre-existing defect that `--git` makes reachable, not one
  `--git` introduces. If it proves larger than it looks, it is separable: raise
  it as its own issue rather than growing this plan.
- Acceptance evidence: `EV-M2-CLI` — `cargo test --test cli_git --test
  git_file_selection` passes, the `--help` snapshot is accepted, and the
  transcripts under "Validation and acceptance" reproduce.
- Conformance check: `src/main.rs` under 400 lines; `open_file_parent` remains
  the only ambient content boundary (**by review**, per the note above); no
  dependency added beyond the five named; every requirement discharged with
  named evidence.
- Recovery: revert the commit; EP-M1's plateau is intact.
- Remaining gaps: documentation.
- Compatibility decision: none required.

### EP-M3 — documentation and decision record

- Outcome: `README.md`, `docs/users-guide.md`, `docs/architecture.md`,
  `docs/developers-guide.md`, `docs/contents.md`, and
  `docs/adrs/0010-git-file-selection.md` describe the feature, the boundary,
  the CRLF caveat, and the rejected alternatives.
- Acceptance evidence: `EV-M3-DOCS` — `make markdownlint` and, if a Mermaid
  diagram was added, `make nixie` both pass.
- Conformance check: every Decision log entry appears in ADR 0010 or a
  component document; `docs/contents.md` indexes the new ADR.
- Recovery: documentation-only.
- Remaining gaps: none. Set Status to COMPLETE only after reconciling.
- Compatibility decision: none required.

## Interfaces and dependencies

`src/main.rs` declares `mod select;`, which resolves to `src/select.rs` and
`src/select/`. `src/lib.rs` does **not** declare it, so the tree compiles only
into the binary and adds no public API (CON-API-001). Each module's `//!`
header must say so, and must state its dependency direction. Unit and property
tests live in `#[cfg(test)] mod tests` within each module, following the
precedent of the existing `#[cfg(test)] mod tests` in `src/main.rs`.

In `src/select/extensions.rs`:

```rust
/// A case-insensitive set of file extensions, stored without dots.
///
/// Named for what it does rather than for Markdown: `--md-exts` accepts any
/// extension, so a `MarkdownExtensions` type would be a false promise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFilter(std::collections::BTreeSet<String>);

impl Default for ExtensionFilter {
    /// Returns `md`, `mdc`, and `markdown`.
    fn default() -> Self;
}

impl ExtensionFilter {
    /// Reports whether `path` ends in one of these extensions.
    #[must_use]
    pub fn matches(&self, path: &camino::Utf8Path) -> bool;

    /// Iterates the extensions shortest-first, then byte-wise, without dots.
    ///
    /// Shortest-first is what renders the default as `md, mdc, markdown`; a
    /// byte-wise sort alone would render it `markdown, md, mdc`. The order is
    /// total, so a rendering does not depend on the order the user gave.
    pub fn iter(&self) -> impl Iterator<Item = &str>;
}

/// Builds a filter from values `parse_extension` has already accepted.
///
/// The values are folded again, so collecting `["md", "MD"]` yields one
/// extension, and a caller need not keep a `Vec` on the way to a set.
impl FromIterator<String> for ExtensionFilter {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self;
}

/// Renders as `md, mdc, markdown`, for `--help` and diagnostics.
impl std::fmt::Display for ExtensionFilter { /* ... */ }

/// Parses one extension, for use as a clap `value_parser`.
///
/// Strips one optional leading dot, trims surrounding whitespace, and folds
/// ASCII case.
///
/// # Errors
///
/// Returns [`ExtensionSpecError`] for an empty or dot-only value, or one
/// containing a path separator or a NUL byte.
pub fn parse_extension(value: &str) -> Result<String, ExtensionSpecError>;

/// The reason an extension value was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ExtensionSpecError {
    #[error("extension is empty")]
    Empty,
    #[error("extension {value:?} is only a dot")]
    DotOnly { value: String },
    #[error("extension {value:?} contains {kind}")]
    InvalidCharacter { value: String, kind: InvalidCharacterKind },
}

/// Why a value is not usable as an extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InvalidCharacterKind {
    /// A `/` or `\`, which would make one extension name two path components.
    #[error("a path separator")]
    PathSeparator,
    /// A NUL byte, which cannot survive an `OsStr` round trip on all platforms.
    #[error("a NUL byte")]
    Nul,
}
```

Note what changed and why. The `EmptySegment { position: usize }` variant of an
earlier draft is gone: `position` had no stated units — for `"md,,markdown"` it
could defensibly be the byte offset `3`, the segment index `1`, or the ordinal
`2` — and an unspecified public field is exactly how an off-by-one becomes
permanent. Per-value parsing removes the question entirely. `InvalidSegment`
likewise discarded *why* the value was rejected and interpolated unbounded user
input into one vague message; `InvalidCharacterKind` (itself
`#[non_exhaustive]`) names the reason. Both enums are `#[non_exhaustive]`
because error enums grow and a struct variant cannot gain a field additively.

In `src/select/policy.rs`:

```rust
/// Identifies a file independently of the path used to reach it.
///
/// The canonicalized path, not `(st_dev, st_ino)`. Replacement writes a new
/// inode over the target, so an identity keyed on the inode would collapse two
/// hard links, format one, and leave the other stale. See INV-DEDUP.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileIdentity(camino::Utf8PathBuf);

impl FileIdentity {
    /// Wraps a canonicalized path, as the probe obtains it.
    #[must_use]
    pub fn from_canonical_path(path: camino::Utf8PathBuf) -> Self;

    /// The canonicalized path this identity was built from.
    #[must_use]
    pub fn as_path(&self) -> &camino::Utf8Path;
}

/// What a candidate path turned out to be in the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathKind {
    /// A regular file that may be read and rewritten.
    RegularFile(FileIdentity),
    /// Absent from the working tree, for example a staged deletion.
    Missing,
    /// A symbolic link. Never followed: the link's extension says nothing
    /// about the target's type, and writing through it escapes the selection.
    Symlink,
    /// Present but neither a regular file nor a symlink, for example a
    /// submodule gitlink.
    Other,
}

/// Reports what a candidate path actually is. Implemented by adapters.
pub trait PathProbe {
    fn probe(&self, root: &camino::Utf8Path, path: &camino::Utf8Path) -> PathKind;
}

/// Narrows candidates to the sorted, alias-free set of files that exist as
/// regular files and carry a configured extension.
#[must_use]
pub fn select_files<P>(
    candidates: &[camino::Utf8PathBuf],
    root: &camino::Utf8Path,
    extensions: &ExtensionFilter,
    probe: &P,
) -> Vec<camino::Utf8PathBuf>
where
    P: PathProbe + ?Sized;
```

`select_files` filters by extension first, so the filesystem is touched once
per distinct Markdown candidate and never for a `.rs` file. Keep that ordering
and comment it: reversing it multiplies the probe count by roughly fifteen on a
typical repository (28 Markdown files out of 416 paths here). It then probes,
keeps `RegularFile`, deduplicates on `FileIdentity` retaining the
lexicographically first path, and sorts.

In `src/select/git_ls_files.rs`:

```rust
/// Lists candidates by running `git ls-files`.
#[derive(Debug, Clone)]
pub struct GitLsFiles {
    program: std::ffi::OsString,
    include_untracked: bool,
}

impl GitLsFiles {
    #[must_use]
    pub fn new(include_untracked: bool) -> Self;

    /// Uses a specific program. The seam that lets tests drive the failure
    /// paths without a real Git installation, and that makes failure-message
    /// snapshots a function of our code rather than of the machine's `git`.
    #[must_use]
    pub fn with_program(program: impl Into<std::ffi::OsString>, include_untracked: bool) -> Self;

    /// # Errors
    ///
    /// See [`GitListError`].
    pub fn list_candidates(&self, dir: &camino::Utf8Path)
        -> Result<CandidateListing, GitListError>;
}

/// Candidate paths, with a count of paths that were not valid UTF-8.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CandidateListing {
    pub paths: Vec<camino::Utf8PathBuf>,
    pub skipped_non_utf8: usize,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GitListError {
    #[error("`{program}` is not installed or not on PATH")]
    ProgramNotFound { program: String },
    #[error("running `{program} ls-files`")]
    Spawn { program: String, #[source] source: std::io::Error },
    #[error("`git ls-files` failed with exit status {status}")]
    Failed { status: std::process::ExitStatus, stderr: String },
}

/// Splits a NUL-terminated byte stream, counting entries that are not UTF-8.
pub(crate) fn split_nul_delimited(bytes: &[u8]) -> CandidateListing;
```

A concrete `GitListError` rather than `anyhow::Result` is required by
`AGENTS.md:264` and is what lets the composition root distinguish "install git"
from "here is git's complaint" without `downcast_ref`. It also puts the wording
in one `Display` impl we own and can snapshot without a subprocess.

The adapter runs exactly one of:

```plaintext
git ls-files -z --deduplicate --cached
git ls-files -z --deduplicate --cached --others --exclude-standard
```

with `Command::current_dir(dir)`. It does not pass `--full-name`, so `--git` is
subtree-scoped per fact 3. It **does** pass `--deduplicate`, because refusing a
correct upstream flag so that a domain invariant has something to test would be
the test wagging the design; INV-DEDUP survives because it is now a claim about
inodes that `--deduplicate` cannot make.

Use `Command::output()`, not `spawn()` plus a manual read. `output()` drains
both pipes concurrently and cannot deadlock. A hand-rolled "streaming" variant
deadlocks once git's output exceeds the 64 KiB pipe capacity — about 1,700
paths at the 39.2 bytes per path measured here — which is small enough that
this repository's 416 paths would pass every test while a user's repository
hung forever. The whole-output buffer is ~3 MB even for the Linux kernel's
80,000 paths, so there is nothing to gain.

In `src/select/fs_probe.rs`:

```rust
/// Probes the real working tree using `std::fs::symlink_metadata`.
///
/// `symlink_metadata`, not `metadata`: see `PathKind::Symlink`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AmbientPathProbe;

impl PathProbe for AmbientPathProbe { /* ... */ }
```

In `src/select/conflict.rs`:

```rust
/// Reports whether the repository is mid-merge, mid-rebase, or
/// mid-cherry-pick, by testing for `MERGE_HEAD`, `rebase-merge`,
/// `rebase-apply`, and `CHERRY_PICK_HEAD` under the Git directory.
pub fn operation_in_progress(git_dir: &camino::Utf8Path) -> bool;

/// Reports whether `content` carries all three conflict-marker forms, each at
/// the start of a line with an exact seven-character run.
#[must_use]
pub fn has_conflict_markers(content: &str) -> bool;
```

### Command-line surface

**Prerequisite: split `src/main.rs` first.** On `check-option` it is 386 lines
against a 400-line cap. Extract the `Cli` and `FormatOpts` declarations into a
binary-private `src/cli.rs`, declared from `main.rs`, as a separate
behaviour-free commit before adding anything. That returns `main.rs` to roughly
300 lines. Do not put `--git` wiring in `src/driver.rs`, which is 369 lines.
As delivered, the base's `src/command.rs` is that module and this branch adds
no `src/cli.rs` — see `EX-REBASE-STRUCTURE`.

`Cli` gains `git` as a member of the existing `inputs` group, and four
supporting flags. The `mode` group and `--in-place`, `--check`, and `--diff`
are already as pull request #464 leaves them and need no change:

```rust
#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
#[command(group(
    clap::ArgGroup::new("inputs").args(["files", "git"]).multiple(false)
))]
#[command(group(clap::ArgGroup::new("mode").multiple(false).requires("inputs")))]
pub struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", group = "mode")]
    in_place: bool,
    /// Report which files would be reformatted, and by how many lines
    #[arg(long = "check", group = "mode")]
    check: bool,
    /// Print a unified diff for each file that would be reformatted
    #[arg(long = "diff", group = "mode")]
    diff: bool,
    /// Print the selected paths and exit, without reading or writing them
    #[arg(long = "list-files", group = "mode")]
    list_files: bool,
    /// Select Markdown files tracked by Git beneath the current directory
    #[arg(long = "git")]
    git: bool,
    /// Also select untracked files that Git does not ignore
    #[arg(long = "include-untracked")]
    include_untracked: bool,
    /// File extensions to select under `--git`
    #[arg(
        long = "md-exts",
        value_name = "EXT",
        value_delimiter = ',',
        default_value = "md,mdc,markdown",
        value_parser = select::extensions::parse_extension,
    )]
    md_exts: Vec<String>,
    /// Rewrite files containing conflict markers during a merge or rebase
    #[arg(long = "allow-conflicted")]
    allow_conflicted: bool,
    #[command(flatten)]
    pub opts: FormatOpts,
    /// Markdown files to fix
    pub files: Vec<PathBuf>,
}
```

Two details differ from this plan's first draft and are recorded in Surprises &
discoveries: the default is one comma-separated `default_value` rather than
three `default_values`, because `clap` renders the latter space-joined; and
`Cli` and the two fields the composition root reads are `pub`, because the
binary's modules are siblings rather than nested.

`--list-files` joins the `mode` group rather than standing beside it, because
it is a fifth thing to do with a selection, not a modifier. That also gets the
mutual exclusion with `--check`, `--diff`, and `--in-place` for free, and makes
`--list-files` require the `inputs` group like every other mode.

Adding `git` to `inputs` was verified against clap 4.6.6 before being specified
here. `mdtablefix a.md b.md` and `--in-place a.md b.md` still parse — a
multi-value positional inside an exclusive group is safe — while `--git a.md`
is rejected, and `--git --check`, `--git --diff`, and `--git --in-place` all
parse.

**`requires = "git"` does not work and must not be used** for
`--include-untracked`, `--md-exts`, or `--allow-conflicted`. Measured on clap
4.6.6, with `git` in the `inputs` group and `files` a `Vec` positional:
`--list-files` alone is correctly rejected, but `--list-files a.md` is
**accepted**, silently running a `--git`-only flag with no `--git`. The same
holds under `conflicts_with = "files"`, so group membership is not the cause;
a bool flag's `requires` on another bool flag is not dependable once a
positional is present, and `--md-exts` is a second instance of the same class
because its default means it always carries a value. Use an explicit post-parse
check emitting a real clap error, so the exit status stays 2 and the usage
footer survives:

```rust
impl Cli {
    /// Parses and enforces the dependencies `clap` cannot express here.
    fn parse_validated() -> Self {
        let cli = Self::parse();
        for (name, present) in [
            ("--include-untracked", cli.include_untracked),
            ("--allow-conflicted", cli.allow_conflicted),
            ("--list-files", cli.list_files),
        ] {
            if present && !cli.git {
                Self::command()
                    .error(
                        clap::error::ErrorKind::MissingRequiredArgument,
                        format!("{name} requires --git"),
                    )
                    .exit();
            }
        }
        cli
    }
}
```

`--md-exts` needs the same treatment keyed on
`ArgMatches::value_source(..) != Some(ValueSource::DefaultValue)` rather than
on a bool, because `default_values` means it always carries a value.

**`--list-files` needs it too**, which EP-M0 established by measurement and an
earlier revision of this section got wrong. The claim was that `mode` already
requires `inputs` and `--git` is the only way to satisfy that without positional
files — but a positional argument *is* a way to satisfy `inputs`, so
`mdtablefix --list-files notes.md` parses and runs. Listing a selection the user
has just typed out by hand has nothing to resolve, so requiring `--git` is also
the honest contract, and the feature file's "Reject `--list-files` without
`--git`" scenario demands it. See Surprises & discoveries.

### Composition: `--git` as a second `Inputs` source

Pull request #464 already resolved the shape this plan previously had to
invent. `driver::Inputs` distinguishes `Stdin` from `Files(Vec<Utf8PathBuf>)`,
and `main` matches on it. `--git` adds a second way to produce `Files`:

```rust
/// Resolves `--git` into the paths to act on, and the guard that governs them.
///
/// `inputs` is `Inputs::Files`, which may be empty: an empty selection is
/// success and must not fall through to standard input. Never `Stdin`.
pub fn resolve(
    cli: &Cli,
    mode: Mode,
    working_directory: &Utf8Path,
) -> Result<GitSelection, GitListError>;

pub struct GitSelection {
    pub inputs: Inputs,
    pub guard: ConflictGuard,
}
```

Two shapes changed from this plan's earlier draft, both in EP-M2 and both
recorded in the Decision log: the function takes `mode` (the guard is resolved
only where it can refuse) and returns the guard beside the inputs rather than
the inputs alone. It lives in `src/git_inputs.rs`, a binary-private module
beside `src/driver.rs`, so that `src/main.rs` stays under its size cap and the
composition is testable without a repository.

Three properties of the surrounding contract are inherited rather than
restated, and must not be re-implemented:

- **Exit status** comes from `driver::exit_status`. A `--git` failure — `git`
  absent, or the directory is not a repository — is an operational failure, so
  `ExitStatus::Error`, exit code **2**. An earlier draft of this plan
  documented exit 1; that was written before the contract existed.
- **Ordering** comes from `driver::in_argument_order`. Selection assigns each
  path an index from its sorted order and lets the driver re-establish it.
  `AX-RAYON-ORDER` remains withdrawn.
- **Non-UTF-8 paths** must be dropped during selection, counted, and reported
  once on stderr, so that no such path ever reaches `Inputs::resolve` — which
  now fails the whole run on one. This keeps a single stray filename elsewhere
  in the tree from aborting a whole-repository operation, without disturbing
  the positional-argument contract.

`--list-files` reads no content: `driver::analyse` answers it in its first
statement, before the read capability is cloned, so the mode holds a directory
capability it never uses rather than being denied one by type. The Decision log
records why the early return is the guarantee and what it leaves to review.

### Dependencies

One runtime dependency, which neither `main` nor `check-option` has:

```toml
thiserror = "2"
```

`AGENTS.md:264` mandates it for domain error enums and `AGENTS.md:268` forbids
the alternative of exporting an opaque error type.

**No development dependency is this plan's to add.** Pull request #464 already
adds `googletest = "0.14"`, `pretty_assertions = "1"`, `rstest-bdd = "0.5.0"`,
and `rstest-bdd-macros = { version = "0.5.0", features =
["strict-compile-time-validation"] }`. Match those exactly, including the
explicit macros crate — `rstest-bdd` 0.5.0 does not re-export its macros — and
the feature. Pull request #464 also adds
`cargo test --doc --all-features` to the `test` target, so the doctest gap this
plan recorded is closed.

Assertion style, per the `rust-unit-testing` skill: use
`pretty_assertions::assert_eq` for structural comparisons of path vectors, and
`googletest` matchers where a matcher reads better than an equality, e.g.
`expect_that!(selected, unordered_elements_are![...])`. Put
`#[googletest::gtest]` **before** `#[rstest]`.

### Test environment hardening

Every test that builds a real repository must neutralize ambient Git
configuration, or the developer's own `core.excludesFile` leaks into the
AX-GIT-LSFILES boundary test — which is precisely the ambient input
CON-SAFE-001 exists to control. Set, on the child process:

```plaintext
GIT_CONFIG_NOSYSTEM=1
GIT_CONFIG_GLOBAL=/dev/null
HOME=<the temporary directory>
LC_ALL=C
LANGUAGE=
GIT_AUTHOR_NAME=<fixture name>
GIT_AUTHOR_EMAIL=<fixture address>
GIT_COMMITTER_NAME=<fixture name>
GIT_COMMITTER_EMAIL=<fixture address>
```

`LC_ALL` and `LANGUAGE` are belt and braces only. Per AX-GIT-NLS, no assertion
may depend on git's message text regardless.

The identity variables are not belt and braces. `GIT_CONFIG_GLOBAL=/dev/null`
removes `user.name` and `user.email` along with the rest of the developer's
configuration, so `git commit` fails outright in the fixture — and the fixture
cannot fix that by writing a configuration file, because a file inside the
repository is exactly what the selection tests read. Supplying the identity
through the environment keeps the repository's contents a function of the
scenario alone. Found at Stage B, where the first commit in the fixture failed
with git's "please tell me who you are"; the hardening list above previously
omitted it.

## Behavioural specification

Create `tests/features/git_file_selection.feature` and bind it from
`tests/git_file_selection.rs`. Keep the two synchronized with this plan.

```gherkin
Feature: Select files from a Git repository

  As a maintainer of a Markdown-heavy repository
  I want mdtablefix to act on the repository's own Markdown files
  So that I need not enumerate them by hand or risk touching ignored files

  Background:
    Given a Git repository containing a committed file "docs/guide.md" with a broken table
    And a committed file "src/lib.rs" with a broken table
    And an untracked file "notes.md" with a broken table
    And an ignored file "build/out.md" with a broken table

  Scenario: Reformat tracked Markdown in place and nothing else
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And the file "docs/guide.md" has a reflowed table
    And the file "notes.md" is unchanged
    And the file "build/out.md" is unchanged
    And the file "src/lib.rs" is unchanged

  Scenario: Extend the selection to untracked files on request
    When I run mdtablefix with "--git --include-untracked --in-place"
    Then the command succeeds
    And the file "docs/guide.md" has a reflowed table
    And the file "notes.md" has a reflowed table
    And the file "build/out.md" is unchanged

  Scenario: List the selection without acting
    When I run mdtablefix with "--git --list-files"
    Then the command succeeds
    And stdout is exactly "docs/guide.md"
    And the file "docs/guide.md" is unchanged

  Scenario: Never write through a symlink
    Given a committed symlink "docs/alias.md" pointing at "../src/lib.rs"
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And the file "src/lib.rs" is unchanged
    And the file "docs/alias.md" is still a symlink

  Scenario: Restrict the selection to chosen extensions
    Given a committed file "rules.mdc" with a broken table
    When I run mdtablefix with "--git --in-place --md-exts mdc"
    Then the command succeeds
    And the file "rules.mdc" has a reflowed table
    And the file "docs/guide.md" is unchanged

  Scenario: Accept extensions written with a leading dot
    When I run mdtablefix with "--git --in-place --md-exts .md"
    Then the command succeeds
    And the file "docs/guide.md" has a reflowed table

  Scenario: Skip a tracked file deleted from the working tree
    Given the file "docs/guide.md" is deleted from the working tree
    And a committed file "other.md" with a broken table
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And the file "other.md" has a reflowed table

  Scenario: Refuse to rewrite a conflicted file mid-merge
    Given an unresolved merge conflict in the tracked file "docs/guide.md"
    When I run mdtablefix with "--git --in-place"
    Then the command fails
    And the file "docs/guide.md" is unchanged
    And stderr contains "conflict markers"

  Scenario: Rewrite a conflicted file when explicitly allowed
    Given an unresolved merge conflict in the tracked file "docs/guide.md"
    When I run mdtablefix with "--git --in-place --allow-conflicted"
    Then the command succeeds

  Scenario: Report drift across the repository without changing it
    When I run mdtablefix with "--git --check"
    Then the exit status is 1
    And stdout names "docs/guide.md"
    And the file "docs/guide.md" is unchanged

  Scenario: Report a clean repository
    Given every tracked Markdown file is already formatted
    When I run mdtablefix with "--git --check"
    Then the exit status is 0

  Scenario: Exit successfully when nothing is selected
    Given a Git repository containing only the committed file "src/lib.rs"
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And stdout is empty
    And standard input was not read

  Scenario: Scope the selection to the current directory
    When I run mdtablefix from "docs" with "--git --list-files"
    Then the command succeeds
    And stdout is exactly "guide.md"

  Scenario: Report a clear error outside a Git repository
    Given the working directory is not inside a Git repository
    When I run mdtablefix with "--git"
    Then the command fails
    And stderr contains "git ls-files"

  Scenario: Reject an unusable extension
    When I run mdtablefix with "--git --md-exts md,,markdown"
    Then the command exits with status 2
    And stdout is empty
    And stderr contains "extension is empty"

  Scenario: Reject combining --git with explicit file arguments
    When I run mdtablefix with "--git notes.md"
    Then the command exits with status 2
    And stdout is empty

  Scenario: Reject --list-files without --git
    When I run mdtablefix with "--list-files notes.md"
    Then the command exits with status 2
    And stdout is empty
    And stderr contains "--list-files requires --git"
```

Assertion choices are deliberate. `stderr contains "git ls-files"` names
**our** wrapper's wording, never git's, per AX-GIT-NLS. The `--git notes.md`
rejection asserts on the **exit status** rather than on message text, because
that wording is clap's: an earlier draft asserted `stderr contains "requires"`
and clap 4.6.6 renders `the following required arguments were not provided:`
for that error kind, so the scenario would have failed on day one. The other
two rejections do assert message text, because it is **ours** — `ExtensionSpec`
renders `extension is empty`, and `--list-files` is rejected by this plan's own
post-parse check, which emits `--list-files requires --git` verbatim. The
`stdout is empty` line in all three denies a naive implementation the escape of
treating the flags as inert and formatting the positional as if `--git` were
absent.

`--list-files` gives the selection an honest oracle; an earlier draft asserted
dedup by grepping a conflict marker out of a concatenated content dump, which
was a symptom of the missing affordance.

The two message assertions were added at Stage B. Without them the three
rejection scenarios passed **before any production code existed**, because an
unknown `--git` is itself an exit-2 clap error: red for the wrong reason, which
is the same as green for the wrong reason. Stage B measured
`1 passed; 16 failed` with the strengthened text, leaving only the status-only
scenario the plan deliberately keeps — see Surprises & discoveries.

Step definitions live in `tests/steps/git_selection.rs` and share state through
an `rstest` fixture holding an `rstest_bdd::Slot` per fact, exactly as
`tests/steps/reporting.rs` does — the same shape the `--check` work already
established, so the two specifications cannot drift in house style. No globals
and no `static`: they would make the scenarios order-dependent. A `Slot` is
read with `get`, which requires `Clone`, so the `TempDir` slots are filled
through `get_or_insert_with` instead; the directory is therefore created by the
first `Given` that needs it rather than by a step of its own.

`standard input was not read` is asserted by **writing a document to the
child's standard input** and requiring standard output to be empty. The plan
previously specified "a closed or empty stdin and a timeout", and the
replacement is strictly stronger: an empty stdin is satisfied by a tool that
reads and discards, whereas a document that would have been printed had it been
read makes emptiness evidence. It is also deterministic — the pipe is closed
after the write, so a read returns the document rather than blocking, and no
timeout is needed. Every step writes `RAGGED` to stdin for this reason, not only
the one that asserts on it.

The fixture builds a real repository with a real `git`: real commits, a real
ignore file, and for the conflict scenarios a real `git merge` that is allowed
to fail. The conflict is not staged by writing marker text into a file — git
writes the markers, and the step asserts that all three marker forms landed at
the start of a line before letting the scenario proceed. The `--git`-absent
scenario likewise runs in a genuine temporary directory that is not a
repository, rather than in a repository whose `.git` was hidden.

The symlink scenario and its step are gated on `#[cfg(unix)]`. On Windows git
checks a symlink out as a plain file holding the target's path, so the scenario
would have no subject there; both the step definition and the `#[scenario]`
binding carry the gate, so the step registry and the feature file stay
consistent.

## Plan of work

### Stage A — understand and propose, no code changes

Read the documents and load the skills. Re-run the reference-command
transcripts under "Measured behaviour" against the `git` on this machine. If
any differs, stop: the axioms have changed and the plan needs revising.

### Stage B — red tests and the behavioural specification

Add the one runtime and four development dependencies. Write the feature file
and its bindings, then the failing unit and property tests for EP-M1, before
any production code. Confirm each fails for the intended reason — for the
property tests, a compilation failure naming the missing item, not a spurious
pass. Do not park a red test with `#[ignore]`.

### Stage C — implementation

Build EP-M1, then EP-M2, each in its own commit, each turning its own red tests
green with the smallest change that does so. Run `make mutants` at the end of
EP-M1 and record the survivor count.

Validation: at each milestone boundary, `make check-fmt`, `make typecheck`,
`make lint`, and `make test` all pass, run **sequentially**, never in parallel.

Note that `make test` runs `cargo test --all-targets`, which **does not run
doctests**. Add `cargo test --doc` to the `test` target as part of EP-M1, or
any `# Examples` block written here is never compiled by any gate.

### Stage D — documentation and wider validation

Deliver EP-M3. Re-read `src/select/` against the refactoring heuristics in
`AGENTS.md`, splitting any file approaching the 400-line cap as a separate
commit after the functional one.

Validation: the full gate set plus `make markdownlint`, and `make nixie` if a
Mermaid diagram was added.

## Concrete steps

Run everything from the repository root. Per `AGENTS.md`, capture long output
through `tee` and read the log afterwards.

Confirm the starting state:

```console
$ git branch --show-current
git-option
$ git status --short
```

Re-establish the axioms:

```console
$ git ls-files -z --deduplicate --cached | tr '\0' '\n' | head -3
.github/dependabot.yml
.github/workflows/ci.yml
.gitignore
```

Run a gate and keep the log:

```console
make test 2>&1 | tee "/tmp/test-mdtablefix-$(git branch --show-current).out"
```

Expected at the tail: `test result: ok.` for every test binary and no
`warning:` lines, because `make test` sets `RUSTFLAGS="-D warnings"`.

Focused suites during Stage C:

```console
cargo test --bin mdtablefix select 2>&1 \
  | tee "/tmp/unit-select-$(git branch --show-current).out"
cargo test --test cli_git --test git_file_selection 2>&1 \
  | tee "/tmp/integration-git-$(git branch --show-current).out"
make mutants 2>&1 | tee "/tmp/mutants-$(git branch --show-current).out"
```

Review a `--help` snapshot deliberately:

```console
cargo insta review
```

Prefer delegating full gate runs to the `scrutineer` subagent, which runs them
sequentially, logs each under `/tmp`, and returns a bounded report. When it
reports a failure, read the log it cites rather than re-running the gate.

## Validation and acceptance

Build a scratch repository and observe the tool.

```console
$ cd "$(mktemp -d)" && git init -q . && git config user.email t@t \
    && git config user.name t
$ mkdir -p docs build
$ printf 'build/\n' > .gitignore
$ printf '| A | B |    |\n| 1 | 2 |  | 3 | 4 |\n' > docs/guide.md
$ cp docs/guide.md build/out.md
$ cp docs/guide.md notes.md
$ cp docs/guide.md keep.rs
$ git add docs/guide.md keep.rs .gitignore && git commit -qm init
$ mdtablefix --git --list-files
docs/guide.md
$ mdtablefix --git --in-place
$ cat docs/guide.md
| A | B |   |
| 1 | 2 |   |
| 3 | 4 |   |
```

Expected: `--list-files` names one path and changes nothing; `--in-place`
reflows `docs/guide.md` and leaves `notes.md`, `build/out.md`, and `keep.rs`
byte-identical; exit status 0.

```console
$ mdtablefix --git --include-untracked --list-files
docs/guide.md
notes.md
```

Expected: the untracked file joins the selection; the ignored one does not.

```console
$ cd / && mdtablefix --git ; echo "exit=$?"
mdtablefix: `git ls-files` failed with exit status: 128: fatal: not a git
repository (or any of the parent directories): .git
exit=2
```

Shown wrapped to this document's margin; the tool prints one line. Expected:
exit **2** and a diagnostic naming the cause. Five things about this transcript
are load-bearing, and four of them correct an earlier draft:

- The status is 2, not 1. Pull request #464 establishes
  `ExitStatus { Success, Drift, Error }`, and a `--git` failure is an
  operational error, which is 2. Exit 1 is reserved for a reporting mode that
  found drift.
- `fn main` now returns `ExitCode`, not `anyhow::Result<()>`, so nothing goes
  through `Termination`. The `Error:` / `Caused by:` rendering a previous draft
  showed cannot occur; the message is whatever the binary prints deliberately.
- The wording before the first colon is this repository's and is composed on
  purpose: `` `git ls-files` failed with {status}``. `ExitStatus`'s own
  `Display` already renders the status as `exit status: 128`, so the message
  does not spell those words a second time — an earlier draft's
  `"{command} failed with exit status {status}"` did, and said "exit status"
  twice. `GitListError::diagnostic` appends git's scrubbed text beside it;
  `Display` alone ends at the status, which is the part a test asserts.
- The text after that colon comes from `git`, is version- and locale-dependent
  (AX-GIT-NLS), and is therefore asserted by **no** test. Snapshots of failure
  messages are driven through `GitLsFiles::with_program` against a fixture
  program emitting fixed bytes, so they are a function of this repository's
  code rather than of the machine's `git`.
- It is one line, at most 1024 characters, with control characters and newlines
  in git's text replaced: a path in the repository cannot forge a second line
  of this tool's stderr, nor drive the terminal reading it. See
  `src/select/git_ls_files.rs`'s `relayable`, and the tests that pin both
  halves.

```console
$ mdtablefix --git notes.md ; echo "exit=$?"
error: the argument '--git' cannot be used with '[FILES]...'
exit=2
```

Expected: clap rejects the combination before any file is touched. This wording
is clap's, so the test asserts the exit status; EP-M0 records the verbatim text
for the documentation.

```console
$ mdtablefix --md-exts md ; echo "exit=$?"
error: --md-exts requires --git
exit=2

$ mdtablefix --list-files ; echo "exit=$?"
error: the following required arguments were not provided:
  <FILES|--git>
exit=2
```

Measured. Expected: the first is the dependency `clap` cannot express, enforced
after parsing and reported as a clap error, so the status is 2 and the usage
footer is clap's. `--list-files notes.md` is the case that forced the mechanism:
on clap 4.6.6 it satisfies `requires = "git"` through the positional, with no
`--git` in sight. The second is the `mode` group's own `requires("inputs")`,
which is why `--git` is a way to satisfy `--in-place` rather than a way to
bypass it.

```console
$ mdtablefix --git --check ; echo "exit=$?"
docs/guide.md +3 -2
1 file would be reformatted.
exit=1
```

Measured on the fixture above, before the `--in-place` run. Expected: exit
**1**, drift rather than error, and the summary on standard error while the
finding stays on standard output. This is the combination `--git` exists to
enable in continuous integration, and it works because `git` joined the
`inputs` group that `mode` requires. The sibling `--diff` renders the same
finding as a unified diff with the same exit status, and `--list-files` over
the same tree exits **0**: it reports paths rather than drift, so a tree full
of drift it was never asked to assess is not a failure. That last asymmetry is
`Mode::reports`, and `driver_tests` pins it.

Red-Green-Refactor evidence to record in Progress:

- Red: `cargo test --bin mdtablefix select` fails with `cannot find function
  \`select_files\`` before EP-M1's production code exists.
- Green: the same command reports `test result: ok.` after the minimal
  implementation.
- Refactor: still `ok.` after cleanup, followed by a clean run of the gates.

Quality criteria — what "done" means:

- Tests: `make test` passes with no warnings; `cargo test --bin mdtablefix
  select`, `--test cli_git`, and `--test git_file_selection` all pass;
  `cargo test --doc` passes.
- Verification: every obligation in the Verification plan is discharged by its
  named artefact, observed red first, and `make mutants` reports zero survivors
  in `src/select/policy.rs` and `src/select/git_ls_files.rs`.
  LEM-SELECT-SETEQ is discharged end to end by the scenarios.
- Lint and typecheck: `make check-fmt`, `make typecheck`, `make lint`, run
  sequentially.
- Documentation: `make markdownlint`; `make nixie` if a diagram was added.
- Performance: no threshold, but two shapes are required rather than optional —
  the chunked stdout drain in EP-M2, and extension-filtering before probing in
  `select_files`. Note for anyone measuring later that the dominant fixed cost
  of `--include-untracked` is git's own working-tree walk, not our spawn or our
  stats.
- Security: the tool now spawns a subprocess. It passes a fixed argument vector
  with no shell interpretation and no user-controlled arguments, so there is no
  injection surface on the **input** side. On the **output** side, git's stderr
  is untrusted, unbounded, and may contain paths and ANSI escapes, so cap it
  and strip control characters before printing. State both halves in ADR 0010.

## Idempotence and recovery

Every step is re-runnable. `mdtablefix --git --in-place` is idempotent on its
own output, which `driver::write_back` strengthens into "a second run performs
no writes at all".

That rests on the formatter being a fixed point, which is a property this plan
consumes rather than establishes. Issue #474 — the `--headings` case — was
**fixed by pull request #477**, merged at `408c76a`, and
`tests/idempotence_drift.rs` now gates the whole repository corpus for the
`make fmt` flag set and for that set plus `--headings`.

One measured exception remains, outside those gated sets: `--code-emphasis`
settles on the second pass rather than the first. See Risks. It converges, so
`--git --in-place` is still safe to re-run; what it costs is the claim that one
pass is enough, which the users' guide must not make for that flag.

The gates are read-only apart from build artefacts. `cargo insta reject` undoes
a snapshot review.

The destructive operation is rewriting files in place. The acceptance
transcripts use a throwaway repository under `mktemp -d`. Never run
`mdtablefix --git --in-place` against this repository while validating; the
resulting diff would be indistinguishable from intended work. That warning
applies to **every user with a dirty working tree**, not only to the
implementor — once a run has interleaved its changes with uncommitted work,
`git restore .` reverts both. Two mitigations are in scope and should both
ship: `--list-files` lets a user see the selection first, and the stderr
summary of changed paths makes `git restore -- <paths>` selective afterwards.
Say so in the users' guide.

Each milestone is a single commit, so `git revert` returns to the previous
plateau.

## Progress

- [x] (2026-09-11) Rebase onto `main` at `d0549d9`; study the three merged
      dependencies and the `check-option` interfaces.
- [x] (2026-09-11) Confirm the sequencing decision: this plan follows #464.
- [x] (2026-09-12) Rebase onto `origin/check-option` at `9bcb95d`, on the
      requester's instruction, so the interfaces this plan consumes are present.
      PR #466 moves with the branch and is stacked on #464. Backup tag
      `backup/git-option-pre-rebase` preserves the pre-rebase tip.
- [x] (2026-09-12) Stage A: reference-command transcripts re-established on
      Git 2.52.0. All four facts and the merge case reproduce unchanged; see
      Artefacts and notes, `EV-A-TRANSCRIPTS`. No axiom changed, so the plan
      was not disturbed.
- [x] (2026-09-12) Prerequisite: `Cli` and `FormatOpts` are out of the
      composition root and into a binary-private module. The base branch had
      already made that move, and made it more completely: `src/command.rs`
      holds both declarations *and* `process_lines`, `format_lines`, and
      `formatting_closure`. This branch therefore carries no `src/cli.rs` of
      its own, and the `--git` additions to `Cli` are made in
      `src/command.rs`. See the rebase entry under Progress.
- [x] (2026-09-12) EP-M0, grammar half: the measured grammar reproduces on the
      real `Cli` under clap 4.6.6. All eight EV-M0-GRAMMAR cases match; the
      verbatim diagnostics are in Artefacts and notes, `EV-M0-GRAMMAR`. **No
      escalation**: the shape holds. Two gaps surfaced, both already in the
      class the plan anticipated, and both are fixed by the mechanism it
      already prescribes — see Surprises & discoveries. The spike was reverted;
      the final form lands in EP-M2, after its red tests.
- [x] (2026-09-12) EP-M0, BDD half: one scenario from this plan's feature file
      runs end to end, delivering `EV-M0-BDD`. The transcript is in Artefacts
      and notes. The `Step failed at index 5` line is the substance of the
      evidence: steps 0–3 are the `Background` and step 4 is the `When`, so the
      real-git fixture built, the binary launched, and only the `Then` failed —
      which is the intended red, `--git` being unknown to `Cli` as it stands.
- [x] (2026-09-12) Stage B, dependencies and specification: `thiserror = "2"`
      added (already in `Cargo.lock` at 2.0.20 as a transitive dependency, so
      nothing was fetched). `tests/features/git_file_selection.feature` written
      with 16 scenarios and a four-file `Background`, and bound from
      `tests/git_file_selection.rs` with step definitions in
      `tests/steps/git_selection.rs`.
- [x] (2026-09-12) Stage B, red evidence: `cargo test --test
      git_file_selection` reports **1 passed; 16 failed**. Every failure is at
      the `Then` or later, never in the fixture, and every one names the
      intended cause. Measured before any production code for the selection
      exists. Three fixture-level corrections were needed to reach that state
      and are recorded under Surprises & discoveries.
- [x] (2026-09-12) Stage B, gate posture: `make check-fmt`, `make typecheck`,
      `make lint`, `make markdownlint`, `make nixie`, and `cargo test --doc` are
      green; `make test` is red **only** on `git_file_selection`, by design. A
      `--no-fail-fast` sweep of the 27 targets cargo never reached found one
      genuine failure, now fixed: this plan document was not a two-pass fixed
      point, because prose added to it was hand-wrapped. See the new Surprises
      entry — hand-fixing it failed three times and the settled text had to be
      measured, not typed.
- [x] (2026-09-12) Stage B, red unit and property evidence: the five test files
      under `src/select/` are written against the Interfaces block, and the
      module files they name hold nothing but their dependency-direction docs,
      so the run fails to compile rather than passing for want of an
      implementation. `cargo test --bin mdtablefix --all-features select`
      reports **7 × E0432 unresolved-import errors**, naming `AmbientPathProbe`,
      `CandidateListing`, `ExtensionFilter`, `FileIdentity`, `GitListError`,
      `GitLsFiles`, `PathKind`, `PathProbe`, `split_nul_delimited`, and
      `select_files`, and ends `could not compile mdtablefix (bin "mdtablefix"
      test) due to 7 previous errors`. That is the intended reason: a
      compilation failure naming the missing item, not a spurious pass.
      Transcript in Artefacts and notes, `EV-M1-RED`. The red commit carries no
      `expect(dead_code)` attribute — see Surprises & discoveries for why the
      attribute belongs to EP-M1 instead.
- [x] (2026-09-12) EP-M1, the selection module: `extensions`, `policy`,
      `conflict`, `git_ls_files`, and `fs_probe` are implemented, and
      `cargo test --bin mdtablefix --all-features select` reports **69 passed; 0
      failed; 46 filtered out**. Discharged: INV-NUL-SPLIT, INV-EXT-SOUND,
      INV-EXT-COMPLETE, INV-DEDUP, INV-ORDER-DET, INV-PROBE-EXCLUSIONS,
      INV-CONFLICT-GUARD. `cargo clippy --all-targets --all-features -- -D
      warnings` is clean, which is what makes `main.rs`'s
      `expect(dead_code, …)` an evidence-backed declaration rather than a
      hopeful one: the attribute is still *fulfilled*, so nothing in the tree
      has been wired up yet. Transcript in Artefacts and notes, `EV-M1-SELECT`.
      Writing the implementation exposed four latent type errors in the red
      tests, which no name-resolution failure can surface; see Surprises &
      discoveries.
- [x] (2026-09-12) EP-M1, gate posture, measured with `scrutineer`:
      `make check-fmt`, `make typecheck`, `make lint`, `make markdownlint`, and
      `make nixie` are green. `make test` is red, and red only on
      `tests/git_file_selection.rs`, which reports **1 passed; 16 failed**,
      every failure being `error: unexpected argument '--git' found`. This is
      the posture the Decision log already accepts rather than a new defect:
      `--git` does not exist until EP-M2, so the committed behavioural suite
      cannot pass until then. Because the first `cargo test` command fails,
      `make test` never reaches its second, so `cargo test --doc --all-features`
      was run separately and reports **40 passed; 0 failed; 20 ignored**.
- [x] (2026-09-12) EP-M1, CodeRabbit review: **completed, zero findings**
      across 19 reviewed files, run through `scrutineer` against the local
      branch's own work. Transcript in Artefacts and notes, `EV-M1-CR`. No
      concern was raised about the red behavioural suite or the unwired module
      tree, both of which were given to the reviewer as stated intent rather
      than left to be inferred.
- [x] (2026-09-12) EP-M1, CodeRabbit review of the mutation work: **completed,
      zero findings** across 21 reviewed files — `EV-M1-CR`'s nineteen plus
      `Makefile` and `.cargo/mutants.toml`. Transcript in Artefacts and notes,
      `EV-M1-CR-2`. EP-M1 carries two reviews because the mutation tooling
      landed after the first, and it was the tooling that found the three
      survivors; a review of the implementation alone would have passed a
      milestone whose acceptance criterion was still unmet.
- [ ] EP-M1: confirm `std::fs::canonicalize` case behaviour on the macOS and
      Windows release targets, per the INV-DEDUP residual gap. **Carried
      forward, 2026-09-12: no machine in this environment can perform it.** The
      host is Linux; the branch's CI runs the suites on Linux and on Windows and
      has no macOS job; and the Windows job has no case-insensitive filesystem
      case that would exercise the question. `1f64236` made both platforms
      release targets but gave neither a runner that answers this. ADR 0010
      records the check among its known risks, the fallback is named in the
      residual-gap bullet above, and the consequence of the assumption failing
      is a redundant format with last-writer-wins content — never data loss. The
      box stays unchecked because the verification it names has not happened;
      ticking it would record a measurement nobody made.
- [x] (2026-09-12) EP-M1, mutation testing: `.cargo/mutants.toml` and
      `make mutants` exist, and the run reports **40 mutants: 34 caught, 6
      unviable, 0 missed** — `missed.txt` empty, exit status 0, over the whole
      `src/select/**` scope rather than the two files the criterion names alone.
      The first run, before the change below, reported **3 missed**. Transcript
      in Artefacts and notes, `EV-M1-MUTANTS`.
- [x] (2026-09-12) EP-M1, the three survivors that first run reported: all three
      were the failed-canonicalization path in `src/select/fs_probe.rs`, whose
      second arm is reachable only by losing a race with the filesystem. Fixed
      by moving the classification into `unnameable`, a function of
      `io::ErrorKind`, and testing both arms by constructing the errors rather
      than by staging a fixture that cannot be staged. See Surprises &
      discoveries.
- [x] (2026-09-12) EP-M2, production code: `git` joins the `inputs` group with
      the four supporting flags and the post-parse dependency check; a new
      binary-private `src/git_inputs.rs` resolves the selection and the conflict
      guard; `driver::Mode` gained `ListFiles`, answered by the first statement
      of `analyse`, before the read capability is cloned; `ConflictGuard`
      refuses a conflicted file only where the write would happen; and
      `run_files` drains in chunks of 256 through one `BufWriter` over one
      `stdout().lock()`. `src/main.rs` is 350 lines, inside both the 400-line
      cap and the 380-line escalation trigger. Every change is covered in the
      Decision log; two of them — `ListFiles`' capability and the guard's
      placement — deviate from this plan's earlier draft and say so there.
- [x] (2026-09-12) EP-M2, first green, before any test file was added:
      `cargo test --test git_file_selection` reports **17 passed; 0 failed** and
      `cargo test --bin mdtablefix` **136 passed; 0 failed**. The behavioural
      suite that was red from Stage B through EP-M1 turns green on the first
      run of the wiring, with no scenario amended.
- [x] (2026-09-12) EP-M2, `tests/cli_git.rs`: 15 tests pinning what a scenario
      cannot state — REQ-GIT-003's replacement semantics over four spellings of
      `--md-exts`, REQ-GIT-004, REQ-GIT-005's both halves, the four post-parse
      rejections with their usage footers, REQ-GIT-010 discharged by listing a
      tracked file whose bytes are not UTF-8, a `--git` failure outside a
      repository as one deliberate line, and the `--help` snapshot
      `tests/snapshots/cli_git_help.snap`. REQ-GIT-010's evidence is the one
      with teeth: the file cannot be decoded, so any mode that read it would
      fail, and `--list-files` lists it.
- [x] (2026-09-12) EP-M2, two measured corrections to the pinned help text,
      both found by running the binary rather than by reading the declaration —
      see Surprises & discoveries. The `--help` snapshot is what caught the
      first: it is the whole rendering, not the flags this milestone added.
- [x] (2026-09-12) EP-M2, `make mutants` with the widening in place: **60
      mutants: 53 caught, 7 unviable, 0 missed** — `missed.txt` empty and exit
      status 0, so the behavioural scenarios are part of the oracle rather than
      merely outside it. The first widened run reported **2 missed**, both in
      `src/select/git_ls_files.rs` and both a test's blind spot rather than a
      defect; two tests now kill them, and the run was repeated to confirm.
      Transcript in Artefacts and notes, `EV-M2-MUTANTS`.
- Measurement, 2026-09-12, against the Scope tolerance (more than 24 files
      touched, or more than 1600 net added lines): **EP-M2's own change is 16
      files — 13 modified and 3 new — with 669 insertions and 94 deletions in
      the tracked files and about 520 added lines across
      `src/git_inputs.rs`, `tests/cli_git.rs`, and the snapshot: roughly 1,190
      added, 95 removed, so ~1,095 net.** Both thresholds hold for the change.
      The *cumulative* branch does not: `git diff --stat
      origin/check-option..HEAD` reports 30 files and 5,374 insertions, most of
      them this plan document. The two readings cannot both be the intended
      one — the plan's Interfaces section names more than 24 files by itself,
      and the Tolerance bullet records that an earlier draft's 14 and 900 were
      "arithmetically unsatisfiable" — so the per-change reading is the one
      taken: the numbers bound a single milestone's diff rather than the sum of
      every commit the plan produces. Recorded rather than assumed, because a
      reader checking the tolerance will reach for the branch diff and find it
      over on both counts.
- [x] (2026-09-12) Rebase onto `origin/check-option` at `6a44a45`, which had
      advanced four commits since the previous rebase. No conflict arose: every
      file those commits touch — `src/report/delta.rs`, `src/report/render.rs`,
      `src/io.rs`, `src/io/replace.rs`, `src/io_tests.rs`,
      `tests/cli_check.rs`, `tests/cli_diff.rs`,
      `tests/document_properties.rs`, and `docs/execplans/check-option.md` — is
      one this branch leaves alone. Three of the four are test-only, and the
      fourth adds a `#[cfg(test)]` re-export and a `pub(super)` on
      `rewrite_with`, so no contract this branch consumes moved. Re-validated
      on the rebased tree before republishing: `make check-fmt`, `make test`,
      `make typecheck`, and `make lint` all exit 0, `make test` reporting
      **1988 passed, 0 failed, 20 ignored**. The four upstream patterns were
      assessed for adoption rather than passed over; see the Decision log.
- [x] (2026-09-12) EP-M2, CodeRabbit review: **completed, zero findings** over
      the 27 files of this branch's diff against `check-option`, its base. The
      branch's third round and the first to see the command-line surface,
      `src/git_inputs.rs`, and the end-to-end tests; EP-M1's two rounds saw 19
      files. No rate or seat limit appeared, so no wait was needed. Transcript
      in Artefacts and notes, `EV-M2-CR`.
- [x] (2026-09-12) EP-M3, ADR 0010: `docs/adrs/0010-git-file-selection.md`
      written — the context (the `git ls-files | xargs` idiom and its four
      defects), the three non-negotiable properties, the six decision drivers,
      the four options with their comparison table, the decision outcome in
      five subsections, the consequences, and nine known risks. Two passages
      come from measured runs rather than from drafting: a mid-merge refusal,
      and a failure outside any repository. Both corrected the draft — the
      refusal is an `anyhow` chain rather than one prefixed line, and its cause
      line is 123 characters, past markdownlint's `code_block_line_length` of
      120, so it is wrapped inside the fence with a note saying the wrap is the
      page's rather than the tool's. The ADR is a wrap fixed point (a second
      `--wrap` pass is byte-identical) and markdownlint-clean.
- [x] (2026-09-12) EP-M3, user's guide: the synopsis, five new rows in the
      flag table, "The three file modes" retitled to four with `--list-files`
      named, a new "Selecting files from Git" section with "Mid-merge safety"
      and "When the selection fails" beneath it, the exit-status row widened,
      the `.gitattributes` consequence added to the line-ending section, and
      three existing sections extended — paths that match nothing, symbolic
      links, and the in-place error context. Wrap-normalised until a second
      pass is byte-identical; three paragraphs outside the edits reflowed as a
      consequence, investigated rather than reverted and kept — see Surprises
      & discoveries.
- [x] (2026-09-12) EP-M3, architecture and developer's guides: a new "Git file
      selection" section in `docs/architecture.md` with a sequence diagram
      tracing `main::run` through `git_inputs::resolve`, `git ls-files`,
      `select_files`, and the guard, "three behaviours" corrected to four with
      `--list-files` named, and the contents list extended. The diagram is
      Figure 5, so the atomic-writes figure became Figure 6 and the plan's one
      cross-reference to it above was updated with it.
      `docs/developers-guide.md` gains "File selection is a second private
      tree": the module map, the inward dependency rule, the three levels the
      selection is tested at, and the fixture's neutralisation of the ambient
      Git configuration.
- [x] (2026-09-12) EP-M3, `README.md` and `docs/contents.md`: the README
      synopsis brought into line with the user's guide's one grammar, "Three
      flags select" replaced by the four modes plus a `--git` paragraph and an
      example, and both ADR 0010 and this plan indexed in the contents list.
- [x] (2026-09-12) EP-M3, this plan reconciled with ADR 0010: the Decision log
      gains three entries (measured transcripts rather than transcribed prose;
      the retained user's-guide reflows; the `.gitattributes` caveat recorded
      in both documents) and the Surprises section gains three. One stale doc
      comment fixed while drafting: `GitSelection::inputs` claimed Git's order
      while `select_files` sorts.
- [x] (2026-09-12) EP-M3, `EV-M3-DOCS`: six gates through `scrutineer` over the
      uncommitted documentation tree — `make check-fmt`, `make typecheck`, `make
      lint`, and `make test` (1988 passed, 0 failed, 20 ignored) green on the
      first run; `make markdownlint` red with one `MD038` inside this plan's own
      new Surprises entry, fixed by describing the measured line rather than
      quoting it in a code span split across two lines, then re-run alone to **36
      files, 0 errors**; and `make nixie` validating every diagram, the new
      sequence diagram among them. `cargo test --test idempotence_drift` reports
      **2 passed**. Transcript in Artefacts and notes, `EV-M3-DOCS`.
- [x] (2026-09-12) EP-M3, `EV-M3-CR`: `coderabbit review --agent --base
      check-option` through `scrutineer` at `9a83339` — **completed, zero
      findings** over 33 reviewed files. No rate, seat, or quota limit, so no
      wait was needed. Transcript in Artefacts and notes, `EV-M3-CR`.
- [x] (2026-09-12) EP-M3 complete: the branch is pushed, the plan's Status is
      COMPLETE, and `Outcomes & retrospective` records what was delivered
      against what was planned. Milestone work is finished; what remains is
      pull request #464's merge, which this stack waits on.
- [x] (2026-09-12) CI, the branch's newest run `34661535150` at `573deb9`:
      **failure on both test jobs**, over five defects that are all in test
      code. `build-test` (ubuntu-latest) fails at "Test and Measure Coverage"
      with one failure, `the_git_directory_is_resolved_through_git`:
      `git ["commit", "-m", "initialise"] failed with exit status: 128: Author
      identity unknown`, because the unit fixture's `git` helper inherited the
      environment and a runner has no configured identity — the developer's own
      machine has one, so the case is green locally and red on a runner.
      `atomic write contract (windows)` fails at "Test the whole suite" over
      four more, each a spelling that holds only where the platform spells
      things the way Linux does: `ExitStatus`'s own rendering (`exit status:`
      here, `exit code:` on Windows) pinned as text in
      `a_directory_outside_a_repository_has_no_git_directory`; the canonical
      path in `a_regular_file_is_identified_by_an_absolute_canonical_path`,
      which Windows renders `\\?\C:\...\docs\guide.md` — the verbatim marker as
      well as the other separator; the behavioural fixture keying its recorded
      bytes by `to_string_lossy()`, which made all seven git-selection scenarios
      read a `docs\guide.md` it had never written; and the `--help` snapshot
      carrying `mdtablefix.exe`, which `clap` prints because it prints
      `argv[0]`. Diagnosis from `gh run view 34661535150 --log-failed`, kept at
      `/tmp/ci-34661535150-failed.log`. The four packaging jobs, including both
      macOS rows, passed throughout — they build a binary and run it, and never
      compile a test target.
- [x] (2026-09-12) CI repair `6384543`: the five defects answered in four
      files, +45/−5, all of it test code and none of it the crate. The unit
      helper hardens its environment the way the integration fixtures already
      did, supplying the identity through `GIT_AUTHOR_*`/`GIT_COMMITTER_*` and
      removing the developer's configuration from the picture; the canonical
      path assertion compares by components through `Utf8Path::ends_with`,
      which is the platform's own spelling of the separator and, on Windows, of
      the verbatim prefix a canonical path carries; the exit-status assertion
      pins the prefix this crate words and leaves the status to the standard
      library; the behavioural fixture joins components with `/`; and the help
      snapshot normalises the executable suffix away. See Surprises &
      discoveries; the general shape is that a defect living only in the test
      tree is invisible to a developer machine that happens to spell things the
      way the test does.

  One repair beyond the five: the linked-worktree assertion in
  `the_git_directory_is_resolved_through_git`, which compared a path as
  `/`-spelled text, was changed to the same component-wise comparison. That
  assertion **never fired** — its fixture calls `git commit` before reaching
  it, so the identity failure panicked first and hid it — and the change is a
  judgement about the same latent defect rather than a response to an
  observation. It is recorded as such: had only the observed five been
  repaired, that suite would have gone green on the identity fix alone and left
  the assertion to fail on the next run in which it was reached with a Windows
  path.
- [x] (2026-09-12) CI repair re-gated locally, `EV-M4-GATES`: `make check-fmt`,
      `make typecheck`, `make lint`, and `make test` all green on `6384543` at
      the first attempt, strictly sequentially through `scrutineer`. `make
      lint` clean under `-D warnings` confirms the two
      `needless_borrows_for_generic_args` findings are gone; `make test` runs 48
      binaries with 944 unit tests and every integration, behavioural, and
      doctest suite reporting `0 failed`, and the two known-flaky idempotence
      suites passed without needing an isolated re-run. Logs
      `/tmp/gate-rerun-{check-fmt,typecheck,lint,test}-git-option.out`.
- [ ] CI, a run for `6384543` or for EP-M3's `9a83339`/`5f294b7`: **none
      exists, and none could exist while #466 conflicted.** The branch's newest
      run is `34661535150` at `573deb9` (00:24:40Z); the three commits pushed
      after it — `9a83339` at 00:38Z, `5f294b7` at 00:41Z, `6384543` at 10:43Z —
      have no run between them, where every earlier push had one within seconds.
      GitHub documents the cause: *"Workflows will not run on `pull_request`
      activity if the pull request has a merge conflict"*, and #466 was
      `CONFLICTING`/`DIRTY` against `check-option`. So EP-M3's two commits and
      the CI repair were CI-untested for as long as that conflict stood, and
      their green was the local gates and the CodeRabbit rounds. `EV-M4-CI-SILENCE`
      holds the earlier run list and the limits of what it proves.
      The silence is specific to the `pull_request` event rather than to the
      branch, and three runs are what show it. The pushes at 10:50Z, 10:51Z, and
      10:54Z each produced `34689669899`, `34689547517`, and `34689481906`,
      every one `completed` with conclusion `skipped`, and every one the
      `dependabot-automerge` workflow — whose trigger is `pull_request_target`,
      an event GitHub still delivers to a conflicting pull request. So a
      workflow that is not on `pull_request` did register on the same pushes
      that produced no test run at all, which is what separates the event that
      is blocked from the events that are not, and rules out "the branch stopped
      receiving webhooks" as an explanation.
- [x] (2026-09-12) **The conflict is cleared, the pipeline has re-opened, and
      its verdict is green**, which is the one thing the entry above could not do
      for itself. The second rebase's push produced run
      [`34691011897`](https://github.com/leynos/mdtablefix/actions/runs/34691011897),
      event `pull_request`, head `70d58ab1`, created 11:25:22Z — the branch's
      first `pull_request` run since `34661535150`, and the first that can carry
      EP-M3's two commits and the CI repair. That is the conflict hypothesis
      confirmed by the thing it predicted rather than by the absence it
      explained: no run while `CONFLICTING`, a run within seconds of the rebase,
      with nothing else about the branch changed except its parent. It concluded
      **`success`** at 11:32:11Z, all six jobs green: `build-test` in 4m11s,
      `atomic write contract (windows)` in 6m46s — including "Test the whole
      suite", the step the branch's own red run never reached — and all four
      `binstall packaging` jobs, `x86_64-unknown-linux-gnu` first at 1m01s and
      `aarch64-apple-darwin` last at 1m19s. So the five test-code defects
      `6384543` repairs are repaired on Linux and on Windows both, and the
      branch's first green CI run is also its first on two platforms; the tool's
      behaviour was never the red part, and it is green where it is now measured.
      What the verdict describes is worth stating precisely, because the head
      has moved since: it is the tree at `70d58ab`, whose only difference from
      the pushed tip is `docs/execplans/check-option.md`, a file belonging to the
      base. `git diff --stat` restricted to `src`, `tests`, and the manifests is
      empty across the third rebase, so the verdict transfers to the rebased code
      rather than having to be re-earned by it — and the push after the third
      rebase produced a run of its own, so the transfer is checked rather than
      assumed. [`34691436697`](https://github.com/leynos/mdtablefix/actions/runs/34691436697),
      also `pull_request`, on `70919bc`, created 11:35:19Z, concluded **`success`**
      at 11:41:47Z with all six jobs green again: `atomic write contract
      (windows)`, `build-test`, and the four `binstall packaging` jobs. Both of
      the branch's runs since its conflict was cleared are green — on `70d58ab`
      and on `70919bc` — and no job in either has been anything but `success`.
      Later commits here move the head again, but only inside this plan file, so
      the shipped tree those two runs measured is the shipped tree under review.

- [x] (2026-09-12) The base branch is still moving, and its own redness has
      since been fixed upstream. While this repair was being gated
      `origin/check-option` gained `530bdbd` (10:39Z) and `914e9ae` (10:43Z),
      and its Windows job was failing on something that was not ours:
      `error: unused import: std::fs` at `tests/cli_check/arguments.rs:3`, a
      file-scope import reached only from inside a `#[cfg(unix)]` test with
      `RUSTFLAGS: -D warnings` in force on the Windows runner. It has since
      gained `3737276` ("Gate the non-UTF-8 argument test's imports to Unix"),
      which is that defect's fix, and `64117e4`, which records it — so the base
      is repairing itself and needs no help from this branch. It has also moved
      to `64117e4` (10:48Z), fifty-seven commits past the merge-base `408c76a`
      — a distance recorded here as twenty when the entry was written, which was
      simply wrong, and is corrected on re-measurement. Decision taken not to
      rebase onto it — see the Decision log, which this strengthens: a base
      repairing its own failures is a base worth waiting for rather than
      chasing. That decision was **superseded** the same day by the requester's
      instruction to rebase; see the rebase entry below.
- [x] (2026-09-12) CI repair, `EV-M4-CR`: `coderabbit review --agent --base
      check-option` through `scrutineer` at `0ee306b` — **completed, zero
      findings** over 95 reviewed files, the fifth round on this branch and the
      third consecutive one to cover a milestone's close. The reviewed set
      matches `git diff --name-only origin/check-option...HEAD` exactly and the
      run's own context line names the two branches, so the zero is about this
      branch's diff rather than about a review that deferred to the pull
      request — a distinction `EX-M4-CR-SCOPE` records, since a deferral and a
      clean review both read as "no findings". No rate, seat, or quota limit, so
      no wait was needed. Transcript in Artefacts and notes, `EV-M4-CR`.
- [x] (2026-09-12) **Rebased onto `origin/check-option` at `64117e4`**, on the
      requester's instruction, so the branch stops conflicting with its base and
      the pipeline re-opens. The pre-rebase tip was `866a935`, which
      `origin/git-option` still holds and
      `backup/git-option-pre-check-option-rebase` tags locally; the rebase left
      the branch 30 commits past the base, at `9af112c`, and this record's own
      commit sits on top of that, so a later reader counts 31. Its diff against
      the base is 36 files, 8240 insertions and 144 deletions as the rebase
      finished. The count on top
      is not the count replayed, so the arithmetic is worth stating: 62 commits
      lay between the merge-base `408c76a` and the pre-rebase tip;
      `origin/check-option` had already landed 30 of them verbatim, the base
      being cut from the same work, and git 2.52's default
      `--no-reapply-cherry-picks` dropped each of those; 32 were replayed; and 2
      of the 32 were skipped by hand, leaving the 30 that stand. The two skipped
      are the `src/cli.rs` extraction (`5789508`), superseded by the base's
      own `src/command.rs`, and the EP-M7 documentation commit (`7b59154`), six
      of whose twelve files are byte-identical to the base's and whose remaining
      text the base's later, fuller version replaces. Nothing on the branch now
      names `src/cli.rs`: the `EV-M1-CR` and `EV-M2-CR` transcripts under
      Artefacts and notes each list it among their reviewed files, and they
      record rounds run before this rebase, describing a file the rebased branch
      does not carry.
- [x] (2026-09-12) The rebase's one structural decision, `EX-REBASE-STRUCTURE`:
      **`src/command.rs` is this branch's single command module, and the `--git`
      additions to `Cli` are made there.** Both branches performed the same
      extraction of `Cli` and `FormatOpts` out of `src/main.rs` — this one into
      `src/cli.rs`, the base into a `src/command.rs` that also holds
      `process_lines`, `format_lines`, and `formatting_closure` — and only one
      can stand. Each declares `Cli` and `FormatOpts`, so keeping both would
      leave `src/main.rs` choosing between two distinct `FormatOpts` types, and
      the pipeline functions exist only in the base's module. `src/cli.rs` never
      lands, and the base's module doc gained this branch's `--git`
      dependencies rather than the reverse.
- [x] (2026-09-12) The base's new patterns adopted, as the instruction to carry
      over pertinent changes requires. Four, each a requirement rather than a
      preference: `crate::command` is the path `src/git_inputs.rs` and
      `src/main_tests.rs` import `Cli` and `FormatOpts` from; `Mode::ListFiles`
      gained its arm in `src/metrics.rs`'s `mode_label` and its element in
      `src/metrics_tests.rs`'s `MODES`, without which the base's new metrics
      module does not compile at all; the base's split of `src/driver_tests.rs`
      into `driver_report_tests.rs` and `driver_in_place_tests.rs` is taken as
      the base has it, with this branch's nine `ConflictGuard::unguarded()` call
      sites ported into the two halves (six and three) and the old file removed
      by `git rm`; and the base's American spelling won the one wording conflict
      — `--fences` normalizes rather than rewrites — which is propagated to
      `tests/snapshots/cli_git_help.snap` so the snapshot and the help text
      agree.
- [x] (2026-09-12) The branch's mutation evidence carries across rather than
      being re-earned, and the measurement is what says so:
      `git diff 866a935 HEAD -- 'src/select/**'` is empty, so every file
      `EV-M2-MUTANTS` mutated is byte-identical on the rebased tree. The whole
      of this branch's diff against its pre-rebase tip outside the base's own
      arrival is two lines — the `crate::cli` to `crate::command` import path
      in `src/git_inputs.rs`, and the same rename in a doc link in
      `src/select.rs` — neither of which is in the mutation scope, and
      `.cargo/mutants.toml`
      pins to `src/select/**`. So the 60 mutants, 53 caught and 7 unviable, still
      describe this tree, and `make mutants` was not re-run for that reason
      rather than by omission.
- [x] (2026-09-12) **Rebased a second time, onto `origin/check-option` at
      `49a2d0a`**, because the base moved while the first rebase was being
      validated. Two commits landed in that window: `9834fcb` ("Heal the
      callsite interest cache in every traced test", 13:13:29 +0200) and
      `49a2d0a`, which records it. A rebase onto a base that has since moved is
      a rebase whose claim to have cleared the conflict is already stale, so the
      instruction was applied again rather than pushing what the first produced.
      The second rebase was clean — no conflict — and left the branch 31 commits
      past the base, at `eda86ca`, the extra one being this record's own commit.
      Its diff against the base is 36 files, 8448 insertions and 144 deletions:
      the earlier 8240 insertions plus the 208 this record adds.
      `origin/check-option` is an ancestor of the tip, and the tree is clean.
      The replay was clean for a reason worth stating rather than assuming. The
      base's two edited test files, `src/driver_report_tests.rs` and
      `src/main_tests.rs`, are edited on this branch as well, but the base's
      hunks are the import line and the attribute positions while this branch's
      are the `ConflictGuard::unguarded()` call sites, so the two sets do not
      overlap. Reading the merged file is what confirms both survived: the
      wrapper import sits at `src/driver_report_tests.rs:10`, the attribute at
      line 131, and all nine guard call sites remain, six and three.
- [x] (2026-09-12) The base's second new pattern, `test_macros::traced_test`,
      **assessed rather than adopted, because this branch adds no traced test to
      convert**. Every traced test in the repository now names the in-repo
      wrapper rather than `tracing_test::traced_test`: the wrapper prepends
      `::tracing::callsite::rebuild_interest_cache()` to the test body and hands
      that body to `tracing_test::traced_test`, which prepends its own
      subscriber install, so the rebuild always follows the install. The defect
      it answers belongs in this record because it is general: `tracing` decides
      once, when a callsite is first used, whether it can ever be dispatched, so
      a callsite used before that lazy install caches `Interest::never()` for
      the life of the process and stays silent — which is how the base's Windows
      job saw 937 tests pass and one traced-snapshot test fail. This branch
      introduces no traced test of its own, so it complies with the base's new
      convention without a call-site change, and the measurement is a grep:
      every `tracing_test::` mention left under `src/` is the wrapper's own
      explanatory comment.
- [x] (2026-09-12) A caution recorded rather than an item this plan owes: the
      base can move again. A third rebase becomes necessary only if it does,
      before #464 merges, and nothing else here depends on that timing.
      `origin/check-option` is pull request #464, still open and still being
      worked — the base's own CI for `9834fcb`,
      [`34690495578`](https://github.com/leynos/mdtablefix/actions/runs/34690495578),
      was still running while the second rebase was performed. So `49a2d0a` is
      the tip this branch was rebased onto, not a tip that promises to hold
      until #464 merges. **It moved once more, and the caution was answered
      rather than merely restated; see the third rebase below.**
- [x] (2026-09-12) **Rebased a third time, onto `origin/check-option` at
      `e462b97`**, which is the caution above being cashed: the base gained
      "Record the green Windows job against the seam fix" at 13:22:35 +0200,
      three minutes before the second rebase's push landed, so the branch that
      rebase produced was one commit behind the tip the instruction names. The
      arithmetic is small enough to state rather than argue about: one base
      commit to absorb, 32 to replay, and the replay was clean. What moved is
      exactly what that base commit touched — 27 lines, 19 insertions and 8
      deletions, all inside `docs/execplans/check-option.md`, a file this branch
      never touches. `git diff --stat 70d58ab HEAD` names that one file and
      nothing else, which is the measurement rather than the claim, and it is
      why the gates and the mutation record describe the same tree on either
      side of the third rebase: every file under `src/` and `tests/`, and every
      manifest, is byte-identical across it. `origin/check-option` is an ancestor
      of the tip and the branch stands 0 behind and 32 ahead of it as the rebase
      finished, the 33rd commit being this record's own — a count stated rather
      than implied because it is the kind of number a later reader measures and
      finds off by one. The pre-third-rebase tip is tagged
      `backup/git-option-pre-third-rebase` so the superseded head is recoverable
      if a later reader wants it.
- [x] (2026-09-12) **Rebased a fourth time, onto `origin/check-option` at
      `6be1a76`**, because the base moved once more — three commits in this
      window, opening with `80f80d1` ("Assert the failing writer's message, not
      only its error kind", 13:36:01 +0200) and closing with its plan record
      `6be1a76` at 13:44:52. This one is worth distinguishing from the third
      rather than filing as one more of the same. `e462b97` moved only the
      base's prose; `80f80d1` moves `src/report/render_tests.rs`, a test file
      that is part of this branch's tree even though this branch never edits it.
      So the replay was clean for the ordinary reason — the file is untouched on
      this side, and `git diff --stat d1d10d4 HEAD` names exactly the base's two
      files and nothing else, `docs/execplans/check-option.md` at +71 and
      `src/report/render_tests.rs` at +26/-5 — but the tree the gates measure is
      **not** byte-identical across this rebase, which is what separates it from
      the third and is why the full gate set was re-run rather than the Markdown
      one. The branch stands 0 behind and 34 ahead, and the pre-fourth-rebase
      tip is tagged `backup/git-option-pre-fourth-rebase`.
- [x] (2026-09-12) **The fourth rebase's push is green on both platforms**, and
      so is the push before it. Run
      [`34692137192`](https://github.com/leynos/mdtablefix/actions/runs/34692137192),
      `pull_request` on `4de8a7d`, created 11:51:41Z, concluded **`success`** at
      11:58:28Z with all six jobs green: `atomic write contract (windows)` in
      6m44s, `build-test` in 3m25s, and the four `binstall packaging` jobs, the
      longest of them at 1m56s. The run it superseded,
      [`34691869670`](https://github.com/leynos/mdtablefix/actions/runs/34691869670),
      on `d1d10d4`, the third rebase's record push, also concluded `success`.
      That makes four `pull_request` runs since the branch's conflict with its
      base was cleared — on `70d58ab`, `70919bc`, `d1d10d4`, and `4de8a7d` —
      every one of them green on Linux and Windows, with no job in any of them
      anything but `success`. What that buys is the claim this plan makes rather
      than a stronger one: every file the branch ships has now been measured on
      both platforms. "The newest commit has a run" would be false again the
      moment a record commit moves the head, and it does not need to be true,
      because a commit that changes only this plan puts no shipped file back
      under test.
- [x] (2026-09-12) CodeRabbit round six over the rebased tree, `EV-REBASE-CR`:
      `coderabbit review --agent --base check-option` through `scrutineer` —
      **`review_completed`, zero findings** over 36 reviewed files, the round
      requested after the rebase. Checked the way every round on this branch is
      checked: the reviewed set equals
      `git diff --name-only origin/check-option...HEAD` path for path, 36 to 36,
      with no file in one set and not the other, and the run's own context line
      names `git-option` against `check-option`. So the zero is about this
      branch's diff rather than about a review that deferred to the pull
      request, which is the distinction `EX-M4-CR-SCOPE` records because a
      deferral and a clean review both read as "no findings". No rate, seat, or
      quota limit was reported, so no wait was needed.
- [x] (2026-09-12) **The rebase instruction was re-issued and measured a
      no-op**, which is a result rather than a step skipped. `origin/check-option`
      has not moved since the fourth rebase: `git ls-remote origin
      refs/heads/check-option` returns
      `6be1a76af63045837205cff7fcbeafffa1b746b1`, the commit this branch was
      rebased onto, `git merge-base --is-ancestor origin/check-option HEAD`
      succeeds, and `git rev-list --left-right --count
      origin/check-option...HEAD` reads `0 36`. Nothing was replayed and no
      conflict arose, so there was nothing to resolve and no recovery path to
      exercise; the branch already stood on the rebased tree rather than
      awaiting one. Replaying the rebase here would have rewritten 36 commits
      to arrive at the tree they already stood on, and that is the cost the
      measurement avoids.
- [x] (2026-09-12) **An entity-level reading of the branch's diff, taken with
      `sem`**, as a second view beside `git diff --stat` rather than instead of
      it: 36 files, 460 entity changes — 306 added, 129 modified, 2 renamed, 22
      orphan, and one chunk reported as deleted. Each flagged item is accounted
      for by the line diff, which is what the reading is for: the two renames
      are the user guide's heading retitled from three file modes to four and a
      `Cargo.lock` chunk whose boundaries moved, the deleted chunk is the
      `Makefile`'s `.PHONY` line rewritten to list the new `mutants` target
      (`git diff --numstat` gives that file 15 insertions and 1 deletion, and
      the deletion is that line), and the orphans are prose and test-support
      entities outside the summariser's reference set. A summariser printing
      "renamed" is not itself evidence of a rename, so the reading is recorded
      with its reconciliation rather than as a finding.
- [x] (2026-09-12) The four gates the re-issued instruction names were run over
      the pushed tree through `scrutineer`, sequentially, and all four are
      green at `8e4d734`: `make check-fmt` (exit 0, no diff),
      `make typecheck` (exit 0), `make lint` (exit 0 — `check-static-regexes`
      clean and clippy under `-D warnings` reporting zero warnings), and
      `make test` (exit 0 — 2014 passed, 0 failed, 20 ignored across 48 result
      lines, the largest binaries reporting 947, 244, and 152 passing, with 40
      doctests passing). Logs:
      `/tmp/gate5-{check-fmt,typecheck,lint,test}-git-option.out`.
- [x] (2026-09-12) **Pull request #466 was marked ready for review**, ending
      the draft posture it had carried since it was opened. The draft was
      deliberate — a stack's upper pull request is normally held while the
      lower one is unmerged, so that a reviewer reads a diff whose base is
      fixed — and `gh pr view 464` gives `state OPEN` with `mergedAt null`, so
      the reason had not expired when the instruction superseded it. The
      Decision log records the reversal, the cost it carries (comments may
      land on a base that can still move, at one rebase per move), and the
      mitigation this plan already practises: each rebase is recorded beside
      the commit each verdict describes.
- [x] (2026-09-12) **A CodeRabbit review was requested on the pull request
      itself**, which reverses the omission the Decision log records, because
      the requester asked for one and the omission was written down as
      reversible by asking. Queued through `comenq` on this host — `hostname
      -f` gives `rohga.df12.net`, the daemon's own host, so the command was run
      locally and a review for this pull request needed no SSH wrapper:
      `comenq put leynos/mdtablefix 466 "@coderabbitai review"` returned
      identifier `fa732d8c` with an ETA of about 1h 02m, queued behind two
      other repositories' comments. `comenq list` was read before queueing and
      `comenq hist -n 15` contains no earlier entry for #466, so this is a
      first request rather than a double-queue. This is a different instrument
      from the six `coderabbit review --agent` rounds: it asks the hosted
      service to review the pull request, which is what the ready-for-review
      state makes available, where a queued review on a draft is skipped and
      spends the seat hour for nothing. Reading the pull request before queueing
      turned up a fact the queue alone would have hidden: CodeRabbit was already
      reviewing. Its walkthrough comment — one comment, edited in place each
      round, which is why it carries a creation date of 2026-09-09 — was last
      updated at 12:09:11Z with the range `6be1a76af63045837205cff7fcbeafffa1b746b1`
      through `9f187a270ba94e5042aec94e4cee81b5e0827826`, 34 files selected and
      `Cargo.lock` and the `--help` snapshot excluded by path filters. That
      review then ended without a verdict and without findings, which is worth
      recording precisely because an empty result reads like a clean one: the
      walkthrough comment is 770 characters of scaffolding with its "review in
      progress" block removed, `gh api .../pulls/466/comments` returns an empty
      list, and the `Kody Code Review` check run on the head commit is
      `completed skipped`. So the queued comment is not a second request
      duplicating a first — it is the request that will produce the round, and
      the push's own attempt was skipped by the seat cooldown rather than
      declined on merit. The same reading found a third reviewer's verdict worth
      recording because it is a limit rather than a finding: Sourcery declined
      the pull request with "larger than the review limit of 150,000 diff
      characters", which is a property of the diff's size and not a defect in
      it, and no round here treats that as an open item.

Superseded and deliberately not carried forward: adding `googletest`,
`pretty_assertions`, `rstest-bdd`, and `rstest-bdd-macros`; adding
`cargo test --doc` to the `test` target; and implementing
INV-NOWRITE-UNCHANGED. Pull request #464 does all four. The rebase adds two more
that this branch drops rather than carries: the `src/cli.rs` extraction, which
the base's `src/command.rs` supersedes, and the EP-M7 documentation commit,
whose every file the base carries in a fuller form.

## Surprises & discoveries

- Observation: **a red test that fails for the wrong reason is as worthless as a
  green one that passes for the wrong reason**, and three of this plan's
  rejection scenarios were in exactly that state. With the feature file as
  written, "Reject an unusable extension", "Reject combining `--git` with
  explicit file arguments", and "Reject `--list-files` without `--git`" all
  **passed before any production code existed** — an unknown `--git` is itself
  an exit-2 clap error, so the exit-status assertion was satisfied by the
  absence of the feature under test. Evidence: the first Stage B run reports
  `3 passed; 14 failed`; after the two message assertions were added and the
  test target rebuilt, `1 passed; 16 failed`, the survivor being the one
  scenario the plan deliberately leaves on clap's exit status. Impact: the two
  scenarios whose diagnostic text is **ours** now assert it —
  `extension is empty` from `ExtensionSpec`'s `Display`, and
  `--list-files requires --git` from the post-parse check. All three gained
  `stdout is empty`, which denies a naive implementation the escape of treating
  the flags as inert and formatting the positional as though `--git` were
  absent. Only `--git notes.md` remains status-only, because its wording is
  clap's.

- Observation: **the feature file is read at macro-expansion time, so editing it
  does not by itself trigger a rebuild.** An unchanged test binary re-ran the
  previous scenarios and reported the previous results. Evidence: the run after
  the edit still reported `3 passed; 14 failed`; after
  `touch tests/git_file_selection.rs` the same command reported
  `1 passed; 16 failed`. Impact: recorded here so that a later revision of the
  specification is not misread as having had no effect. `cargo test` cannot see
  the dependency, and no build-script indirection is worth adding for it.

- Observation: **`GIT_CONFIG_GLOBAL=/dev/null` leaves the fixture's `git commit`
  with no identity, so the first commit fails outright.** The plan's hardening
  list omitted the four `GIT_AUTHOR_*`/`GIT_COMMITTER_*` variables. Evidence:
  the initial Stage B run failed in the `Background` of all 17 scenarios, at
  the first `commit_file`, before any `When` had run. Impact: the fixture
  supplies the identity through the environment rather than a configuration
  file, because a file inside the repository is precisely what the selection
  tests read. The hardening list has been amended.

- Observation: **the `ignored_file` fixture step asserted its premise
  backwards.** `git check-ignore` exits zero when the path *is* ignored, and
  the step asserted `!success()`, so it rejected its own correct fixtures.
  Evidence: the first run's message,
  `build/out.md must not be ignored, or the scenario proves nothing`, firing in
  the `Background` of every scenario. Impact: fixed, and the assertion kept —
  its message now reads "must be ignored". Worth recording because the
  inversion is invisible in a passing run: it turned a premise check into its
  own negation, and only fired because the fixture happened to be correct.

- Observation: **a `Slot` cannot hold a `TempDir` read with `get`**, because
  `get` requires `T: Clone` and `TempDir` is not `Clone`. Evidence:
  `the trait bound TempDir: Clone is not satisfied` at the first `repo_path`
  call. Impact: the `TempDir` slots are filled through `get_or_insert_with`, so
  the directory is created lazily by the first `Given` that touches it rather
  than by a dedicated step. `tests/steps/reporting.rs` already used this form,
  so the plan's step sketch was the thing out of step, not the pattern.

- Observation: **`--list-files` with a positional argument is accepted by the
  grammar**, and the plan's own reasoning for why it need not be guarded is
  wrong. The plan says "`--list-files` no longer needs it, because `mode`
  already requires `inputs` and `--git` is the only way to satisfy that without
  positional files" — but a positional argument *is* a way to satisfy `inputs`,
  so `mdtablefix --list-files notes.md` parses. This is the same false-accept
  the plan measured for `requires = "git"`, reached by a different route.
  Evidence: `EV-M0-GRAMMAR`, where `--list-files a.md` exits 2 only because
  `a.md` is absent — the parse succeeded. Under the feature file's own fixture,
  where `notes.md` exists, it would exit 0. Impact: the scenario "Reject
  `--list-files` without `--git`" would fail as written. `--list-files`
  therefore joins `--include-untracked` and `--allow-conflicted` in the
  post-parse check, rejected when `--git` is absent, by the mechanism the plan
  already prescribes for exactly this class of bug. The command-line shape does
  not change, which is why this is recorded rather than escalated.

- Observation: **an explicitly-given `--md-exts` without `--git` is accepted**,
  which the plan predicted would need the `ArgMatches::value_source` treatment
  rather than a bool test. Confirmed: `mdtablefix --md-exts md a.md` reaches
  the file stage instead of being refused. Evidence: `EV-M0-GRAMMAR`,
  supplementary cases. Impact: EP-M2's post-parse check keys `--md-exts` on
  `value_source(..) != Some(ValueSource::DefaultValue)`, as the plan already
  specifies. A bool test is impossible here: `--md-exts` always has a value,
  because `default_values` supplies one.

- Observation: the ADR numbers moved twice between the plan's last revision and
  the start of implementation. `docs/adrs/` now holds **0008** (byte-order-mark
  preservation) and **0009** (check and diff reporting), both delivered on
  `check-option`; the plan's draft said its record would be 0008. Evidence:
  `ls docs/adrs/` on the rebased tree. Impact: this plan's decision record is
  **ADR 0010**, and every reference in this document has been renumbered.
  Nothing else in the plan depends on the number.

- Observation: pull request #464 has not merged, and its own ExecPlan records
  `EP-M6` as unrun, so "wait for the merge" would have stalled this plan
  indefinitely. Evidence: `gh pr view 464` reports `OPEN`;
  `docs/execplans/check-option.md` reads `Status: IN PROGRESS`. Impact: on the
  requester's instruction the work is stacked on `check-option` rather than
  sequenced behind it. The plan's composition section is unchanged, because the
  interfaces it names are present on that branch exactly as described; what
  changes is only which ref they arrive from.

- Observation: `src/main.rs` on `check-option` is 386 lines against a 400-line
  cap, and `src/driver.rs` is 369. Evidence:
  `git show origin/check-option:src/main.rs | wc -l`. Impact: the file-size
  contingency became a prerequisite. `Cli` and `FormatOpts` must move to
  `src/cli.rs` before any field is added, and `--git` wiring cannot live in
  `driver.rs`.

- Observation: atomic replacement changes what deduplication should key on.
  Evidence: `docs/v0-6-0-migration-guide.md` — "handles, hard links, and
  watches tied to the previous file keep the old contents and do not follow the
  replacement". Impact: `FileIdentity` becomes the canonicalized path rather
  than `(st_dev, st_ino)`. Collapsing two hard links would now format one and
  leave the other stale. See INV-DEDUP.

- Observation: the destructive symlink write this plan was designed to prevent
  is already impossible. Evidence: `src/io/replace.rs`, `replace_file_inner`
  reads `symlink_metadata` and returns `InvalidInput` for a symlink. Impact:
  INV-PROBE-EXCLUSIONS survives with a different and weaker rationale —
  avoiding a spurious error on a file the user never named — and commit
  `83e6150` makes that error intermittent by skipping unchanged files.

- Observation: `--headings` convergence (issue #474) was fixed by #477, but
  `--code-emphasis` still needs two passes, and nothing tracks it. Evidence: a
  sweep of 110 fixtures under `tests/data/` at `408c76a` found
  `tests/data/cli-matrix/table-prose.dat` differing between pass one and pass
  two under `--code-emphasis --in-place`, and identical between passes two and
  three. `tests/idempotence_drift.rs` gates only the `make fmt` flag set and
  that set plus `--headings`. Raised as issue #478; its siblings #468, #474 and
  #375 are all closed. Impact: `--git --check` is a sound gate for the gated
  flag sets and not for `--code-emphasis`. Do not cite a green
  `tests/idempotence_properties.rs` as evidence of convergence for an ungated
  flag — its generators do not reach these shapes.

- Observation: `git ls-files -t` does not reliably flag a tracked file deleted
  from the working tree; it reported `H`, not `R`, because the `R` tag requires
  `--deleted`. Evidence: probing a repository with a committed then deleted
  `a.md` produced `H a.md`. Impact: status tags cannot substitute for a
  filesystem probe. This is why the design uses `PathProbe` rather than parsing
  `-t` output.

- Observation: `rstest-bdd` 0.5.0 does not re-export its procedural macros.
  Evidence: its `lib.rs` re-exports `context`, `registry`, `pattern`,
  `localization` and others, but nothing from `rstest_bdd_macros`. Impact:
  `rstest-bdd-macros` is a required second dev-dependency. The upstream
  README's install block is incomplete.

- Observation: `tests/table/` is not compiled by any Cargo target. No top-level
  `tests/*.rs` declares `mod table` or a `#[path]` to it. Evidence: an
  exhaustive grep of `tests/` returned no match; the only `mod table` in the
  repository is `src/lib.rs`'s unrelated production module. Impact: do not add
  tests to `tests/table/`; they would never run. Do not fix it here either —
  out of scope.

- Observation: `make test` runs `cargo test --all-targets`, which excludes
  doctests. Evidence: the `test` target in `Makefile`. Impact: any `# Examples`
  block is unverified by every current gate. Stage C adds `cargo test --doc`.

- Observation: **hand-wrapping prose in this plan is what breaks
  `idempotence_drift`, and hand-fixing it does not work.** The gate runs the
  binary over every `docs/**/*.md` with
  `--wrap --renumber --breaks --ellipsis --fences` and asserts that pass 1
  equals pass 2. A paragraph wrapped by hand to a different width than the
  tool's is not settled after one pass, so the document fails even though
  nothing is wrong with the prose. Three separate attempts to type a correct
  fixed point all failed, including one that introduced a line with a trailing
  space, which the tool then propagated. Evidence:
  `docs/execplans/git-option.md` was the single failing file in an otherwise
  green corpus;
  `target/debug/mdtablefix --wrap --renumber --breaks
  --ellipsis --fences --diff docs/execplans/git-option.md`
  shows it drifting from its own first pass. Impact: **do not hand-wrap
  paragraphs added to this plan.** When the gate fails, converge the file with
  the tool rather than editing by eye: copy it aside, run the flag set with
  `--in-place` until the output stops changing, take the settled region from
  that converged copy, and splice it in — then confirm by running the flag set
  twice more and diffing, as was done here. The whole-file alternative was
  measured and rejected only for diff noise: it is 765 changed lines against
  264 for the splice. Note also that the drifted region showed up *only* as
  pass1 ≠ pass2; pass0 ≠ pass1 is normal for this corpus and is not what the
  gate checks.

- Observation: **red-before-green and "gate every commit" cannot both hold**,
  and this plan mandates red-before-green while EP-M1 mandates that the CLI
  stay unchanged. The consequence is unavoidable rather than incidental: from
  Stage B until EP-M2 lands, `make test` reports `git_file_selection` as
  failing, and no smaller commit can make it pass. Evidence: EP-M1's own
  "Remaining gaps: no CLI flag", against Stage C's "Write the feature file and
  its bindings … before any production code". Impact: the Stage B and EP-M1
  commits are deliberately red at `make test`, and say so in their messages.
  Everything else — `make check-fmt`, `make markdownlint`, and `make nixie` —
  is green on both. The window is narrower than the first Stage B commit
  suggested, and the reason is worth stating: the red state is a *compile*
  failure of the binary's test target, and `make typecheck` and `make lint`
  both run `--all-targets`, so from the EP-M1 red tests until EP-M1 lands those
  two gates are red as well, not just `make test`. Measured, not assumed: the
  six-green posture recorded in the previous Progress entry was taken before
  those test files existed. No CodeRabbit review is requested until EP-M2
  restores a fully green tree. The alternative considered and rejected was
  pinning the scenarios to current behaviour so they pass, which is impossible
  here: the feature under test does not exist at all, so there is no current
  behaviour to pin. The second alternative, parking the scenarios behind
  `#[ignore]` or a cargo feature, is forbidden by Stage B and would leave the
  target outside `make test` permanently rather than for two commits.

- Observation: **a private module that nothing calls is dead code even when its
  contents are `pub`, and the compiler will not let that state persist
  silently.** EP-M1 lands the selection tree one milestone ahead of the code
  that calls it, so between EP-M1 and EP-M2 every item in `src/select/` is
  unreachable, and `clippy --all-targets --all-features -- -D warnings` fails
  the commit. Measured against the real gates with a scratch `pub fn` in a
  private module of the binary: `` error: function `scratch` is never used ``,
  from `-D dead-code` implied by `-D warnings`. Evidence: the reproduction is
  in Artefacts and notes, `EV-M1-DEADCODE`. Decision: EP-M1 declares the tree
  with `#[cfg_attr(not(test), expect(dead_code, …))]` around `mod select;`,
  rather than with a plain `allow`, which would rot: an `expect` that becomes
  unnecessary is itself an error (`unfulfilled_lint_expectations`), so EP-M2
  cannot forget to remove it. Measured the same way — the attribute passes
  clippy while the module is unwired, and fails with
  `error: this lint expectation is unfulfilled` once `main` calls into it.
  Impact: the removal in EP-M2 is enforced by the toolchain rather than by
  review. The red commit of Stage B is the exception the other way round: its
  modules hold no items at all, so the expectation would be unfulfilled and the
  attribute must not appear there yet.

- Observation: **the test modules are separate files, declared one level below
  `src/select.rs`**, as `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`
  inside each module file. This is the `src/wrap/continuation.rs` and
  `src/wrap/paragraph.rs` precedent, adopted for the file-size cap rather than
  for taste: `policy`'s tests alone run to about 330 lines, and CON-SIZE-001
  caps a file at 400. Declaring them at the `src/select.rs` level was drafted
  first and rejected, because it would have contradicted this plan's own
  artefact list. Two consequences are load-bearing. The property tests reach
  the parent with `use super::…`, so an item that stops being visible to its
  own test module is a compile error rather than a silent gap. And INV-DEDUP's
  one case that needs the real filesystem — two hard links, one inode, two
  directory entries — lives in `fs_probe_tests.rs`, not `policy_tests.rs`,
  because `policy` may not import `std::fs` at all; that constraint is stated
  in the module's own doc comment and is what the test placement respects.

- Observation: **the Interfaces block needed three small corrections, all found
  while writing the tests it specifies.** `FileIdentity` gains
  `from_canonical_path` and `as_path`, because the tests must build an identity
  without a filesystem and must read one back; its doc comment said "On Unix
  this is `(st_dev, st_ino)`; elsewhere it is the canonicalized path", which
  the revised INV-DEDUP obligation had already reversed and which is now
  corrected in place. And `ExtensionFilter` gains `impl FromIterator<String>`,
  so a filter can be built from parsed extensions without an intermediate
  `Vec`. Impact: none on the contract; the additions are the smallest shape
  that lets the stated obligations be tested. `has_conflict_markers` also has a
  limitation that is now written down as a test rather than left to be
  rediscovered: it is not a Markdown parser, so a fenced example carrying all
  three markers is indistinguishable from a conflict. That is why the scan is
  gated on an operation actually being in progress and why `--allow-conflicted`
  exists.

- Observation: **"the document is a two-pass fixed point" is a weaker property
  than "the document is what the formatter writes", and this plan had one pass
  of slack throughout.** `idempotence_drift` checks pass1 = pass2, which a
  document satisfies as soon as one further pass leaves it alone — including a
  document that a single pass would still rewrite. Measured: format
  `git show HEAD:docs/execplans/git-option.md` with the full flag set and the
  result differs from the committed file in more than twenty regions, while
  pass1 = pass2 holds exactly. Consequence: splicing a converged region into
  this document canonicalizes its neighbours too, which is why the Stage B
  red-evidence commit is dominated by rewrapping rather than by the three edits
  it actually makes. Verified harmless before accepting it rather than after:
  the formatter's effect on this document is whitespace-only apart from table
  column padding, checked by comparing the concatenated word streams of the
  file before and after the pass — the only differences were two table
  separator rows whose runs of `-` grew to the column width, which is the
  tool's stated job. Taking the canonical form once is also what makes every
  later splice in this document a minimal diff.

- Observation: **the red transcript proves less than it appears to: name
  resolution fails before type checking, so `E0432` hides every type error
  behind it.** Four were waiting in the test files once the imports resolved,
  and none was reachable from the `EV-M1-RED` run. `ascii_text()` was
  `vec(0x20u8..=0x7e, 1..=40).prop_map(String::into_bytes)`, whose strategy
  yields `Vec<u8>` while the function takes a `String` — a `prop_map` over a
  byte range needs `fn(Vec<u8>) -> _`, and the range already *is* the byte
  vector. `splitting_is_a_faithful_inverse` moved `textual` into
  `prop_assert_eq!` and then read `textual.len()` two lines later; the count now
  comes first. `.map(Utf8PathBuf::as_str)` names an associated function that
  does not exist — `as_str` belongs to `Utf8Path`, and a path cannot be reached
  from `&Utf8PathBuf` as a function pointer — so the map is now a closure over
  the deref. And `regular(path.as_str())` over an iterator of `&str` resolves to
  the unstable `str_as_str`; passing `path` directly lets the argument coercion
  do the same job. Each fix is mechanical and none changes what a test asserts:
  the same generator, the same assertion, the same expected values. The same
  files also needed one Clippy fix under `-D warnings`
  (`redundant_closure_for_method_calls`), which is the gates doing their job
  rather than a further defect. Impact: the "red before green" claim needs
  restating precisely, and the Progress entry now does — the red state is a
  *name-resolution* failure, and it is evidence that the items under test were
  absent, not evidence that the tests around them were sound.

- Observation: **`BTreeSet`'s own order contradicts the order this plan
  specifies for `--help`.** Byte-wise, `markdown < md < mdc`, because `a` sorts
  before `d`; the default set would therefore have rendered as
  `markdown, md, mdc`, while the `Display` doc comment in the Interfaces block
  says `md, mdc, markdown` and the `--md-exts` declaration in this plan gives
  `default_values = ["md", "mdc", "markdown"]`. Both statements cannot hold
  under a plain byte-wise sort, and the tests written against the Interfaces
  block assert the documented order. Resolved by making `iter()` sort
  shortest-first and then byte-wise — the order that puts the common `md` first
  — and correcting the `iter()` doc comment, which had said only "in sorted
  order". The chosen order is total and independent of how the user spelled the
  flag, so a `--help` rendering or a diagnostic is reproducible from the set
  alone; insertion order would have made it depend on the command line. Storage
  stays the `BTreeSet` the plan specifies, so membership and deduplication are
  unchanged. Impact: one doc-comment correction, made in place.

- Observation: **a relative `TMPDIR` breaks `cargo-mutants`' own baseline**,
  because the tool's child processes run inside the scratch copy of the tree
  rather than in the directory it was invoked from. Evidence: the first
  `make mutants` run failed at `FAILED   Unmutated baseline in 0s build` with a
  `mktemp: failed to create file via template` diagnostic naming
  `target/mutants-scratch/tmp.XXXXXXXXXX` as a path it could not create, raised
  from a rustc wrapper after cargo had already been started. Impact:
  `TMPDIR=$(CURDIR)/target/mutants-scratch`, absolute, with the Makefile giving
  the reason at the point of use. Worth recording because the failure names a
  path the invoking shell can see and the failing process cannot, so it reads
  as a missing directory rather than as a bad variable.

- Observation: **`make <target> | tee <log>` exits with `tee`'s status, so a red
  gate can be recorded as a green one.** Evidence: that same failed run ended
  `make: *** [Makefile:57: mutants] Error 4` in the log, while the shell around
  it reported `0`. Impact: gate runs piped to a log now run under
  `set -o pipefail`, since the convention of teeing a log exists precisely to be
  read by whoever checks the exit status afterwards. The log itself was never
  wrong; only the status was, and it was wrong in the direction that hides a
  failure.

- Observation: **the first mutation run found three survivors, and all three were
  the same two source lines: the classification of a failed `canonicalize`.**
  A path `symlink_metadata` has already accepted can reach the second arm only
  by losing a race with the filesystem, so the guard's arms were unobservable to
  any test rather than merely untested — which is why three mutants of it
  (`true`, `false`, and an `==`/`!=` swap) outlived a suite that covers every
  other branch of that function. Impact: the classification moved to
  `unnameable`, a function of `io::ErrorKind`, tested by constructing both
  errors. Excluding the three with `exclude_re` was the cheaper route to a green
  run, and is what the rigour section above rules out: a survivor either has a
  test that kills it, or has a written reason it cannot have one — and "the
  fixture is impossible to stage" is a reason to change the seam, not to record
  an exception.

- Observation: **`cargo-mutants` does not mutate test code**, so the five
  `*_tests.rs` files that `src/select/**` matches produce no mutants at all.
  Evidence: `cargo mutants --list-files` names the five production modules and
  nothing else. Impact: the count of 40 is a count over production code, so a
  reader comparing it against module sizes should not read the test files as
  either covered or uncovered by it — they are where the killing is done, not
  where it is measured.

- Observation: **a doc comment on a `clap` field is user-facing help text, and a
  second paragraph in one changes the rendering of every flag.** Evidence: the
  first `--help` snapshot, taken while `--md-exts`'s doc comment carried a second
  paragraph explaining why it declares `default_value` rather than
  `default_values`. `clap` reads the first paragraph as `help` and the rest as
  `long_help`, and the presence of any long help switches the *whole* `--help`
  to the long layout: every flag in its own indented block, the internal
  rationale printed to users, and the snapshot 71 lines instead of 31. Resolved
  by moving the rationale to a `//` comment, which `clap` does not read.
  Impact: no behaviour changed, and nothing short of the whole-rendering
  snapshot would have caught it — a `help.contains("--md-exts")` assertion
  passes in both layouts, which is the argument for snapshotting the text.

- Observation: **`default_values` renders space-joined, so the default extension
  set read as one extension with spaces in it.** Evidence:
  `[default: md mdc markdown]` in the first snapshot, and a user copying that
  string into `--md-exts` would have had it accepted by `parse_extension` as a
  single extension named `md mdc markdown`, which matches no file. Resolved by
  one `default_value = "md,mdc,markdown"` under the same `value_delimiter`:
  identical rendering to the grammar the flag accepts, identical three
  extensions after splitting, and `ValueSource::DefaultValue` either way, so the
  post-parse check is unaffected. Impact: the Interfaces block's
  `default_values = [...]` is superseded, and
  `cli_git.rs::md_exts_replaces_the_default_set`'s unflagged case is what shows
  the default selection did not move.

- Observation: **`ExitStatus`'s own `Display` already renders the words "exit
  status", so the pinned failure message said it twice.** Evidence: the rendered
  `` `/bin/false ls-files` failed with exit status exit status: 1``. The unit
  test that existed asserted only `.contains("git ls-files")`, so the doubling
  passed every gate until EP-M2's sanitizer work added an assertion on the whole
  string. Resolved by `` `{command}` failed with {status}``, with the reason
  recorded where the message is declared. Impact: an illustration of what a
  substring assertion on a message pins — the substring, and nothing else about
  the sentence around it.

- Observation: **a mutation survivor can be a test's blind spot rather than a
  defect, and the widened oracle found two of exactly that kind.** With the
  whole suite killing mutants, the guard on an empty `stderr` in
  `GitListError::diagnostic` and the `>` against `relayable`'s cap both
  survived: the first because nothing asserted what a failure with no Git text
  to relay shows a user, so a line ending in a dangling `": "` passed every
  test, and the second because the existing test used a 4096-character flood,
  which `>` and `>=` cut identically — only a run of exactly `RELAYED_LIMIT`
  characters tells them apart. Evidence: the two `MISSED` lines in
  `EV-M2-MUTANTS`. Impact: both are killed now by tests that pin the boundary
  rather than the neighbourhood —
  `a_failure_with_no_git_text_carries_our_wording_alone` compares the diagnostic
  to `Display` exactly, and the cap has a test on either side of it. A survivor
  is therefore evidence about the tests as much as about the code, which is the
  argument for widening the oracle before believing a green run.

- Observation: **writing the ADR found a stale doc comment that four gate runs
  and two CodeRabbit passes had not.** `GitSelection::inputs` described its
  paths as "in Git's order", while `git_inputs::resolve` hands them to
  `select_files`, which sorts them byte-wise — the invariant INV-ORDER-DET
  exists to guarantee. Impact: a record that states the design in prose is a
  reading of the code from a direction no test takes, and it earns its keep
  before it is published, not after. The comment now names the sort and the two
  places the order is visible. It is not a correctness defect — the sort is what
  runs — but a reader who trusted the comment would have expected a different
  `--list-files`.

- Observation: **markdownlint's line rule applies inside a `console` fence at
  120 characters, and to a code span in prose at 80.** Evidence: the measured
  failure line recorded in the ADR — this tool's wording, the command name, the
  exit status, and Git's fatal message — is 123 characters, so it is wrapped
  inside its fence; and the non-UTF-8 notice, at 203 characters on one line of
  prose with an inline code span, is an error there too, which is why it is a
  fenced transcript rather than a quoted span. Impact: a transcript is not
  exempt from the line limit, and the two limits differ by more than half, so a
  reader writing a long measured line should reach for the fence and check its
  width. Both wrap points are marked in the ADR as the page's wrapping rather
  than the tool's, because a reader may otherwise take a wrapped line for a
  wrapped message, and the tool prints one cause on one line however long it
  is.

- Observation: **normalising one section of `docs/users-guide.md` to the wrap
  fixed point reflowed three paragraphs the change never touched.** Evidence:
  `git diff` shows the three hunks, and each is a pure re-wrap with no word
  changed. Reverted or kept was a real choice, so it was decided on evidence
  rather than on the tidiness of the diff: `make fmt`'s `mdformat-all` runs
  exactly the flags used here, so these paragraphs were going to reflow the next
  time anyone ran it, and `tests/idempotence_drift.rs` compares pass 1 against
  pass 2 rather than committed input against pass 1 — so committed documentation
  is not required to be a fixed point, and the drift gate says nothing either
  way. Kept, and the file is now a fixed point. Impact: a documentation change
  can carry reflows it did not make, and the honest thing is to say so in the
  commit message rather than to leave the diff looking smaller than the change.

- Observation: **the missing-identity lesson was learned in one fixture and not
  applied to the second.** The Surprises entry above records
  `GIT_CONFIG_GLOBAL=/dev/null` leaving the *behavioural* fixture with no
  identity, and the four `GIT_AUTHOR_*`/`GIT_COMMITTER_*` variables as the
  remedy. The *unit* fixture in `src/select/git_ls_files_tests.rs` had the same
  gap and kept it, because it was written in the same milestone and its helper
  inherited the environment rather than hardening it. Evidence: CI run
  `34661535150`, `git ["commit", "-m", "initialise"] failed with exit status:
  128: Author identity unknown`, in that file's helper on **both** jobs. Impact:
  a lesson recorded in one place does not propagate itself, and the fixture that
  most needs the hardening is the one written before the lesson was learned. The
  two helpers now harden their environments identically, and the shared shape is
  the reason the repair was mechanical once the failure was read.

- Observation: **a green local gate is evidence about the local machine, not
  about the code.** All four of the Windows defects came from the test tree
  spelling a platform fact the way this Linux box spells it — a separator, an
  `ExitStatus` rendering, an `argv[0]` suffix, a canonical path's verbatim
  marker — and each test was green here every time it was run. Evidence: CI run
  `34661535150`'s Windows job reports `124 passed; 3 failed` on the library,
  `13 passed; 1 failed` on `cli_git`, and all seven git-selection scenarios
  failing at one fixture line. Impact: the four are string and path assertions
  rather than behaviour, so the tool itself was never wrong on either platform;
  but the plan's own gates could not have caught them, and the *first* run of a
  new test suite on a second platform is doing real work rather than confirming
  local green. Where a fact is spelled by the standard library, the repair is to
  assert the part this crate words and let the platform supply the rest — which
  is also what makes the assertion survive the next platform.

- Observation: **GitHub runs no `pull_request` workflows at all while the pull
  request conflicts, so silence from CI is not agreement.** Evidence: the
  branch's newest run is `34661535150` at `573deb9` (00:24:40Z), while
  `9a83339` (00:38Z), `5f294b7` (00:41Z), and `6384543` (10:43Z) were each
  pushed with no run between them; #466 is `CONFLICTING`/`DIRTY` against
  `check-option`. The behaviour is documented rather than inferred — GitHub's
  events reference states that workflows will not run on `pull_request`
  activity if the pull request has a merge conflict, and that the conflict must
  be resolved first. The same push that carried this observation supplied its
  control: run `34689481906` is a `pull_request_target` run on that commit,
  which GitHub documents as not blocked by conflict, while no `pull_request` run
  exists beside it. Impact: EP-M3's documentation commits have never been tested
  by CI, and neither has this repair, so no CI verdict exists for either and the
  plan must not read the absence of a red run as a green one. It also means the
  rebase this plan defers is what re-opens the pipeline, which turns a tidiness
  question into the precondition for CI evidence — see the Decision log.

- Observation: **a test can be unreachable behind another test's failure, and
  the repair that hides it is the one that looks tidiest.** The linked-worktree
  assertion compared a path as `/`-spelled text and was the same latent defect
  as the canonical-path failure Windows did report; it never fired, because its
  fixture calls `git commit` before reaching it and the missing identity
  panicked first. Evidence: CI run `34661535150`'s Windows log shows that test
  panicking at the helper's own assert (`git_ls_files_tests.rs:145`) and the
  assertion below it never being reached. Impact: repairing only the five
  observed defects would have left the suite green on the identity fix alone,
  and the assertion would have failed later, on the next run that reached it
  with a Windows path — a worse place to find it. It is recorded in `Progress`
  as a judgement rather than as an observation, because that is what it is.

- Observation: **a configured merge driver can exit 0 and still corrupt the
  merge, so a rebase on a machine whose attributes name one has to run with that
  driver bypassed.** Evidence: this machine's Ansible-managed
  `~/.config/git/attributes` maps `*.rs merge=weave`, the driver itself defined
  in `/home/leynos/.gitconfig` as
  `merge.weave.driver = weave-driver %O %A %B %L %P`; the repository's own
  `.gitattributes` names no merge rule, so the branch cannot opt out through
  anything it tracks. With the driver engaged, `git rebase` reported success and
  exited 0 while producing a tree that cannot compile: `src/main_tests.rs` with
  its module doc and a `#[cfg(unix)]` block duplicated around the resolution,
  `src/main.rs` garbled outside any conflict marker, and a HEAD declaring
  `mod cli;` while importing `Cli` from `command`. Impact: "the rebase finished"
  is a claim about git's exit status, and a merge driver is part of what decides
  it, so the status is worth less than the compile. The corrupted resolution was
  discarded rather than repaired, the pre-weave state was saved aside, and the
  rebase was rerun with `core.attributesFile=/dev/null`. One further trap, which
  cost a second abort: `-c core.attributesFile=/dev/null` on the starting command
  does not survive `git rebase --skip` or `--continue`, because those are fresh
  processes that re-read configuration — the bypass has to be inherited by
  children, in the environment form
  `GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.attributesFile
  GIT_CONFIG_VALUE_0=/dev/null`, or the driver re-engages mid-rebase with no
  marker to warn that it did.

- Observation: **lines that both sides agree on are not necessarily lines that
  are right together, so a resolution is a reading of the whole region rather
  than a per-hunk choice.** Evidence: in `src/main.rs`'s `run()`, this branch's
  hunk introduced `let result = match inputs { … };` and the base's introduced
  `let result = match result { … };` — separate hunks, so git merged both
  without conflict — but the base's antecedent sat in the hunk above and this
  branch's in the hunk below, leaving the text between them, which neither side
  disputed, binding a name to nothing. Impact: an auto-merged region is not
  evidence of a coherent region, and a dangling binding shows up only when the
  hunks around it are read as one unit; the resolution rewrote the whole of
  `run()` rather than the conflicting lines alone, and `cargo check` is what
  confirmed it. The same pass caught the converse in the other direction:
  leaving `process_lines` in `command.rs` would have left `src/main.rs`
  importing `std::borrow::Cow` for nothing, an unused import that `-D warnings`
  would have failed the build over.

- Observation: **a base still under test moves while the rebase onto it is being
  validated, so "rebased onto the base" is a statement about a commit rather
  than about a branch.** Evidence: the first rebase onto `origin/check-option`
  at `64117e4` was completed, its six gates ran green, and its record was
  written; inside that window the base gained `9834fcb` (13:13:29 +0200) and its
  record commit `49a2d0a`, so the tree that had just been measured was no longer
  the tree on the base's tip. That the arrival was itself a CI-only repair — a
  traced test that fails according to which tests the harness happens to run
  alongside it, the same class as the five this branch had repaired — is the
  part worth carrying: it says the failures CI reports are a property of the
  stack rather than a debt this branch pays and is done with. Impact: the same
  instruction was applied again rather than the first rebase being pushed, and
  the record now names `49a2d0a` as the tip rebased onto while stating that a
  third rebase becomes necessary only if the base moves once more.
  **It did, and the shape of the second instance is what makes the observation
  worth keeping rather than a footnote.** `e462b97` landed at 13:22:35 +0200 and
  the second rebase's push at 11:25:22Z, three minutes apart, with the second
  rebase measured, gated, and pushed in between; so this is not one unlucky
  window but the ordinary state of a branch stacked on a branch that is still
  being worked. The two instances also differ in a way that matters: the first
  arrival was a code change that touched files this branch edits, and the second
  is 27 lines of the base's own plan document that this branch never touches.
  The rule that falls out is the one now applied — absorb the base, then
  measure what actually moved with `git diff --stat` against the superseded tip
  rather than assuming the replay changed anything — and the reason to state it
  is that the assumption would have been right here by luck and wrong the time
  before.

- Observation: **the Markdown line-length rule's exception is about the last
  whitespace-delimited word, not about whether a line could be broken, and the
  difference between the two readings is one column.** `MD013` shortens each
  line before comparing it — it replaces the trailing run of non-whitespace with
  a single character — so a line passes when its final word begins at or before
  column 80 and fails when that word begins at column 81, even if the word is a
  single em dash. Evidence: this plan's own Progress entry, reading "…in
  `src/select.rs` —", was reported as `[Expected: 80; Actual: 81]` while eight
  longer lines in the same file pass, because those carry a long code span after
  their last space. The rule is in markdownlint's `md013.mjs`, which does the
  shortening before the comparison, and the eight were re-derived under it
  rather than assumed. Impact: a hand check that asks "is there whitespace past
  column 80" answers a different question and will pass a line the gate fails;
  "where does the final word begin" is the question that matches the
  implementation, and it is the one this plan now uses.

## Decision log

Entries are pointers; the reasoning lives in the body sections named. ADR 0010
is the durable record, and EP-M3 reconciles this log into it.

- Decision: `--list-files` is answered by the first statement of
  `driver::analyse`, before the read capability is cloned, and is *not* given a
  `ReadOnlyDir` parameter of its own. Rationale: the plan asked for the mode to
  be unable to write by type, and the type is not available here — `analyse` is
  one function whose mode is a run-time value, so the capability it receives
  cannot change type with the mode. What the early return does give is that the
  listing path reaches no capability and no `Directory` call at all, so the
  reading and writing arms are unreachable rather than merely unentered; what it
  leaves to review is that the `&Dir` is still held. The alternatives were to
  split `analyse` into a read-only half and a write half (a larger change to
  #464's composition than this milestone is for) or to build the listing payload
  in `src/main.rs` (which would put the meaning of listing in two places).
  Cost: one plan sentence softened from "by type" to "by construction, with the
  capability held but unused", recorded in Table 1 and in the composition
  section rather than left as a claim a reader would have to falsify.
  Date/Author: 2026-09-12, implementation.

- Decision: `git_inputs::resolve` takes `Mode` and returns
  `GitSelection { inputs, guard }` rather than returning `Inputs` alone.
  Rationale: two facts, one reason. The guard is needed by the caller exactly
  when the mode can write, and resolving it costs a second `git` process, so a
  `--check`, `--diff`, or `--list-files` run must not pay for one it cannot use
  — hence the mode parameter. And the guard has to reach `driver::analyse`
  through `run_files`, so it must travel out of the resolver beside the inputs;
  returning it through a field of `Inputs` would have put a conflict policy
  inside the type that answers "where does the text come from". Cost: the
  composition section's pinned signature is superseded, and the resolver now
  lives in `src/git_inputs.rs` rather than in `src/main.rs` so that the choice is
  testable without a repository. Date/Author: 2026-09-12, implementation.

- Decision: the conflict guard refuses inside the `Mode::InPlace if is_changed`
  arm, not before the assessment. Rationale: the refusal is only meaningful for
  a file this run would write, and `is_changed` is what decides that — a clean
  file containing a marker (a resolved file already committed, say) has nothing
  to refuse, and refusing it would fail a run that would have changed nothing.
  It also means the guard sees `Assessment::original`, the bytes actually on
  disk, rather than a second read that could race the first. Cost: `analyse`
  gained a `ConflictGuard` parameter, which is one more argument at eight call
  sites in `driver_tests` and two in `main_tests`. Date/Author: 2026-09-12,
  implementation.

- Decision: the Git directory is resolved with
  `git rev-parse --absolute-git-dir` rather than by walking up for a `.git`
  entry. Rationale: a linked worktree and a submodule hold a `.git` *file*
  naming a directory elsewhere, and `GIT_DIR` may point anywhere at all, so a
  walk misresolves exactly the repositories a merge is most likely to be paused
  in — the case the guard exists for. It also answers for a bare repository,
  where the working tree has no `.git` entry to find. Cost: one more subprocess
  per `--in-place` run that is not `--allow-conflicted`, and a failure mode
  (`NoGitDir`) that has no analogue in a walk. Date/Author: 2026-09-12,
  implementation.

- Decision: git's own stderr is relayed beside this repository's message, as one
  scrubbed line, capped at 1024 characters — rather than dropped, or printed
  verbatim. Rationale: AX-GIT-NLS forbids asserting on git's text, so the part a
  test may assert has to be this repository's wording; but dropping git's text
  would leave a user with "git ls-files failed" and no reason. `relayable`
  replaces control characters and newlines with single spaces and non-UTF-8
  bytes with the replacement character, so a path in the repository cannot forge
  a second line of stderr or drive the terminal reading it, and the cap keeps a
  kilobyte of a repository's name from burying the message it supports. Cost:
  one screen of scrubbing logic, five cases in `git_ls_files_tests`, and a
  `diagnostic()` method whose output differs from `Display`. Date/Author:
  2026-09-12, implementation.

- Decision: REQ-GIT-010's evidence is a tracked file whose bytes are not UTF-8,
  not an unreadable file. Rationale: a file this process cannot read proves
  "reads no content" only for a user who cannot read it, and the suite may run
  as root or as the file's owner; a file that cannot be *decoded* fails every
  mode that reads it, whoever runs it. The test therefore means the same thing
  on every machine. Cost: none; the scenario in the feature file pins the
  ordinary case, and this pins the strong one. Date/Author: 2026-09-12,
  implementation.

- Decision: narrow the mutation run to the binary's own test target
  (`additional_cargo_test_args = ["--bin", "mdtablefix"]`) rather than the whole
  suite. Rationale: `cargo-mutants` checks the unmutated baseline first, and
  until EP-M2 lands `--git`, `tests/git_file_selection.rs` fails however the
  source reads — a suite that is already red would report every mutant as
  caught. Cost: the behavioural suite contributes nothing to the run yet, so the
  setting is a temporary narrowing and EP-M2 revisits it once that suite is
  green. Date/Author: 2026-09-12, implementation.

- Decision: drop the narrowing at EP-M2, and re-run the whole suite as the
  oracle, rather than leaving it until a review asked for it. Rationale: the
  narrowing existed only because the baseline was red, and the milestone that
  turns the baseline green is the earliest commit that can widen the run — a
  mutation gate that excludes the behavioural scenarios cannot support a claim
  about behaviour, and the criterion is zero survivors anywhere in
  `src/select/**`, not zero survivors among the units. Cost: every mutant now
  runs the whole suite, so 60 mutants take 5 to 6 minutes wall where 40 took 84
  seconds. Outcome: the first widened run reported two survivors the narrowed
  run could not have reached at all, both a test's blind spot rather than a
  defect; both are killed now and the run is green. See `EV-M2-MUTANTS`.
  Date/Author: 2026-09-12, implementation.

- Decision: change the seam rather than exclude the mutants that could not be
  killed. Rationale: three mutants of `fs_probe`'s failed-canonicalization match
  survived because their two arms differ only under a filesystem race, and
  `exclude_re` would have turned the run green in one line. Extracting the
  classification into a function of `io::ErrorKind` costs six lines, kills all
  three, and pins a decision the module's own comment already claimed — that
  absence is reported as absence and every other failure as unnameable. Cost:
  one production change after the milestone's review; see the Progress entry.
  Date/Author: 2026-09-12, implementation.

- Decision: keep the mutation scratch trees under `target/`, with `TMPDIR` set
  absolutely to it, rather than letting the tool default to `/tmp`. Rationale:
  `/tmp` is not a build target on this machine, and `cargo-mutants` builds a
  full dependency tree in its scratch copy. Cost: none measured; the scratch is
  inside an already-ignored directory on the filesystem the work lives on.
  Date/Author: 2026-09-12, implementation.

- Decision: accept a deliberately red `make test` on the Stage B and EP-M1
  commits, rather than parking the specification or pinning it to behaviour
  that does not exist. Rationale: the plan requires the specification to exist
  and be observed failing before the code that satisfies it, and EP-M1 requires
  the CLI to remain unchanged, so no commit between Stage B and EP-M2 can be
  green. The redness is bounded to two commits, stated in each commit message,
  and resolved by EP-M2, which is the first commit eligible for review. Cost:
  `git bisect` over these two commits lands on a failing test target by design.
  Date/Author: 2026-09-12, implementation.

- Decision: stack this work on `check-option` by rebasing onto it, rather than
  waiting for #464 to merge or re-deriving its composition locally. Rationale:
  #464 was still open with `EP-M6` unrun when implementation began, so neither
  waiting nor duplicating was acceptable. Consuming the interfaces from the
  branch that defines them keeps the plan's composition section exactly as
  written and avoids building a second ordering scheme, a second exit-status
  contract, and a second `--list-files` mode that would have to be reconciled
  the moment #464 merged. Cost: PR #466's diff is stacked, and this branch must
  be rebased again once #464 lands. Date/Author: 2026-09-12, requester.

- Decision: sequence this plan after pull request #464 and consume its
  interfaces rather than duplicating them. Rationale: #464 was already in
  implementation when the collision was found, and its commit `83e6150` adopted
  the four forward-compatibility requests this plan made.
  `driver::{Inputs, Mode, ReadOnlyDir, Assessment, ExitStatus,
  exit_status, in_argument_order}`
  now exist, so the alternative would mean building a second changed-file
  comparison, a second ordering scheme, and a second exit-status contract.
  Date/Author: 2026-09-11, requester.

- Decision: this plan's decision record is **ADR 0010**, not 0006.
  Rationale: #470 took 0006 (single-pass idempotence) and #469 took 0007
  (line-ending detection) while this plan was in review. Date/Author:
  2026-09-11, planning agent.

- Decision: cover `--git --check` and `--git --diff` with scenarios rather than
  leaving them merely parseable. Rationale: adding `git` to the `inputs` group
  makes these combinations parse for free, and `--git --check` is the
  continuous-integration gate the feature exists to enable. Shipping a
  combination that parses but is untested would be worse than either supporting
  or forbidding it. This is a judgement call the requester has not confirmed;
  if the intent is to defer it, strike the two scenarios and the transcript,
  and the rest of the plan is unaffected. Date/Author: 2026-09-11, planning
  agent.

- Decision: key deduplication on the canonicalized path, reversing the earlier
  `(st_dev, st_ino)` choice, and downgrade the invariant's severity. Rationale:
  see INV-DEDUP. Atomic replacement closed the data-loss scenario and
  simultaneously made inode-keyed deduplication wrong for hard links.
  Date/Author: 2026-09-11, planning agent.

- Decision: spawn `git ls-files -z` rather than link `git2` or `gix`.
  Rationale: equivalence by construction rather than by reimplementation; no
  Git-operating dependency; inherits every Git configuration input for free.
  libgit2 diverges from Git on nested `.gitignore` negation, so `git2` would
  make equivalence approximate. The stronger argument, which ADR 0010 should
  lead with, is that a walker-based approach such as the `ignore` crate
  **structurally cannot see force-added ignored files**, whereas `--cached`
  gets them right by construction. Cost: `git` on `PATH`, turned into an
  actionable message by REQ-GIT-007. Date/Author: 2026-09-09, planning agent,
  confirmed by the requester.

- Decision: `--git` selects tracked files only; untracked selection is opt-in
  behind `--include-untracked`. Rationale: `--others` is exactly the set of
  files Git cannot restore. A user with `$HOME` under version control who runs
  `--git --in-place --wrap` would otherwise reformat every Markdown file in
  their home directory with no undo. Making the recoverable set the default
  bounds the blast radius; the full reference-command equivalence remains
  available in one extra flag. Date/Author: 2026-09-09, requester, on a
  reviewer pre-mortem.

- Decision: narrow to Markdown extensions, defaulting to `md`, `mdc`,
  `markdown`, with `--md-exts` to override by replacement, and no effect on
  positional paths. Rationale: `git ls-files` reports every file in the
  repository, and `mdtablefix` has never filtered by extension, so an
  unfiltered run would rewrite `.rs` and `.toml` files. This is a safety
  requirement, hence CON-SAFE-001 and the bidirectional INV-EXT-SOUND/COMPLETE.
  Requiring `--git` is deliberate: silently discarding a file the user named
  would be wrong. Date/Author: 2026-09-09, requester and planning agent.

- Decision: deduplicate on file identity, not on the path string; sort the
  result. Rationale: see INV-DEDUP. The identity is free because the probe
  already fetches the metadata. Date/Author: 2026-09-09, planning agent, on a
  reviewer finding.

- Decision: `symlink_metadata` and an explicit `PathKind::Symlink`, excluded.
  Rationale: see INV-PROBE-EXCLUSIONS. CON-SAFE-001 is stated in terms of
  inodes written rather than paths selected for the same reason. Date/Author:
  2026-09-09, planning agent, on a reviewer finding.

- Decision: do not pass `--full-name`; `--git` is subtree-scoped, and the tool
  does not announce the scope. Rationale: this is the reference command's own
  behaviour, so preserving it is what equivalence means, and it lets a user
  format one subtree without extra arguments. Three reviewers argued for
  announcing the resolved scope on stderr, on the grounds that a flag named
  `--git` reads as "the repository"; the requester chose silence. Document the
  scoping prominently in the users' guide, since it is the behaviour most
  likely to surprise. Adding a `--repo-root` flag later is additive; changing
  the default would not be. Date/Author: 2026-09-09, requester.

- Decision: refuse to rewrite conflict-marked files during an in-progress Git
  operation, with `--allow-conflicted` to override. Rationale: see
  INV-CONFLICT-GUARD. Two-layer detection — repository state first, then file
  content — keeps false positives away from documents that merely discuss
  conflict markers. Date/Author: 2026-09-09, requester, on a reviewer
  pre-mortem.

- Decision: add `--list-files`.
  Rationale: two independent needs converge on it. Without `--in-place`,
  `--git` otherwise concatenates every selected file to stdout, which is an
  unlabelled blob nobody wants and the cause of the memory profile EP-M2 has to
  fix; and the behavioural scenarios need an honest oracle for "which files
  were selected", which an earlier draft faked by grepping a conflict marker
  out of that blob. It costs about ten lines. If scope must be cut, this is the
  first candidate — but the scenarios would need reworking. Date/Author:
  2026-09-09, planning agent, on reviewer findings.

- Decision: keep the selection module tree private to the binary crate; add no
  public library API. Rationale: one in-package consumer, and `AGENTS.md:268`
  forbids exporting an opaque error type from a library. Publishing nine items
  on a crates.io crate to serve one caller is how a crate arrives at 1.0 with a
  surface nobody chose. Testability is unaffected: unit and property tests live
  in `#[cfg(test)] mod tests` inside the binary, exactly as `src/main.rs`
  already does. Cost: evidence commands read `cargo test --bin mdtablefix`
  rather than `--lib`. Date/Author: 2026-09-09, planning agent, on reviewer
  findings.

- Decision: use `thiserror` domain enums, not `anyhow`, inside `src/select/`.
  Rationale: `AGENTS.md:264` mandates it and `AGENTS.md:268` forbids the
  alternative. An earlier draft banned `thiserror` under a self-issued
  constraint, which is a plan overriding a binding repository rule — and it was
  vendoring `thiserror` transitively through `rstest-bdd` anyway. `anyhow`
  remains correct in `main.rs`, the application boundary. Date/Author:
  2026-09-09, planning agent, on a reviewer finding.

- Decision: delete the `RepositoryFileSource` port; keep only `PathProbe`.
  Rationale: see Architectural boundaries. Date/Author: 2026-09-09, planning
  agent, on three concurring reviewers.

- Decision: replace hand-applied negative controls with `cargo-mutants`.
  Rationale: see the note under the Verification plan. Date/Author: 2026-09-09,
  planning agent, on a reviewer finding.

- Decision: `std::fs::symlink_metadata` in `AmbientPathProbe` is a deliberate,
  narrow exception to `AGENTS.md:232`. Rationale: classification needs the real
  path, and `cap_std` offers no ambient stat. The exception covers metadata
  only; contents still flow through the capability boundary, which is why
  CON-CAP-001 is worded as it is. Date/Author: 2026-09-09, planning agent.

- Decision: consider and reject the broader redesign in which positional
  arguments accept directories and git-awareness becomes a property of path
  expansion rather than a mode flag. Rationale: a reviewer showed this is what
  comparable tools do — `ruff`, `dprint`, `prettier`, `markdownlint-cli2` and
  `typos` all take positional paths and consult `.gitignore` — and it would
  remove the `--git`-versus-files exclusion, the `ArgGroup`, and
  AX-CLAP-GRAMMAR entirely. It is a genuinely better long-term shape. It is
  also a behaviour change to an existing argument: `mdtablefix somedir/` is an
  error today and would become a recursive rewrite. That is a different feature
  from the one requested, and bundling it would widen the blast radius of a
  change whose whole risk profile is unintended writes. Record it in ADR 0010
  as the recommended successor. Date/Author: 2026-09-09, planning agent, on a
  reviewer alternative.

- Decision: the roadmap instruction is **not applicable**.
  Rationale: see Conformance basis. Do not create a roadmap entry to tick off.
  Date/Author: 2026-09-09, requester.

- Decision: adopt none of the four patterns `check-option` added in `44c718e`,
  `565a697`, `4cfb2d6`, and `6a44a45`; record the assessment rather than the
  port. Rationale, pattern by pattern. **Capability-`Dir` fixtures**
  (`tests/document_properties.rs`) do not transfer: this branch's fixtures are
  boundary fixtures, and the one thing a capability cannot carry is the host
  path handed to a subprocess — which is exactly what that commit carves out,
  and what `git` and the built binary are given here (`tests/cli_git.rs`,
  `tests/steps/git_selection.rs`). The in-process fixtures under `src/select/`
  have no subprocess, and already take whichever shape the code under test
  takes. **`rewrite_with`'s `pub(super)`** exists so `src/io_tests.rs` can
  drive a private boundary directly; this branch has no such boundary, because
  its `#[path]` test modules are declared inside the module they test and name
  private items already — which is why nothing under `src/select/**` needed a
  visibility change. **Platform-sized closed-pipe fixtures** guard a Windows
  `CreateProcess` command-line cap of 32 767 characters; the largest fixture
  here passes six short arguments, so there is nothing to size, and the
  pattern's own first form broke the Windows job until `4cfb2d6` fixed it. A
  fixture a branch does not need is a liability rather than insurance. **A
  closed-pipe test for `--list-files`**, the third instance of the per-mode
  shape, was considered and declined on the same reasoning: that mode's payload
  goes through `run_files`' one `BufWriter` over `stdout().lock()` and `main`'s
  `is_broken_pipe` arm, the identical write path `tests/cli_check.rs` and
  `tests/cli_diff.rs` already pin, so a third copy would re-test the same
  statements rather than a new one. Recorded because the next reader will find
  those four commits on the base branch and ask whether they were considered.
  Date/Author: 2026-09-12, implementation agent, on the rebase.

- Decision: ADR 0010 quotes measured tool output, and a passage that could not
  be measured was measured before being written. Rationale: the first draft of
  the refusal passage read `mdtablefix: refusing to rewrite docs.md: ...` as one
  line, which is not what the binary prints — the refusal is an `anyhow` chain,
  so the tool's line is followed by a blank line, `Caused by:`, and an indented
  cause. The draft was plausible and wrong, which is the failure mode a
  decision record can least afford, because a reader checks a transcript against
  the tool only when it disagrees with their expectation. Cost: two fixtures
  built to measure rather than to test — a repository paused mid-merge with
  `MERGE_HEAD` and conflict markers in the file, and a `--check` run outside any
  repository — and a wrapping note under each fence, since the cause line is 123
  characters and the fence's limit is 120. Date/Author: 2026-09-12, EP-M3.

- Decision: keep the three incidental reflows in `docs/users-guide.md` rather
  than reverting them or formatting the file in a mode that avoids them.
  Rationale: `make fmt`'s `mdformat-all` runs exactly the flags used here, so
  those paragraphs reflow the next time anyone runs it and the drift would be
  rediscovered as a surprise; `tests/idempotence_drift.rs` compares pass 1
  against pass 2, so a committed document is not required to be a fixed point
  and the gate neither demands the change nor forbids it. The file is now a
  fixed point. Cost: three hunks in the diff for words the change did not
  author, named in the commit message and in Surprises & discoveries rather than
  left for a reviewer to attribute. Date/Author: 2026-09-12, EP-M3.

- Decision: `.gitattributes` is documented as a caveat rather than implemented.
  Rationale: EP-M3's acceptance names a CRLF caveat, and the risk is real but
  older than this feature — the tool has always chosen its line ending by the
  document's majority, over the whole file and including fenced code. What
  `--git` changes is the blast radius: a repository-wide run rewrites the LF
  code samples of a predominantly CRLF document to CRLF, so the content of a
  code block changes rather than its formatting. Implementing `text`/`eol`
  attributes was out of scope and would be a second source of truth beside the
  index. Cost: one known-risk bullet in ADR 0010 and one sentence in the user's
  guide, placed before the sections that use line-ending behaviour so a reader
  meets it before a repository-wide run rather than after. Date/Author:
  2026-09-12, EP-M3.

- Decision: do **not** rebase `git-option` onto `origin/check-option` now, even
  though the base has moved and the branch is in conflict. Rationale: the base
  is being actively rewritten (`530bdbd` at 10:39Z and `914e9ae` at 10:43Z
  arrived within minutes of each other, during this repair's own gate run), so a
  rebase performed now would be against a moving target and would need repeating
  before the stack merges; the base's own Windows job is red on an unused import
  at `tests/cli_check/arguments.rs:3`, so a rebase would fold a failing job into
  a branch that is otherwise explainable as red-for-its-own-reasons; and the
  conflict is not blocking anything this branch can still do — the plan's stated
  position is that the stack waits on #464's merge, at which point the base
  stops moving and one rebase settles it. The cost is real and is recorded rather
  than waved away: while the conflict stands, GitHub runs no `pull_request`
  workflows at all, so the repairs in `6384543` and EP-M3's two commits have no
  CI verdict and cannot get one until the conflict is resolved. The decision is
  therefore to hold the correctness position and pay in delayed evidence, and to
  re-open the pipeline as soon as the base settles. Date/Author: 2026-09-12,
  EP-M4 (CI repair).

- Decision: **rebase onto `origin/check-option` now**, superseding the deferral
  above, and make `src/command.rs` this branch's one command module.
  Rationale: the deferral held that the base was moving and would stop once #464
  merged, so one late rebase would settle the stack — and it recorded its cost
  honestly, that while the conflict stood GitHub ran no `pull_request` workflows
  at all, leaving the branch's newest commits without a CI verdict and with no
  way to obtain one. Both halves of that cost are now payable. The base's own
  Windows failure has been fixed upstream in `3737276`, so a rebase no longer
  folds a red job into this branch; and the conflict is itself the reason no run
  exists for `6384543` or for EP-M3's two commits, which makes resolving it the
  precondition for the evidence rather than a tidiness question. The structural
  half follows from what the base did: it extracted `Cli` and `FormatOpts` into
  `src/command.rs` together with the pipeline functions this branch's
  `src/cli.rs` never held, so keeping both modules would put two `Cli` types and
  two `FormatOpts` types in one binary. Keeping the base's module and folding the
  `--git` surface into it is the only arrangement that leaves one of each, and
  the `--git` behaviour is untouched by it — the module is the outermost adapter
  either way, and the mode it names is still passed inward unchanged. The base's
  newer patterns were adopted rather than mirrored, on the rule that a pattern
  the base introduces is the one a later reader will expect: `crate::command`
  paths, `Mode::ListFiles` in `mode_label` and `MODES`, the split driver test
  files, and the `--fences` "Normalize" wording; `Progress` lists them, and the
  metrics one is a compile requirement rather than a preference. Cost: 30 of the
  branch's 62 commits were dropped as patch-equivalent and never replayed, which
  is invisible in the resulting tree but changes every hash on the branch; the
  two transcripts that list `src/cli.rs` now describe a file the branch does not
  carry, and the plan says so beside them; and one commit was
  skipped by hand, the EP-M7 documentation merge, which is a judgement that the
  base's text is the fuller one rather than a mechanical outcome. Date/Author:
  2026-09-12, EP-M4 (rebase).

- Decision: rebase **a second time**, onto `origin/check-option`'s new tip
  `49a2d0a`, rather than pushing the rebase onto `64117e4` that had just been
  gated. Rationale: the first rebase's whole purpose was to stop conflicting
  with the base and re-open the pipeline, and a base two commits further on
  makes that claim stale at the moment it is made — the push would have
  produced a pull request measured against a tip it no longer sits on, which is
  the condition the exercise exists to clear. The cost was measured rather than
  assumed: the base's two commits touch `src/driver_report_tests.rs` and
  `src/main_tests.rs`, which this branch edits too, so a conflict was plausible;
  the replay was in fact clean, because the base edits import lines and
  attribute positions while this branch edits `ConflictGuard::unguarded()` call
  sites, and reading the merged file is what confirms both survived. The new
  base also carried a convention this branch had to be measured against rather
  than mirrored — `test_macros::traced_test` for every traced test — and the
  measurement is that this branch has none to convert, adding no traced test of
  its own. Date/Author: 2026-09-12, EP-M4 (rebase).

- Decision: rebase **a third time**, onto `origin/check-option`'s tip
  `e462b97`, rather than holding the second rebase and recording the base's
  third move as an observation. Rationale: the instruction is to rebase onto
  `origin/check-option`, and a branch one commit behind the tip it names has not
  discharged it — the same reasoning as the second rebase, applied to the same
  condition, so treating this instance differently would need a reason and there
  is none. The cost was measured before the decision rather than after: one base
  commit to absorb, 32 to replay, and `e462b97` touches exactly one file,
  `docs/execplans/check-option.md`, which this branch never touches. What the
  third rebase cost in evidence is worth stating honestly: it supersedes the
  head the in-flight run `34691011897` was launched against, so a verdict
  arrives on `70d58ab` while the branch tips at a new commit whose only
  difference from it is the base's own plan file. The alternative considered and
  rejected was to hold `70d58ab` until `34691011897` reported, then rebase and
  re-push; rejected because it makes the branch's correctness posture contingent
  on the timing of a base still being pushed to, which is the treadmill the
  second rebase already declined to join. Date/Author: 2026-09-12, EP-M4
  (rebase).

- Decision: rebase **a fourth time**, onto `origin/check-option` at `6be1a76`,
  and with it a rule for a branch stacked on a branch that is still being worked
  on rather than a case-by-case judgement. The rule: absorb the base whenever it
  moves, but measure first what the replay actually moves, because that
  measurement is what says which gates the rebase has invalidated. Only one of
  the four moved the base's prose alone — the third, whose replay touched
  nothing but `docs/execplans/check-option.md`, and it is therefore the one case
  where the Markdown gate was the only gate that had to be re-run. The other
  three each absorbed base commits that reach code, and each was followed by the
  full set: the first took the base's own refactor into this tree across 45
  files, splitting `src/cli.rs` into `src/command.rs` and giving the reporting
  tests and the check suite their own modules; the second absorbed `9834fcb`,
  the traced-callsite fix, which edits 17 files under `src/`; and this one moved
  `src/report/render_tests.rs`. The alternative considered was to stop at the
  third rebase and let later base commits sit unabsorbed until #464 merges, on
  the argument that the pull request's diff and its CI verdict are unaffected
  either way. Rejected because it is not true of the verdict in the long run —
  the base's new test file joins this branch's tree the moment it is absorbed,
  and a reader should know which run measured the tree they are looking at —
  and because the stopping condition it implies is "whenever the base happens
  to settle", which is unbounded while another session is actively pushing to
  it. The rate is measured rather than asserted: six base commits landed in the
  fifty minutes from the tip tagged before the first rebase to the fourth
  arrival, three of them inside the nine minutes from 13:13:29 to 13:22:35,
  which is a rate rather than an accident. Cost: each rebase rewrites every
  hash on the branch and so re-points the review and run verdicts at a
  superseded commit, which is a real loss of traceability mitigated only by the
  `backup/git-option-pre-*` tags and by the plan naming the commit each verdict
  describes. Date/Author: 2026-09-12, EP-M4 (rebase).

- Decision: **no seventh CodeRabbit round** over the fourth rebase, with the
  reasoning recorded rather than the omission left silent. The round would
  review a diff this branch has not changed: `git diff --stat 70d58ab HEAD`,
  over the tree round six reviewed, names three files — the base's own
  `src/report/render_tests.rs` and `docs/execplans/check-option.md`, and this
  plan's prose — so every file this branch authors is byte-identical to the tree
  round six returned zero findings on, and the only new code in the tree arrives
  from the base, where the base's own pull request is what reviews it. The
  mutation record is carried by a stronger measurement than that diffstat:
  `git rev-parse <tag>:src/select` returns
  `dd44c4b496878fa4ced57448b1dfada0ed94ee27` at every checkpoint from
  `backup/git-option-pre-check-option-rebase` (12:54) through the fourth rebase
  (13:51) and at `4de8a7d`, so the 60 mutants over the selection module describe
  the same bytes on every side of all four rebases and `make mutants` was not
  re-run. Against
  that, a round consumes one of the reviews the service rate-limits, and this
  branch's six rounds have each returned zero findings, so the expected yield of
  a seventh over an unchanged diff is a repeat of the sixth at the price of a
  wait that would fall on whoever needs a round next. The alternative considered
  and rejected was to run it anyway for symmetry with the third rebase, on the
  argument that a review of the pushed tree is cheap insurance. Rejected because
  the third rebase's round was not symmetric: that rebase cleared the conflict,
  moved the branch from a tree the pipeline had never measured to one it could,
  and its round was the first over a diff GitHub was willing to test. This
  omission is reversible by asking — if a reader wants a seventh round over
  `4de8a7d`, nothing here prevents it, and the branch's diff against its base is
  36 files, 8891 insertions, and 144 deletions. The omission was reversed the
  same day, on the requester's instruction: a review was queued through `comenq`
  as `fa732d8c`. That is the hosted service reviewing the pull request rather
  than a seventh `--agent` round, and the omission was recorded as reversible
  precisely so that asking would be a decision to take rather than a rule to
  argue with. Date/Author: 2026-09-12, EP-M4 (review).

- Decision: **mark pull request #466 ready for review while its base is
  unmerged**, on the requester's instruction, which reverses the draft posture
  the pull request has carried since it was opened. The draft was deliberate
  rather than incidental: this branch is stacked on #464, and a stack's upper
  pull request is normally held until the lower one merges, so that a reviewer
  reads a diff whose base is fixed. `gh pr view 464` gives `state OPEN` and
  `mergedAt null`, so the base has not merged and that reason has not expired —
  it is superseded by the instruction rather than satisfied, and the difference
  is worth keeping. Cost, stated here rather than discovered later: review
  comments may arrive against a base that can still move, and each move costs a
  rebase, four of which are already recorded above with the measurement of what
  each moved, so a comment anchored to a line a later rebase renumbers is
  possible. The mitigation is the practice this plan already follows: every
  rebase is recorded beside the commit each verdict describes, and the branch's
  own files are measured as unchanged across the moves rather than assumed to
  be. Date/Author: 2026-09-12, EP-M4 (review).

## Outcomes & retrospective

Completed 2026-09-12, at commit `6384543`, rebased the same day onto
`origin/check-option` — first at `64117e4`, again at `49a2d0a`, again at
`e462b97`, and once more at `6be1a76`, the base having moved four times while
this branch was being measured against it. Every milestone is
delivered: EP-M0's
grammar measurement, EP-M1's selection tree with zero surviving mutants, EP-M2's
command-line surface and end-to-end behaviour, and EP-M3's ADR 0010 and the five
component documents. Every Surprise and Decision above is reconciled into ADR
0010 or into a component document — the reconciliation is the ADR's "Decision
outcome" and "Known risks and limitations" sections, which is where the
alternatives and the residuals now live — and no deviation is left unrecorded.

**Delivered against the plan.** `--git` with its four modifiers and
`--list-files`; the selection tree private to the binary, with `PathProbe` as
its one driven port and `src/lib.rs` untouched; a conflict guard that runs only
when a write can corrupt a resolution; Git's diagnostics scrubbed to one capped
line before being relayed; five new flags on the command line with a post-parse
dependency check standing in for `requires = "git"`; seventeen behavioural
scenarios, fifteen tests pinning the command-line surface, and unit and property
tests for every selection module; a mutation run of 60 mutants with none missed
once the oracle was widened; ADR 0010; and the user's guide, architecture,
developer's guide, README, and documentation contents updated to match. Six
CodeRabbit rounds are recorded in Artefacts and notes — two at EP-M1 (the
second after the mutation widening), one at EP-M2, one at EP-M3, one for the
CI repair, and one over the rebased tree — and every one returned zero findings.
A seventh, over the fourth rebase, was deliberately not requested, and the
Decision log records the measurement behind that rather than leaving it an
omission.

**Deviation from the plan's pinned interfaces, all recorded.** The command
module is the base's `src/command.rs` rather than the `src/cli.rs` this plan
pins, because the base had performed the same extraction and given the module the
pipeline functions as well; no `src/cli.rs` exists on the branch, and the
`--git` surface is folded into the module that does. The composition
root is `src/git_inputs.rs` rather than `src/main.rs`, and `resolve` returns
`GitSelection { inputs, guard }` rather than `Inputs` alone, because the guard
must travel beside the inputs and only a writable run should pay for it.
`--md-exts` declares `default_value = "md,mdc,markdown"` rather than
`default_values = [...]`, because the latter renders space-joined and reads as
one extension with spaces in it. `--list-files` is answered by the first
statement of `driver::analyse` instead of by a type, because one function's
capability cannot change type with a run-time mode. Each has its Decision log
entry, and each supersedes a pinned signature or table row that the plan's
Interfaces section still shows; a reader comparing the two should take the
Decision log as current.

**The two closure checks this section demanded.** Neither merged feature was
reimplemented: `src/select/**` names no `replace_file`, no `LineEnding`, and no
`detect_line_ending`, and its only filesystem call is `std::fs::canonicalize`
inside the probe's identity rule. The `--git` write path routes through
`driver::write_back` — the same `Mode::InPlace` arm of `driver::analyse` that a
positional path reaches, so there is one writer rather than two.

Issue #474 was fixed by pull request #477 before this plan was implemented, so
no caveat was needed. The `--code-emphasis` two-pass residual is issue #478 and
is not this plan's work; the user's guide makes no convergence claim for that
flag, and the one sentence in it that reads as a whole-formatter guarantee
("The formatter therefore reaches its final form in one pass", in the wrapping
section) is scoped by its own paragraph to the wrapper's handling of prefixed
blocks. The residual is recorded where a reader of the design will meet it, in
ADR 0010's "Known risks and limitations", with the blast radius the new
repository-wide mode gives it.

**What the second platform found.** The first CI run on the branch — run
`34661535150` at `573deb9`, the only one the branch has — was red on both test
jobs over five defects that are all in test code, and `6384543` answers them.
Four are the test tree spelling a platform fact the way this Linux box spells
it: a canonical path compared as text, `ExitStatus`'s own rendering pinned as
this crate's wording, a fixture keyed by `to_string_lossy()`, and a `--help`
snapshot carrying `argv[0]`'s `.exe`. The fifth is older and blunter: the unit
fixture's `git` helper inherited its environment and a runner has no identity,
which the same plan had already learned and fixed in the behavioural fixture
without carrying it across. Nothing in the tool's behaviour was wrong on either
platform, and no defect was reachable by any local gate — the four local gates
were green before the run and green again after the repair, on the same machine
that could not have found any of the five. The honest summary is that the first
run of a new suite on a second platform is doing real work, and that a green
gate is a statement about the machine it ran on. A sixth change, the
linked-worktree assertion, was made on judgement rather than observation because
another test's failure had hidden it; `Progress` says so rather than presenting
it as a sixth finding.

**The CI posture, stated plainly.** No CI run existed for `6384543`, nor for
EP-M3's `9a83339` or `5f294b7`, because #466 stood in conflict with its base and
GitHub runs no `pull_request` workflows while a conflict stands — so at the time
those commits were written, their green rested on the local gate set and the
CodeRabbit rounds, and nothing else. That conflict was resolved the same day by
the rebase onto `64117e4`, then by the second rebase onto `49a2d0a` and the third
onto `e462b97` as the base moved twice more. Resolving it re-opened the pipeline,
and the re-opening was observed rather than hoped for: run `34691011897`, event
`pull_request`, on `70d58ab1`, created within seconds of the push. It concluded
`success`, all six jobs green, `atomic write contract (windows)` among them in
6m46s — so the five test-code defects the repair answers are now answered on
Linux and on Windows, and the platform the first run was red on is the platform
the repair was measured on. The one qualification the first verdict carried was
the head rather than the content: it described `70d58ab`, and the third rebase
had moved the branch one base commit on, touching
`docs/execplans/check-option.md` and no file this branch ships. That
qualification is discharged rather than left standing: the push after the third
rebase produced run `34691436697` on `70919bc`, concluded 11:41:47Z, `success`
with all six jobs green again. The plan records both runs rather than the more
convenient of the two, and states the residual precisely: recording them adds
further commits, so the head moves past `70919bc` — but only inside
`docs/execplans/git-option.md`, and a commit that changes no file the tool ships
does not put the shipped code back under test. Two later pushes produced two
more runs, `34691869670` on `d1d10d4` and `34692137192` on `4de8a7d`, both
`success` with all six jobs green, so the pattern held rather than having to be
argued. The honest form of the claim is therefore "every file this branch ships
is CI-verified on Linux and Windows", not "the newest commit has a run", which
is a different and more perishable statement.

**What a reader should take from the record.** Three things were found by
writing rather than by testing: a stale doc comment on the selection's order, a
refusal transcript that was plausible and wrong until it was measured, and a
survivor count that fell only after the oracle was widened. Documentation is a
reading of the code from a direction no test takes, which is why it found the
first; a decision record that quotes a tool must run the tool, which is why it
found the second.

## Artefacts and notes

The reference-command transcripts and the acceptance transcripts are the
primary artefacts. Add, as work proceeds: EP-M0's verbatim clap diagnostics;
the red and green output for each milestone; the `make mutants` survivor
report; the accepted `--help` snapshot; and the final gate run. Keep them short.

**EV-A-TRANSCRIPTS** — re-established 2026-09-12 on Git 2.52.0, log at
`/tmp/stage-a-mdtablefix-git-option.out`. Every transcript in "Measured
behaviour of the reference command" reproduced byte for byte: the unsorted
six-path listing, the same listing verbatim under `-z`, the subtree-scoped
`sub/`-relative listing, the C-quoted `"we ird \303\251\"q.md"` without `-z`
against the raw `303 251` bytes with it, the three-fold `c.md` staging during
an unresolved merge collapsing to one line under `--deduplicate`, and
`exit=128` with
`fatal: not a git repository (or any of the parent directories): .git` outside
a repository. The `-t` observation also holds: a tracked file deleted from the
working tree reports `H`, and only `--deleted` produces `R`. No axiom changed.

**EV-M0-GRAMMAR** — measured 2026-09-12 against the real `Cli` on clap 4.6.6
and Git 2.52.0, log at `/tmp/stage-ep-m0-mdtablefix-git-option.out`. Each
diagnostic below is verbatim; the spike that produced it was reverted.

```plaintext
$ mdtablefix --git a.md                                   [exit 2]
  error: the argument '--git' cannot be used with '[FILES]...'

$ mdtablefix --in-place                                   [exit 2]
  error: the following required arguments were not provided:
    <FILES|--git>

$ mdtablefix a.md b.md                     [exit 0, both files printed]
$ mdtablefix --in-place a.md b.md          [exit 0, both files rewritten]
  (regression cases: a multi-value positional inside an exclusive group
   still parses, and still satisfies a mode flag; both run in a directory
   holding a.md and b.md. With `--check` added, the two mode flags are
   rejected as usual, and the usage footer still reads `<FILES>...`)

$ mdtablefix --git --in-place                             [exit 0]
$ mdtablefix --git --check                                [exit 0]
$ mdtablefix --git --diff                                 [exit 0]
$ mdtablefix --git --list-files                           [exit 0]

$ mdtablefix --include-untracked                           [exit 2]
  error: --include-untracked requires --git

$ mdtablefix --allow-conflicted                            [exit 2]
  error: --allow-conflicted requires --git

$ mdtablefix --list-files                                  [exit 2]
  error: the following required arguments were not provided:
    <FILES|--git>

$ mdtablefix --list-files a.md                             [exit 2]
  (accepted by the grammar; exit 2 is a runtime "No such file" error —
   the false-accept recorded under Surprises & discoveries)

$ mdtablefix --git --check --diff                          [exit 2]
  error: the argument '--check' cannot be used with '--diff'

$ mdtablefix --git --list-files --in-place                 [exit 2]
  error: the argument '--list-files' cannot be used with '--in-place'

$ mdtablefix --git --md-exts                               [exit 2]
  error: a value is required for '--md-exts <EXT>' but none was supplied

$ mdtablefix --git --md-exts md                            [exit 0]
$ mdtablefix --git --include-untracked --list-files        [exit 0]

$ mdtablefix --md-exts md a.md                             [exit 2]
  (accepted by the grammar; needs the value_source check, see Surprises)

$ mdtablefix --md-exts md --list-files                     [exit 2]
  error: the following required arguments were not provided:
    <FILES|--git>
  Usage: mdtablefix --md-exts <EXT> --list-files <FILES|--git>
```

The exit-0 rows ran with standard input at `/dev/null`, which is what makes
"accepted" observable: a parse failure exits 2 and can never reach the
formatter, so an exit of 0 or a runtime error is proof that clap admitted the
command line. Adding `git` to the `inputs` group was the change most likely to
break the positional: the two regression rows say it did not, and the first row
says the same group closes in the other direction.

**EV-M0-BDD** — measured 2026-09-12 on the Stage B tree, with a filtered run of
one scenario, abridged to the lines that carry the evidence; the scenario's
`Given` steps are the four-file `Background`.

```plaintext
cargo test --test git_file_selection -- --exact list_the_selection_without_acting --nocapture
```

```plaintext
---- list_the_selection_without_acting stdout ----
Step failed at index 5: Then the command succeeds
  - Panic in step 'the command succeeds', function 'command_succeeds':
    assertion `left == right` failed: the command must succeed, stderr:
      error: unexpected argument '--git' found
        tip: to pass '--git' as a value, use '-- --git'
      Usage: mdtablefix [OPTIONS] [FILES]...
    left: 2
   right: 0

test list_the_selection_without_acting ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 16 filtered out
```

The index is the substance. Steps 0–3 are the `Background` — a repository
initialised, committed to four times, with a tracked, an untracked, and an
ignored file laid down — and step 4 is the `When`, which launches the real
binary. All five succeeded. Only the `Then` failed, and it failed for the
intended reason: `--git` does not exist on `Cli` yet. The feature file is
therefore discovered, its `Background` executes, the step registry binds every
step, and the failure is the missing feature rather than a broken harness.

**EV-B-RED** — measured 2026-09-12, before any production code for the
selection exists, log at `/tmp/stage-b-mdtablefix-git-option.out`.
`cargo test --test git_file_selection`:

```plaintext
test accept_extensions_with_a_leading_dot ... FAILED
test exit_successfully_when_nothing_is_selected ... FAILED
test extend_the_selection_to_untracked_files ... FAILED
test list_the_selection_without_acting ... FAILED
test never_write_through_a_symlink ... FAILED
test reformat_tracked_markdown_in_place ... FAILED
test refuse_to_rewrite_a_conflicted_file ... FAILED
test reject_an_unusable_extension ... FAILED
test reject_combining_git_with_explicit_files ... ok
test reject_list_files_without_git ... FAILED
test report_a_clean_repository ... FAILED
test report_a_clear_error_outside_a_repository ... FAILED
test report_drift_across_the_repository ... FAILED
test restrict_the_selection_to_chosen_extensions ... FAILED
test rewrite_a_conflicted_file_when_allowed ... FAILED
test scope_the_selection_to_the_current_directory ... FAILED
test skip_a_tracked_file_deleted_from_the_working_tree ... FAILED

test result: FAILED. 1 passed; 16 failed; 0 ignored; 0 measured
```

Every failure is at a `Then` or later and every one names `--git` as an unknown
argument. The single pass is `reject_combining_git_with_explicit_files`, whose
assertion the plan deliberately restricts to the exit status because the
wording is clap's; it is red for the right reason only in the sense that a
rejection is expected, and EP-M2 must re-examine it against the real
diagnostic. The earlier run of the same command read `3 passed; 14 failed`, and
why that was the worse result is under Surprises & discoveries.

**EV-M1-RED** — measured 2026-09-12, before any production code for the
selection exists, log at `/tmp/red-mdtablefix-git-option.out`.
`cargo test --bin mdtablefix --all-features select`:

```plaintext
error[E0432]: unresolved imports `super::has_conflict_markers`,
  `super::operation_in_progress`
  --> src/select/conflict_tests.rs:11:13

error[E0432]: unresolved imports `super::ExtensionFilter`,
  `super::ExtensionSpecError`, `super::InvalidCharacterKind`, `super::parse_extension`
  --> src/select/extensions_tests.rs:11:13

error[E0432]: unresolved import `super::AmbientPathProbe`
 --> src/select/fs_probe_tests.rs:5:5

error[E0432]: unresolved imports `crate::select::extensions::ExtensionFilter`,
  `crate::select::policy::PathKind`, `crate::select::policy::PathProbe`,
  `crate::select::policy::select_files`
 --> src/select/fs_probe_tests.rs:7:5

error[E0432]: unresolved imports `super::CandidateListing`, `super::GitListError`,
  `super::GitLsFiles`, `super::split_nul_delimited`
  --> src/select/git_ls_files_tests.rs:17:13

error[E0432]: unresolved imports `super::FileIdentity`, `super::PathKind`,
  `super::PathProbe`, `super::select_files`
  --> src/select/policy_tests.rs:19:13

error[E0432]: unresolved import `crate::select::extensions::ExtensionFilter`
  --> src/select/policy_tests.rs:20:5

error: could not compile `mdtablefix` (bin "mdtablefix" test) due to 7 previous errors
```

Two properties of this transcript are the evidence. Every one of the seven
errors is E0432, an unresolved import, so no test compiled and none could have
passed — the run cannot report a spurious green. And every error names the
production item it needs, in the module the plan's Interfaces block puts it in,
so the failure is the absence of the implementation rather than a mistake in
the test's own binding.

The headings are wrapped here to fit this document's code-block line limit,
where rustc prints each import list on one line; the log holds the unwrapped
text. Nothing else is altered, and the wrapping is the only reason this copy is
not byte-identical to it.

**EV-M1-DEADCODE** — measured 2026-09-12 against the real Clippy gate, log at
`/tmp/deadcode-mdtablefix-git-option.out`. Each arm runs
`cargo clippy --bin mdtablefix --all-features -- -D warnings` on a tree
carrying one scratch `pub fn scratch()` in `src/select.rs`. Only the lines that
carry the evidence are shown.

```plaintext
A. plain `mod select;`

  error: function `scratch` is never used
    --> src/select.rs:22:8
     |
  22 | pub fn scratch() {}
     |        ^^^^^^^
     = note: `-D dead-code` implied by `-D warnings`

  error: could not compile `mdtablefix` (bin "mdtablefix") due to 1 previous error

B. #[cfg_attr(not(test), expect(dead_code, reason = "lands ahead of its wiring in EP-M2"))]
   mod select;

  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s

C. as B, with `select::scratch()` called from `main`

  error: this lint expectation is unfulfilled
    --> src/main.rs:36:12
     |
  36 |     expect(dead_code, reason = "lands ahead of its wiring in EP-M2")
     |            ^^^^^^^^^
     = note: lands ahead of its wiring in EP-M2
     = note: `-D unfulfilled-lint-expectations` implied by `-D warnings`

  error: could not compile `mdtablefix` (bin "mdtablefix") due to 1 previous error
```

Arm A is the problem, arm B is the remedy, and arm C is why the remedy cannot
rot: the moment `main` uses the module, the expectation is unfulfilled and the
build fails, so EP-M2's removal of the attribute is enforced by the compiler
rather than by review. The measurement was taken on the non-test build alone,
which is where the attribute applies; the test build is covered by the module's
own tests, which reference every production item. The scratch item was reverted
by restoring `src/main.rs` and `src/select.rs` from copies taken beforehand —
`git diff` is configured to render side by side here and its output is not an
applicable patch.

**EV-M1-SELECT** — measured 2026-09-12, after the five modules were
implemented, log at `/tmp/test-select-mdtablefix-git-option.out`. Abridged at
the two points marked, and nowhere else. The command is a filter on the
binary's test target, so it runs the selection tests and nothing else:

```plaintext
RUSTFLAGS="-D warnings" cargo test --bin mdtablefix --all-features select
```

```plaintext
   Compiling mdtablefix v0.6.0 (/…/mdtablefix)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.93s
     Running unittests src/main.rs (target/debug/deps/mdtablefix-e8d034ec44cfb1db)

running 69 tests
test select::conflict::tests::a_document_about_conflict_markers_is_not_a_conflicted_one ... ok
test select::conflict::tests::all_three_markers_are_required_at_the_start_of_a_line::case_1 ... ok
(… 65 more `ok` lines …)

test result: ok. 69 passed; 0 failed; 0 ignored; 0 measured; 46 filtered out; finished in 0.10s
```

Counted per module, so that no obligation rests on a single module's suite:
`conflict` 16, `extensions` 27, `fs_probe` 7, `git_ls_files` 10, `policy` 9. The
`RUSTFLAGS` setting is the one `make test` uses, and it matters here for the
reason it matters everywhere else: a warning is an error in this build too. The
`expect(dead_code, …)` attribute is absent from this build, because `cfg(test)`
is set for a test target, so what keeps the *non-test* build clean is measured
separately, in `EV-M1-DEADCODE`.

**EV-M1-CR** — CodeRabbit review, run 2026-09-12 through `scrutineer` against
the local branch, log at `/tmp/coderabbit-mdtablefix-git-option.out`. The base
is the branch this one is stacked on, so the review is scoped to this branch's
work rather than to pull request #464's:

```plaintext
coderabbit review --agent --base check-option
```

```plaintext
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":["Cargo.lock","Cargo.toml", …]}
```

Nineteen files were reviewed: `src/select.rs` and the five `src/select/`
modules with their five test files, `src/cli.rs`, `src/main.rs`, `Cargo.toml`,
`Cargo.lock`, the `git_file_selection` feature with its step definitions and
its integration test, and this plan. The review reported **zero findings**. The
status line is quoted with its file list elided; the log holds all nineteen
names. This is a *local* review, not a pull-request review: it diffs the
working tree against a base commit, so it reviewed the rebased work that is not
yet pushed, which a pull-request review could not have reached while
`origin/git-option` still points at the pre-rebase tip.

**EV-M1-MUTANTS** — measured 2026-09-12, log at
`/tmp/mutants-mdtablefix-git-option.out`. Run with the target as committed, so
the command line is the whole of what a reader has to reproduce:

```plaintext
TMPDIR=/…/target/mutants-scratch cargo mutants -j 3
```

```plaintext
Found 40 mutants to test
ok       Unmutated baseline in 18s build + 0s test
 INFO Auto-set test timeout to 20s
40 mutants tested in 84s: 34 caught, 6 unviable
```

An empty `missed.txt` is the whole of the acceptance criterion: no mutant
anywhere in `src/select/**` survives the tests. The six unviable mutants are
one cause and not six — each substitutes `Default::default()` for a return type
that deliberately has no `Default`: `PathKind` (twice), `FileIdentity`,
`GitLsFiles`, and `InvalidCharacterKind`. A mutant that does not build cannot
be caught, and the tool counts it apart from the survivors rather than among
them. The baseline line is what makes the rest legible: the unmutated tree is
built and tested first, so a mutant reported caught was caught by a suite that
had already passed on its own.

The run before the classifier extraction, same command:

```plaintext
Found 41 mutants to test
MISSED   src/select/fs_probe.rs:49:27: replace match guard error.kind() == ErrorKind::NotFound with true
MISSED   src/select/fs_probe.rs:49:27: replace match guard error.kind() == ErrorKind::NotFound with false
MISSED   src/select/fs_probe.rs:49:40: replace == with != in <impl PathProbe for AmbientPathProbe>::probe
41 mutants tested in 87s: 3 missed, 33 caught, 5 unviable
```

Those three lines are cut at the `with … in <impl …>` boundary and before the
per-mutant timings, to stay inside the 120 columns markdownlint allows a code
block; the log holds them whole. All three are the same two source lines, which
is why one change kills all three.

**EV-M1-CR-2** — CodeRabbit review of the mutation work, run 2026-09-12 through
`scrutineer`, log at `/tmp/coderabbit-mdtablefix-git-option.out`:

```plaintext
coderabbit review --agent --base check-option
```

```plaintext
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":[…21 files…]}
```

Twenty-one files were reviewed — the nineteen of `EV-M1-CR`, plus `Makefile` and
`.cargo/mutants.toml` — and the review reported zero findings. Two reviews for
one milestone is the honest count rather than a repetition: the mutation
tooling landed after the first, and it is the tooling that found the three
survivors, so a review of the implementation alone would have passed a
milestone whose acceptance criterion was not yet met.

**EV-M2-CLI** — measured 2026-09-12, logs at
`/tmp/test-cli-git-mdtablefix-git-option.out` and
`/tmp/test-grown-mdtablefix-git-option.out`. The acceptance command, on the tree
this milestone lands:

```plaintext
cargo test --test cli_git --test git_file_selection --bin mdtablefix
```

```plaintext
test result: ok. 138 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The lines are in the order the command reports them, which is not the order of
its arguments. The binary's count is 138 rather than the 136 this milestone
first measured: the two tests that kill the widened mutation run's survivors
were added after it, and the run was repeated. `EV-M2-MUTANTS` is that pair.

The second run is that command *without* `INSTA_UPDATE`, which is what makes the
`--help` snapshot **accepted** rather than merely written: the first run created
`tests/snapshots/cli_git_help.snap`, and the second compared against it and
passed. The snapshot holds the whole rendering, 31 lines of it — see Surprises &
discoveries for the two defects it caught, neither of which an assertion of the
form `help.contains("--md-exts")` would have.

The five transcripts under "Validation and acceptance" were re-measured on
`target/debug/mdtablefix` at this commit, against a scratch repository built by
the commands shown there. Every exit status quoted in them is the measured one,
including the `--check` transcript's exit 1 and the empty-selection case's exit
0.

**EV-M2-MUTANTS** — measured 2026-09-12, log at
`/tmp/mutants-mdtablefix-git-option.out`. This is EP-M1's run repeated with the
widening in place: the whole suite is the oracle, so the behavioural scenarios
that were red through EP-M1 kill mutants now rather than being excluded from the
run. The Makefile is what sets the scratch directory, so the command to
reproduce it is `make mutants`, which runs:

```plaintext
TMPDIR=$HOME/.cache/mdtablefix/mutants/<worktree> cargo mutants -j 3
```

```plaintext
Found 60 mutants to test
ok       Unmutated baseline in 21s build + 87s test
 INFO Auto-set test timeout to 440s
60 mutants tested in 5m: 53 caught, 7 unviable
```

An empty `missed.txt` is again the whole of the acceptance criterion, and it is
met: exit status 0, 53 caught, 7 unviable, none missed. The count is 60 rather
than EP-M1's 40 because this milestone added production code inside
`src/select/**` — the Git-directory query, the diagnostic relay and its cap, the
conflict guard — and `cargo-mutants` mutates production code only.

The run before the two tests below, same command:

```plaintext
Found 60 mutants to test
ok       Unmutated baseline in 18s build + 76s test
 INFO Auto-set test timeout to 384s
MISSED   src/select/git_ls_files.rs:213:44: replace match guard !stderr.is_empty()
MISSED   src/select/git_ls_files.rs:255:33: replace > with >= in relayable
60 mutants tested in 6m: 2 missed, 51 caught, 7 unviable
```

Those two lines are cut at the mutator's own description and before the
per-mutant timings, to stay inside the 120 columns markdownlint allows a code
block; the log holds them whole. Both survivors were correct code that no test
observed rather than code that was wrong — see Surprises & discoveries, and note
that no gate short of this one could have said so.

The seven unviable mutants are EP-M1's six with one more of a new type: each
substitutes `Default::default()` for a return type that deliberately has no
`Default` — `ConflictGuard::unguarded`, `invalid_character`,
`AmbientPathProbe::probe`, `unnameable`, `GitLsFiles::with_program`,
`GitLsFiles::run`, and `FileIdentity::from_canonical_path`.

What the widening costs is legible in the baseline line: 87 seconds of test
where the narrowed run reported 0, the build time being much the same. Each
mutant pays that, so 60 mutants take 5 to 6 minutes wall at `-j 3` instead of
84 seconds. That is the trade the milestone exists to make: a suite that
includes the scenarios is what lets "0 missed" mean the behaviour is pinned and
not merely the units.

**EV-M2-CR** — CodeRabbit review of this milestone, run 2026-09-12 through
`scrutineer` against the local branch, log at
`/tmp/coderabbit-mdtablefix-git-option.out`. The base is again the branch this
one is stacked on, so the review is scoped to this branch's work rather than to
pull request #464's:

```plaintext
coderabbit review --agent --base check-option
```

```plaintext
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":[".cargo/mutants.toml", …]}
```

Twenty-seven files were reviewed, which is the whole of this branch's diff
against `check-option`: the five selection modules with their test files,
`src/select.rs`, `src/cli.rs`, `src/git_inputs.rs`, `src/main.rs` and
`src/driver.rs` with their unit tests, `tests/cli_git.rs` and its snapshot, the
feature file with its step definitions and integration test, `Cargo.toml`,
`Cargo.lock`, the `Makefile`, `.cargo/mutants.toml`, and this plan. The review
reported **zero findings**. The status line is quoted with its file list elided,
as the earlier rounds quote theirs; the log holds all twenty-seven names. No
rate, seat, or quota limit appears anywhere in the round's output, so no wait
was needed.

**EV-M3-DOCS** — the documentation milestone's gates, run 2026-09-12 through
`scrutineer` over the uncommitted EP-M3 tree, one log per gate under
`/tmp/<gate>-git-option.out`. Six gates, run sequentially:

```plaintext
make check-fmt     exit 0    cargo fmt --all -- --check
make typecheck     exit 0    cargo check --all-targets --all-features
make lint          exit 0    cargo clippy -- -D warnings
make test          exit 0    1988 passed, 0 failed, 20 ignored
make markdownlint  exit 2    one MD038 in docs/execplans/git-option.md
make nixie         exit 0    all diagrams validated successfully
```

The Markdown gate failed on the first run and is the reason this milestone's
evidence is not a single green run. `MD038/no-space-in-code`, at
`docs/execplans/git-option.md:2495`, was a code span split across two source
lines whose content therefore began with an escaped backtick and a space — a
span written in the Surprises entry *about* line-length rules, which is a
small lesson of its own. The entry now describes the measured line instead of
quoting it inline, and the gate was re-run alone over the whole corpus:
**36 files, 0 errors**. The other five gates were not re-run, because the only
change since they passed is prose inside one Markdown file; their logs remain
canonical for the committed tree.

`cargo test --test idempotence_drift` was run separately, before the gates, and
reports **2 passed** — `repository_documents_do_not_drift_on_a_second_pass` and
`repository_fixtures_do_not_drift_under_headings` — so the new ADR, the new
architecture section, and the edits to `docs/**` are all wrap fixed points.

**EV-M3-CR** — CodeRabbit review of this milestone, run 2026-09-12 through
`scrutineer` against the local branch at `9a83339`, log at
`/tmp/coderabbit-git-option.out`. The base is the branch this one is stacked
on, so the review is scoped to this branch's work rather than to the work of
pull request 464:

```plaintext
coderabbit review --agent --base check-option
```

```plaintext
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":[".cargo/mutants.toml", …]}
```

Thirty-three files were reviewed and the review reported **zero findings**, so
no concern was raised about the new ADR, the four edited documents, or the
figure renumbering. `"reviewType":"all"` in the round's context line means the
file list is the review's own scope rather than this branch's diff; the branch's
diff against `check-option` at this commit is the eight files of `9a83339`.
No rate, seat, or quota limit appears anywhere in the round's output, so no wait
was needed.

**EV-CI-1** — the branch's first CI run, `34661535150` at `573deb9`, read
2026-09-12 from `gh run view 34661535150 --log-failed`, kept at
`/tmp/ci-34661535150-failed.log`. Two of its six jobs failed and four passed:

```plaintext
build-test                              failure   Test and Measure Coverage
atomic write contract (windows)         failure   Test the whole suite
binstall packaging (x86_64-unknown-linux-gnu)  success
binstall packaging (aarch64-apple-darwin)      success
binstall packaging (x86_64-apple-darwin)       success
binstall packaging (x86_64-pc-windows-msvc)    success
```

The Linux job reports one failure, and the Windows job four, every one of them
a test rather than the crate. Quoted as the logs print them, with the Windows
separator left as it arrived so the defect is legible in the evidence:

```plaintext
build-test:  git ["commit", "-m", "initialise"] failed with exit status: 128: Author identity unknown
windows:     FileIdentity("\\\\?\\C:\\Users\\runneradmin\\AppData\\Local\\Temp\\.tmpV8mRZH\\docs\\guide.md")
windows:     unexpected message: `git rev-parse` failed with exit code: 128
windows:     test help_documents_the_git_flags ... FAILED        (13 passed; 1 failed)
windows:     7 git-selection scenarios FAILED at tests\steps\git_selection.rs:198
```

The Windows library run's own tally is `124 passed; 3 failed; 0 ignored`, and
the panics are at `src\select\fs_probe_tests.rs:45`,
`src\select\git_ls_files_tests.rs:316`, and `src\select\git_ls_files_tests.rs:145`
— the last of which is the helper's assert rather than the test body, which is
what put the sixth, unreached assertion out of the log's reach. The four
packaging jobs never compile a test target, which is why they pass on both
platforms while the suites fail on one.

**EV-M4-GATES** — the repair's gates, run 2026-09-12 through `scrutineer` over
the committed tree at `6384543`, one log per gate under
`/tmp/gate-rerun-<gate>-git-option.out`, strictly sequentially:

```plaintext
make check-fmt     exit 0    cargo fmt --all -- --check
make typecheck     exit 0    cargo check --all-targets --all-features
make lint          exit 0    cargo clippy --all-targets --all-features -- -D warnings
make test          exit 0    1988 passed, 0 failed, 20 ignored
```

All four passed on the first run, with no retry. The change is test code in four
files, so the documentation gates — `make markdownlint` and `make nixie` — are
not in this set; they belong to the plan edit that follows it and are recorded
with it. `make lint` clean under `-D warnings` is what closes the two
`needless_borrows_for_generic_args` findings that a first attempt at the repair
introduced, and `make check-fmt` clean is what closes the rustfmt layout it
also wanted. The 1988/0/20 tally is identical to `EV-M3-DOCS`'s, which is the
expected result: the repair changes assertions, not the number of tests.

**EV-M4-CI-SILENCE** — measured 2026-09-12 from `gh run list --branch git-option`,
read alongside GitHub's events reference. The branch's newest run is
`34661535150` at `573deb9`, created 00:24:40Z; the three commits pushed after it
have no run between them:

```plaintext
573deb9   run 34661535150   failure    2026-09-12T00:24:40Z
9a83339   —                 no run     committed 00:38:40Z
5f294b7   —                 no run     committed 00:41:44Z
6384543   —                 no run     committed 10:43:16Z
```

The cause is not flakiness in the trigger. Pushes triggered promptly up to and
including `573deb9`, whose run was created six seconds after the commit; then
nothing. `gh pr view 466` reports `"mergeable":"CONFLICTING"` and
`"mergeStateStatus":"DIRTY"`, and GitHub's events reference states that
workflows will not run on `pull_request` activity if the pull request has a
merge conflict, and that the conflict must be resolved first. The inference is
that the branch went conflicting during the rebase whose pushes `8f75fdd`,
`56b68de`, and `573deb9` were — `8f75fdd` is no longer an ancestor, which is
what a force-pushed rebase leaves behind — and that every push since has been
silently untested.

The control this entry first lacked arrived with the push that carried it.
Pushing `4c8a872` produced run `34689481906`, a **`pull_request_target`** run on
that very commit, created 10:50:08Z — and no `pull_request` run at all:

```plaintext
34689481906  4c8a872cd  pull_request_target  completed  skipped  2026-09-12T10:50:08Z
             —           pull_request         —          —        (none)
```

One push, one commit, one conflicting pull request, two triggers: the one
GitHub documents as not blocked by conflict fires, and the one it documents as
blocked does not. The skipped conclusion is the workflow's own condition
declining the run, not a failure to trigger, which is why the run record exists
to be counted. That is the same push the prose above was written about, so the
evidence and the claim are of the same date and the same head.

One limit remains, stated rather than papered over: the mergeable state is a
present-tense query with no history behind it, so the run list is what dates the
silence and the documented rule explains it. The moment the branch went
conflicting is not itself recorded anywhere this session can read. What follows
for the plan is unaffected either way: no run will appear for any of these
commits until the conflict is resolved, so the missing red run must not be read
as a green one.

**EV-M4-CR** — CodeRabbit review of the CI repair and the plan reconciliation,
run 2026-09-12 through `scrutineer` against the local branch at `0ee306b`, log
at `/tmp/coderabbit-git-option.out` — the previous round's log having been moved
aside to `/tmp/coderabbit-git-option-prev-round.out` before the intended
overwrite. The base is the branch this one is stacked on, so the review is
scoped to this branch's work rather than to pull request #464's:

```plaintext
coderabbit review --agent --base check-option
```

```plaintext
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":[".cargo/mutants.toml","CHANGELOG.md", …]}
```

Ninety-five files were reviewed and the review reported **zero findings**. The
round's context line gives `"reviewType":"all"` with `currentBranch` `git-option`
and `baseBranch` `check-option`, and the reviewed set matches
`git diff --name-only origin/check-option...HEAD` exactly at 95 paths, so the
scope is verifiable rather than assumed.

That 95 is a wider scope than the four earlier rounds, and the difference is
worth a sentence because it changes what the zero means. `EP-M3-CR` reviewed 33
files at `9a83339`; this round reviews the whole span from the merge-base
`408c76a`, which includes the commits this branch and `check-option` both carry
— `git cherry origin/check-option HEAD` classifies 31 as this branch's own and
the rest as patch-equivalent upstream. So roughly two thirds of the 95 are the
base's own work arriving under this review's eye rather than this branch's, and
the zero findings cover that traffic too. It is a stronger result than the
earlier rounds rather than a differently-measured one, and it is also a reminder
that the review's file count tracks the merge-base rather than the branch, so a
count that grows between rounds is not evidence of new work here.

No rate, seat, quota, or deferral condition appears in the round's output;
`coderabbit auth status` reports the Team plan, an assigned seat rather than a
waiting room, and the CLI exited 0. The tool did not fall back to a
pull-request-level review, which matters on a draft whose head conflicts with
its base — `EX-M4-CR-SCOPE` records why that distinction is checked rather than
assumed.

**EX-M4-CR-SCOPE** — the check that the CodeRabbit round above reviewed this
branch's diff rather than deferring to the pull request, kept as evidence
because a deferral and a clean review are both "no findings" to a reader who
does not look. Three conditions were verified: the run's own context line names
`git-option` as `currentBranch` and `check-option` as `baseBranch`; the reviewed
file list equals `git diff --name-only origin/check-option...HEAD` path for
path, 95 in each; and `coderabbit auth status` reports an assigned seat rather
than a waiting-room entry. The distinction is not academic here: the earlier
`--base check-option` rounds on this same branch printed a 33-file list, so a
file count that changed between rounds was a real signal to chase down rather
than noise to ignore. It resolved to the merge-base, as described above, and not
to a scope regression.

**EX-REBASE-STRUCTURE** — why the rebase's collision between `src/cli.rs` and
`src/command.rs` was resolved by keeping the base's module. The two are one
refactor performed twice: each declares `Cli` and `FormatOpts`, extracted from
`src/main.rs`, and the base's `src/command.rs` additionally holds
`process_lines`, `format_lines`, and `formatting_closure`, which this branch's
extraction never moved — this branch's `src/main.rs` still calls them, and after
the merge it reaches them through `command`. The evidence for the collision is
in the rebase's own conflict labels, which name `HEAD:src/command.rs` against
`parent:src/cli.rs` on the same hunks, and in the import list, which would
otherwise hold two `FormatOpts` types. It is kept as a record because the choice
looks reversible and is not: restoring `src/cli.rs` would mean moving the three
pipeline functions back out of a module the base's `src/main.rs` already imports
them from, which is a change to the base's design made for no gain to this
feature. A reader meeting the `EV-M1-CR` or `EV-M2-CR` transcript, each of which
lists `src/cli.rs` among its reviewed files, should read it as dated: it
records a round run before the rebase, and the file is not on the branch now.

**EX-REBASE-SECOND** — the second rebase onto `origin/check-option`, and the
command form that performs any rebase on this machine at all. The base's tip was
`49a2d0a`, the branch's pre-rebase tip was `5cb8425`, which
`backup/git-option-post-rebase-64117e4` tags locally, and the replay left the
branch 31 commits past the base, at `eda86ca`. The command is worth copying
rather than reconstructing, because `core.attributesFile` has to be redirected
for the rebase *and its children* — a `-c` flag is re-read away by every
`--continue` and `--skip`, and the weave driver then re-engages with no marker
to say so:

```console
GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.attributesFile \
  GIT_CONFIG_VALUE_0=/dev/null git rebase origin/check-option
```

The replay was clean, and that is a fact about the two sides' hunks rather than
about luck: the base's `9834fcb` edits the import line and the attribute
positions in `src/driver_report_tests.rs` and `src/main_tests.rs`, while this
branch's edits are the nine `ConflictGuard::unguarded()` call sites in the same
two files, so the hunks do not overlap. Both survive at `eda86ca` — the wrapper
import at `src/driver_report_tests.rs:10`, the attribute at line 131, and the
guard call sites six and three.

**EX-REBASE-THIRD** — the third rebase onto `origin/check-option`, onto
`e462b97`, and the measurement that decides whether a rebase like it needs any
gate run of its own. The base's one new commit is
`docs/execplans/check-option.md` and nothing else, so the whole question is what
moved across the replay:

```console
git diff --stat 70d58ab HEAD -- src tests Cargo.toml Cargo.lock Makefile
```

That prints nothing, and the unrestricted form of the same command names one
file and one file only:

```plaintext
 docs/execplans/check-option.md | 27 +++++++++++++++++++--------
 1 file changed, 19 insertions(+), 8 deletions(-)
```

So the third rebase is the second rebase's counterpart in kind but not in
consequence: there, the base changed code this branch also edits and the merged
file had to be read to know both sides survived; here, no file under `src/`,
`tests/`, or the manifests differs at all, so the deterministic gates and the
mutation record describe the same bytes before and after. That is the reason the
gates were re-run over the frozen tree anyway — the instruction asks for them
over the rebase, and "the measurement says nothing changed" is a claim to verify
rather than a licence to skip. The gates' verdict is recorded under `Progress`,
and the artefact worth keeping is the rule: absorb the base, then ask `git diff`
what actually moved, because the answer has been "nothing but the base's own
prose" once and "two test files this branch edits" once.

**EV-REBASE-CR** — CodeRabbit round six, over the rebased tree, run 2026-09-12
through `scrutineer` with the log at
`/tmp/coderabbit-mdtablefix-git-option-rebase.out`:

```plaintext
coderabbit review --agent --base check-option
```

```plaintext
{"type":"review_context","reviewType":"all","currentBranch":"git-option","baseBranch":"check-option"}
{"type":"complete","status":"review_completed","findings":0,"reviewedFiles":[".cargo/mutants.toml", …]}
```

Thirty-six files were reviewed and the round reported **zero findings**, with
the reviewed set equal to `git diff --name-only origin/check-option...HEAD` path
for path at 36 each, so the scope is checked rather than assumed — the same
check `EX-M4-CR-SCOPE` describes, applied to a round whose count is much lower
than the previous one's 95 for the reason recorded there, that the count tracks
the merge-base rather than the branch. That drop is itself worth one sentence,
since the earlier entry warns that a changing count is a signal: the base
landed the commits both branches had been carrying separately, so the set this
review sees is this branch's own work alone. No rate, seat, quota, or deferral
condition appears in the output.

**One qualifier, kept because it is the kind of thing a clean result is tempted
to drop.** The round took 45 seconds, which is fast for a full review, so the
possibility of a service-side cached result was raised against it. What argues
against that reading is the shape of the transcript rather than its speed: the
run emitted the whole phase sequence — `connecting_to_review_service`,
`setting_up`, `preparing_sandbox`, `summarizing`, `tools_completed`,
`reviewing` — and returned a concrete 36-file set that matches the diff exactly,
where a review that had short-circuited or deferred to the pull request would
present as an empty or absent set, which is the signature `EX-M4-CR-SCOPE` exists
to catch. What the client log cannot exclude is a service-side cache, and that is
stated rather than argued away: excluding it would take a fresh re-run rather
than a better reading of this one. The zero stands as recorded, with that limit
named.

## Revision note

Revised 2026-09-12, eighth pass, after a fourth rebase onto
`origin/check-option`, whose tip had moved to `6be1a76`, and after the full gate
set was re-run because this rebase moved more than prose.

What changed. The base gained three commits in this window — `80f80d1`, which
hardens a writer-error test in `src/report/render_tests.rs`, and its two plan
records `69bc8ea` and `6be1a76` — so the same instruction was applied again,
making four rebases in one day. The rule this pass records is the one the three
earlier rebases had been deciding case by case: absorb the base whenever it
moves, but measure what the replay actually moved before deciding which gates
the rebase invalidated. The measurement is `git diff --stat d1d10d4 HEAD`, which
names the base's plan document (+71) and `src/report/render_tests.rs` (+26/-5)
and nothing else. Where the third rebase left the tree byte-identical and needed
only the Markdown gate, this one does not, so all six gates were re-run over the
frozen tree: `make check-fmt`, `make typecheck`, `make lint`,
`make markdownlint` (36 files, 0 errors), `make nixie`, and `make test` — 2014
passed, 0 failed, 20 ignored, across 48 suites.

Two figures in this pass's first draft were corrected against measurement rather
than written down as they stood, and they are recorded because the same mistake
is easy to make again. The draft claimed that three of the four rebases had
moved only the base's prose; `git diff --stat` over the tips on either side of
each says otherwise — the first moved 45 files, the second absorbed a commit
editing 17 files under `src/`, and only the third moved prose alone. It also
gave the interval from the first rebase to the third base arrival as four and a
quarter hours; the base's own commit dates give fifty minutes to the fourth
arrival, with six moves in that window and three of them inside nine minutes.
The lesson is the plan's own rule applied to itself: a figure that can be
measured against the repository should be measured before it is recorded, and a
long session's memory is exactly where an unmeasured figure gets in.

Recorded with the rebase is the push it produced: run `34692137192` on
`4de8a7d`, `pull_request`, created 11:51:41Z and concluded `success` at
11:58:28Z with all six jobs green, preceded by `34691869670` on `d1d10d4`, also
`success` — four green runs since the branch's conflict was cleared, so the
Status header's "both CI runs" becomes "all four". One omission is recorded
rather than left silent: no seventh CodeRabbit round was requested over
`4de8a7d`, because `git diff --stat 70d58ab HEAD` names only the base's own test
file and plan record plus this plan's prose, so every file this branch authors
is byte-identical to the tree round six cleared. The Decision log carries that
reasoning, and the omission can be reversed by asking for the round.

Revised 2026-09-12, seventh pass, after a third rebase onto
`origin/check-option`, whose tip had moved to `e462b97` while the second
rebase's push was landing, and after the CodeRabbit round over the rebased tree
returned zero findings.

What changed. The base gained one further commit, `e462b97`, which records its
green Windows job; it touches `docs/execplans/check-option.md` and nothing else,
so the branch was rebased again and the third rebase is recorded with the
measurement that says what moved — `git diff --stat 70d58ab HEAD` names that one
file, and the same command restricted to `src`, `tests`, and the manifests
prints nothing, which is why the gates' verdict and the mutation record carry
across unchanged while the gates were re-run over the frozen tree all the same.
`EX-REBASE-THIRD` holds the command and the rule, `EV-REBASE-CR` the round's
transcript and its scope check, and a Decision-log entry states why the third
rebase was performed rather than the second held. What this pass adds beyond the
rebase is the end of the conflict's silence and of the wait it caused: the push
produced run `34691011897` on `pull_request` within seconds, which is the
pipeline re-opening as the conflict hypothesis predicted, and that run concluded
`success` with all six jobs green — so the plan's longest-standing open item, a
CI verdict for the repaired test code, is closed, with
`atomic write contract (windows)` among the jobs that passed. The verdict's own
head is named where it is recorded, `70d58ab`, because the third rebase moved the
branch one base commit past it while the run was in flight; the difference is the
base's own plan document, and the run the push produced is its successor — which
also concluded `success`, on the pushed tip, so the qualification about the head
is discharged rather than carried.

Revised 2026-09-12, sixth pass, after a second rebase onto `origin/check-option`,
whose tip had moved to `49a2d0a` while the first rebase's gates were running.

What changed. The base gained two commits inside that window — `9834fcb`, which
heals the `tracing` callsite interest cache in every traced test, and `49a2d0a`,
which records it — so the tip the first rebase had been measured against was no
longer the base's tip, and the same instruction was applied again. The replay
was clean, leaving thirty-one commits past the base at `eda86ca`, and why it was
clean is recorded rather than assumed: the base's hunks and this branch's sit in
different regions of the two test files that both of them edit. The base's new
convention, `test_macros::traced_test` in place of `tracing_test::traced_test`
for every traced test, is recorded in `Progress` as assessed rather than adopted,
with the measurement that decides it: this branch adds no traced test to convert,
and every `tracing_test::` mention left under `src/` is the wrapper's own
explanatory comment. A Surprises entry states the general observation and a
Decision-log entry states why the second rebase was performed rather than the
first pushed.

What this pass still does not do is claim a CI verdict, for the reason the fifth
pass did not: the run the push should produce is a separate observation, and the
plan leaves that item open rather than inferring it.

Revised 2026-09-12, fifth pass, after rebasing onto `origin/check-option` at
`64117e4` on the requester's instruction.

What changed. The branch no longer conflicts with its base. Thirty commits now
stand on `64117e4` where sixty-two had stood on the previous base: git dropped
thirty of them as patch-equivalent, replayed thirty-two, and two of those were
skipped by hand — the `src/cli.rs` extraction, superseded by the base's
`src/command.rs`, and the EP-M7 documentation merge, whose text the base carries
in a fuller form. The branch's one command module is therefore the base's
`src/command.rs`, with the `--git` surface folded into it, and `src/cli.rs` never
lands; `EX-REBASE-STRUCTURE` records why that is the only arrangement leaving one
`Cli` and one `FormatOpts` in the binary. Four of the base's newer patterns were
adopted, because they are now the shape a later reader will expect: the
`crate::command` import paths, `Mode::ListFiles` in the metrics label and its
test list, the split driver test files, and the `Normalize` help wording. The
metrics one is a compile requirement rather than a preference — the base's new
`mode_label` match is exhaustive over `Mode`, so it does not build without a
`ListFiles` arm.

Two entries were corrected rather than merely extended. The Progress record of
the base's movement said `64117e4` stood twenty commits past the merge-base; the
distance is fifty-seven, and the note is corrected in place with the correction
stated rather than quietly overwritten. And the entry recording the decision not
to rebase is now marked superseded, as is this section's reading of it, because
the reason to wait — a base repairing its own failures — is a reason that expires
when the repair lands, and `3737276` is that landing.

What the pass deliberately does not do is claim a CI verdict. The rebase is what
re-opens the pipeline, but the run it produces is a separate observation; the
plan leaves that item open rather than inferring it, so until a run exists the
rebased tip's green is the local gates and nothing more. Two Surprises entries
were added for the rebase itself, and both are about trusting a tool's exit
status: a globally configured `merge=weave` driver returned 0 for a merge that
could not compile, and an auto-merged region between two conflict hunks bound a
name to nothing, because lines both sides agree on are not necessarily lines that
work together.

Revised 2026-09-12, fourth pass, after the branch's first CI run came back red,
its repair was gated, the plan reconciliation was reviewed, and the base branch
was observed repairing its own redness.

What changed. The first CI run on the branch — `34661535150`, the only one it
has — failed both its test jobs over five defects, all of them in the test
tree: a unit fixture whose `git` helper had inherited an environment with no
author identity, and four assertions that spelled a platform fact the way Linux
spells it. `6384543` answers them, and `EV-M4-GATES` records the local gates
green on the repair. Two things are recorded that a summary would have
flattened: one of the six changes is a judgement rather than a finding, because
another test's failure hid the assertion it repairs; and no CI run exists for
the repair, nor for EP-M3's own two commits, because a conflicting pull request
runs no `pull_request` workflows at all — so those commits are, as of this
record, CI-untested. `EV-M4-CI-SILENCE` holds that, and the push that carried
the entry supplied the control the entry had said was missing: a
`pull_request_target` run on the same commit, with no `pull_request` run beside
it.

The round that reviewed all of this, `EV-M4-CR`, returned **zero findings over
95 files** — a wider scope than the earlier rounds, because the reviewed set
tracks the merge-base rather than the branch and roughly two thirds of it is the
base's own traffic. `EX-M4-CR-SCOPE` records the three conditions that
distinguish this from a review that quietly deferred to the pull request, since
both read as "no findings" to anyone who does not look. The base branch, for its
part, has fixed the Windows dead-import failure this plan had recorded against
it, in `3737276`, and has moved fifty-seven commits past the merge-base — which
was the deferral decision's reasoning arriving as evidence, until the requester's
instruction to rebase superseded it the same day, as the fifth pass above
records. The distance is given here as twenty in this pass's first writing and is
corrected on re-measurement.

The Epic path itself is not reopened: the tool's behaviour was correct on both
platforms and every defect was in the tests that observe it. What the pass adds
is the evidence for that claim, the four Surprises entries the run taught, the
Decision-log entry for deferring the rebase, and the one verification this
environment cannot perform — EP-M1's macOS and Windows `canonicalize` case
question — annotated as carried forward in `Progress` and in the INV-DEDUP
residual gap rather than left as a bare unchecked box.

Revised 2026-09-11, third pass, after rebasing onto `main` at `d0549d9` and
studying the merged code.

What changed. Three dependencies merged — #467 (atomic writes), #469 (line
endings) and #470 (single-pass idempotence) — and #464 is in review with the
four forward-compatibility requests adopted in commit `83e6150`. The sequencing
question is settled, so the status returns to DRAFT.

The merged code changed two invariants on their merits rather than merely
renumbering them. `INV-DEDUP` lost its data-loss rationale, because atomic
replacement closes the truncate-and-read race; and it gained a corrected
identity, because collapsing hard links on `(st_dev, st_ino)` would now format
one and leave the other stale. `INV-PROBE-EXCLUSIONS` lost its destructive-write
rationale, because `replace_file` already declines symlinks, and kept the
variant for a weaker but real reason. `INV-NOWRITE-UNCHANGED` is discharged
by #464 and is no longer this plan's work.

Corrections: the decision record moves to ADR 0010, because 0006 and 0007 were
taken; the failure transcript exits 2 rather than 1 and does not render through
`Termination`, because `main` now returns `ExitCode`; and `src/main.rs` at 386
lines makes the file-size contingency a prerequisite rather than a fallback.
Issue #474 has since been fixed by pull request #477 and the plan no longer
carries a caveat for `--headings`. A sweep of the fixture corpus at `408c76a`
found one residual the gates do not cover: `--code-emphasis` settles on the
second pass, not the first. That is now issue #478. Scenarios for
`--git --check` and `--git --diff` were added, since the group change makes
them parse for free.

Revised 2026-09-09, second pass, after the requester identified two in-flight
pieces of work. Atomic `--in-place` writes are issue #465 and CRLF handling is
part of pull request #464, so both were removed from this plan's follow-up list
and repointed at their owners. Reading #464's plan then surfaced a substantial
two-way collision, now recorded as Table 1 under `Conformance basis` with an
unresolved sequencing decision; the plan's status is **BLOCKED** on it.

One correction is independent of that decision and has been made regardless:
`AX-RAYON-ORDER` is **withdrawn**. This plan asserted that
`par_iter().collect()` preserves input order because existing code relies on
it; #464 refuted that with a source citation showing `rayon` documents no such
guarantee. The existing reliance is a latent defect, so ordering must be
carried explicitly by index rather than inherited.

Revised 2026-09-09, first pass, after a six-lens design review, before any
implementation.

What changed. `--git` now selects tracked files only, with
`--include-untracked` restoring full reference-command equivalence, because
untracked files are unrecoverable if damaged. `PathKind` gained a `Symlink`
variant and the probe moved to `symlink_metadata`, closing a path by which
Markdown output could be written over a same-directory source file inside the
capability sandbox. Deduplication moved from path strings to file identity,
closing a concurrent truncate-and-read race that produced a zero-byte file and
exit 0 on case-insensitive filesystems. The composition root returns an
`Inputs` enum rather than a `Vec`, because the previous shape made
`--git` with an empty selection block on standard input. The
`RepositoryFileSource` port was deleted as a pattern transplant, leaving one
port. The module tree moved out of the public library API, and errors moved
from `anyhow` to `thiserror` enums, both to comply with `AGENTS.md`.
`--list-files` and `--allow-conflicted` were added. Assertions on git's and
clap's message text were removed — one scenario asserted a string clap does not
emit. Hand-applied negative controls were replaced with `cargo-mutants`.
EP-M1 and EP-M2 were merged, the scope tolerances were corrected from
arithmetically unsatisfiable values, `rstest-bdd-macros` and `thiserror` were
added to the dependency list, and duplicated rationale was cut.

How it affects the remaining work. Nothing has been implemented, so there is no
rework. The plan is longer in the areas that carry risk and shorter in the
areas that carried ceremony.
