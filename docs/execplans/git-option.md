# Add a `--git` option that selects repository files to format

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: IN PROGRESS — rebased onto `check-option` and being implemented there,
stacked on pull request #464. The prerequisite extraction of `src/cli.rs` has
landed, Stage A is discharged, and EP-M0 to EP-M3 follow. See `Progress`,
`Surprises & discoveries`, and `Conformance basis`, "Related work".

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
  five new fields and the composition wiring. Do the extraction as its own
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
  `docs/architecture.md`, "Atomic in-place writes", with Figure 5.
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
| `driver::ReadOnlyDir` | `--list-files` and any read-only selection path take it, so writing is impossible by type |
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
- Residual gap: whether `std::fs::canonicalize` normalizes case on macOS APFS
  and on Windows is asserted here from documentation, not measured — this
  machine is Linux. EP-M1 must confirm it on the release matrix, both of which
  are release targets as of commit `1f64236`. If it does not, fall back to
  comparing `(st_dev, st_ino)` **plus** parent-directory identity, and record
  the change here.

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
manifest entry, so CON-DEP-001 is untouched. Add a `make mutants` target scoped
to `--file 'src/select/**'`, commit a `mutants.toml`, and make "zero surviving
mutants in `src/select/policy.rs` and `src/select/git_ls_files.rs`" an EP-M1
acceptance criterion. Keep the named controls in this document as a
specification of what must die; let the tool prove it.

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
  passes with every obligation above discharged and observed red first, and
  `make mutants` reports zero survivors in the two named files.
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
        default_values = ["md", "mdc", "markdown"],
        value_parser = select::extensions::parse_extension,
    )]
    md_exts: Vec<String>,
    /// Rewrite files containing conflict markers during a merge or rebase
    #[arg(long = "allow-conflicted")]
    allow_conflicted: bool,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}
```

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
positional is present, and `default_values` on `--md-exts` is a second instance
of the same class. Use an explicit post-parse check emitting a real clap error,
so the exit status stays 2 and the usage footer survives:

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
/// Resolves `--git` into the paths to act on.
///
/// Returns `Inputs::Files`, which may be empty: an empty selection is success
/// and must not fall through to standard input. Never returns `Inputs::Stdin`.
fn resolve_git_inputs(
    cli: &Cli,
    working_directory: &Utf8Path,
) -> Result<Inputs, GitListError>;
```

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

`--list-files` takes `driver::ReadOnlyDir`, or no capability at all, so that it
cannot write by construction rather than by convention.

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
mdtablefix: running `git ls-files`: fatal: not a git repository (or any of
the parent directories): .git
exit=2
```

Expected: exit **2** and a diagnostic naming the cause. Three things about this
transcript are load-bearing and each corrects an earlier draft:

- The status is 2, not 1. Pull request #464 establishes
  `ExitStatus { Success, Drift, Error }`, and a `--git` failure is an
  operational error, which is 2. Exit 1 is reserved for a reporting mode that
  found drift.
- `fn main` now returns `ExitCode`, not `anyhow::Result<()>`, so nothing goes
  through `Termination`. The `Error:` / `Caused by:` rendering a previous draft
  showed cannot occur; the message is whatever the binary prints deliberately.
- The text after the colon comes from `git`, is version- and locale-dependent
  (AX-GIT-NLS), and is therefore asserted by **no** test. Snapshots of failure
  messages are driven through `GitLsFiles::with_program` against a fixture
  program emitting fixed bytes, so they are a function of this repository's
  code rather than of the machine's `git`.

```console
$ mdtablefix --git notes.md ; echo "exit=$?"
error: the argument '--git' cannot be used with '[FILES]...'
exit=2
```

Expected: clap rejects the combination before any file is touched. This wording
is clap's, so the test asserts the exit status; EP-M0 records the verbatim text
for the documentation.

```console
$ mdtablefix --git --check ; echo "exit=$?"
docs/guide.md: would reformat
exit=1
```

Expected: exit **1**, drift rather than error. This is the combination
`--git` exists to enable in continuous integration, and it works because `git`
joined the `inputs` group that `mode` requires.

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
- [ ] EP-M1: confirm `std::fs::canonicalize` case behaviour on the macOS and
      Windows release targets, per the INV-DEDUP residual gap.
- [ ] EP-M1: add `make mutants` and `mutants.toml`; reach zero survivors in
      `src/select/policy.rs` and `src/select/git_ls_files.rs`.
- [ ] EP-M2: add `git` to the `inputs` group and the four supporting flags,
      with post-parse dependency checks; wire `resolve_git_inputs` into
      `driver::Inputs`.
- [ ] EP-M2: bound `Mode::Print` memory with the chunked drain.
- [ ] EP-M2: land the scenarios and the `--help` snapshot.
- [ ] EP-M3: write **ADR 0010** and update `README.md`, `docs/users-guide.md`,
      `docs/architecture.md`, `docs/developers-guide.md`, `docs/contents.md`.
- [ ] Reconcile Decision log and Surprises with ADR 0010, then set Status.

Superseded and deliberately not carried forward: adding `googletest`,
`pretty_assertions`, `rstest-bdd`, and `rstest-bdd-macros`; adding
`cargo test --doc` to the `test` target; and implementing
INV-NOWRITE-UNCHANGED. Pull request #464 does all four.

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

## Decision log

Entries are pointers; the reasoning lives in the body sections named. ADR 0010
is the durable record, and EP-M3 reconciles this log into it.

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

## Outcomes & retrospective

To be completed at EP-M3. Before setting Status to COMPLETE, reconcile every
Surprise and Decision against ADR 0010 and the component documents. Do not mark
COMPLETE while any deviation remains unrecorded.

Two items an earlier draft listed as follow-up work have since **merged** and
must not be reopened: atomic `--in-place` writes are issue #465, delivered by
pull request #467; line-ending preservation is issue #451, delivered by pull
request #469 under ADR 0007. Confirm at closure that neither was reimplemented
in `src/select/`, and that the `--git` write path routes through
`driver::write_back` rather than calling `replace_file` directly.

Issue #474, the `--headings` fixed-point defect, was fixed by pull request #477
before this plan was implemented, so no caveat is needed there. The
`--code-emphasis` two-pass residual recorded under Risks is issue #478 and is
not this plan's work; confirm at closure that it is resolved, or that the
users' guide does not claim one-pass convergence for that flag.

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

## Revision note

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
