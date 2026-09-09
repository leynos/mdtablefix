# Add a `--git` option that selects repository files to format

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: BLOCKED — awaiting the sequencing decision recorded under
`Conformance basis`, "Related work in flight".

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

- Risk: a repository contains a path that is not valid UTF-8, which
  `cap_std::fs_utf8` cannot represent.
  Severity: low. Likelihood: low.
  Mitigation: count and drop such candidates rather than aborting; the
  composition root prints one content-free stderr warning giving the count.

- Risk: `src/main.rs` exceeds 400 lines once the new flags and wiring land.
  Severity: medium. Likelihood: medium.
  Mitigation: all policy and adapter code lives in `src/select/`, so `main.rs`
  gains field declarations plus roughly 60 lines of wiring. Contingency at the
  380-line tolerance: move the composition root into `src/select/compose.rs`,
  following the precedent of `src/process/buffer.rs`.

- Risk: `rstest-bdd` is new to this repository, on a pinned
  `nightly-2026-03-26` toolchain, and pulls in `gherkin`, `fluent`,
  `i18n-embed`, `rust-embed`, and `inventory` — a large increase over the
  current ten lean dev-dependencies. Version `0.6.0-beta3` already exists with
  a reorganized harness, so this adopts an API one minor version before it
  moves.
  Severity: medium. Likelihood: medium.
  Mitigation: EP-M0 proves one trivial scenario compiles and runs before any
  real scenario is written. If it cannot be made to work within the iteration
  tolerance, escalate; do not silently downgrade behavioural coverage to plain
  `assert_cmd`.

- Risk: `format_to_string` splits with `content.lines()`, which strips `\r`,
  then rejoins with `\n`. On a CRLF checkout every selected file's line endings
  change, so a whole-repository run produces a diff of pure churn in which a
  genuine corruption would be invisible.
  Severity: medium. Likelihood: medium on Windows, low elsewhere.
  Mitigation: **resolved elsewhere, not by this plan.** Pull request #464
  introduces `src/document.rs` with `SourceDocument` and
  `LineEnding::detect`, which preserves each document's majority line-ending
  style and its byte-order mark across a rewrite. This plan must not
  reimplement line-ending handling; it consumes that boundary. Retained here
  only so the dependency is visible.

- Risk: `--in-place` writes are truncate-then-write with no signal handling, so
  interrupting a large run can leave a file truncated. `--git` makes long runs
  routine and therefore makes interruption routine.
  Severity: medium. Likelihood: low.
  Mitigation: **tracked separately as issue #465**, "Write files atomically in
  `--in-place` mode", which specifies temporary-file-plus-rename with mode
  preservation and is sequenced with pull request #464 because both rewrite the
  same serialization path. This plan must not implement it. Skipping the write
  when output is byte-identical still shrinks the window in the meantime, and
  that check comes from #464's `Assessment::is_changed` rather than from here.

- Risk: the extension filter is the only thing between `--git` and rewriting
  source files, so a defect there is destructive.
  Severity: high. Likelihood: low.
  Mitigation: verify the filter in both directions rather than only checking
  that selected paths look right. See INV-EXT-SOUND and INV-EXT-COMPLETE.

## Conformance basis

There is **no Terms of Reference document and no separate technical design
document** for this feature. Do not invent one. The governing upstream
artefacts that exist are `AGENTS.md` (at commit `c792270`),
`docs/developers-guide.md`, `docs/documentation-style-guide.md`, and ADRs 0001
to 0005 (none of which constrains file selection). This plan creates the
missing design record as **ADR 0006**.

### Related work in flight, and an unresolved sequencing decision

**Status: this plan is BLOCKED on the decision recorded below.** Do not begin
implementation until it is settled.

Pull request #464, "Plan: add `--check` and `--diff` reporting modes" (branch
`check-option`), is an unapproved draft plan that restructures the exact code
this plan modifies. Issue #465, "Write files atomically in `--in-place` mode",
is sequenced with it. The overlap is not incidental:

| This plan | Pull request #464 | Nature of the overlap |
| --- | --- | --- |
| `ArgGroup "inputs"` over `files` and `git`; `--in-place` requires it | `ArgGroup "mode"` over `--in-place`, `--check`, `--diff`, requiring `files` | **Hard conflict.** Their group requires positional `files`, so `--git --in-place` would be rejected outright. One of the two must give. |
| INV-NOWRITE-UNCHANGED, skip the write when output matches input | `Assessment::is_changed`, "a direct byte comparison, and the authoritative answer" | **Duplicate.** This plan should consume theirs. |
| `--list-files` as a boolean flag | `Mode { Print, InPlace, Check, Diff }` | **Shape.** Selection listing belongs as a `Mode` variant, not a parallel flag. |
| `resolve_inputs` in `src/main.rs` | `src/driver.rs`, binary-private, with `main.rs` reduced to an adapter | **Placement.** The composition root moves. |
| CON-CAP-001 discharged by code review only | `ReadOnlyDir`, making read-only-ness a property of the type | **Theirs is stronger.** `--list-files` should be unable to write by construction. |
| Exit 1 when `git` fails | `ExitStatus { Success, Drift, Error }`, with `Error` mapping to 2 | **Contract conflict.** A `git` failure is an `Error`, so 2, not 1. |
| Transcript rendering `Error: … Caused by:` via `Termination` | `fn main` returns `ExitCode`; crate bumps to `0.6.0` | **This plan's transcript is wrong** if #464 lands first. |
| CRLF churn recorded as an accepted risk | `SourceDocument` and `LineEnding::detect` | **Resolved by theirs.** |

*Table 1: overlap between this plan and pull request #464.*

Beyond the conflicts, the two features compose into the combination most worth
having: `mdtablefix --git --check` is a continuous-integration gate answering
"is every Markdown file in this repository formatted?" with a non-zero exit on
drift. Neither plan currently delivers it, and #464's `mode` group forbids it
by construction.

The decision to be taken, and its consequence for this document:

1. **Sequence this plan after #464.** Rewrite the interfaces here to consume
   `SourceDocument`, `Assessment`, `driver.rs`, `Mode`, `ReadOnlyDir`, and the
   exit-status contract, and change #464's `mode` group to require the
   `inputs` group rather than `files`. Cleanest result; this plan stays blocked
   until #464 merges.
2. **Sequence this plan before #464.** Keep it self-contained and let #464
   absorb `--git` while it rewrites `main.rs` anyway. Cost: this plan builds a
   changed-file comparison and an ordering scheme that #464 then deletes.
3. **Proceed independently.** Not recommended: both modify the same `Cli`
   struct, the same `main`, and the same write path, so whichever merges second
   faces a non-trivial rebase in the code carrying the highest destructive
   risk.

Whichever is chosen, record it in the Decision log, update this section, and
set Status accordingly before Stage A begins.

**Roadmap**: this repository has no general-purpose roadmap. The two roadmap
documents that exist are feature-scoped and neither mentions `--git`,
`git ls-files`, or file selection. The instruction to mark a roadmap entry as
done is therefore **not applicable**; do not create a roadmap entry to satisfy
it. ADR 0006 is the durable record.

Requirements, traced through milestones to evidence:

```plaintext
REQ-GIT-001 -> ADR-0006 -> EP-M1 -> select::git_ls_files::tests::lists_tracked_only
REQ-GIT-002 -> ADR-0006 -> EP-M1 -> select::policy::tests::selects_only_configured_extensions
REQ-GIT-003 -> ADR-0006 -> EP-M2 -> cli_git.rs::md_exts_replaces_the_default_set
REQ-GIT-004 -> ADR-0006 -> EP-M2 -> cli_git.rs::rejects_git_with_explicit_files
REQ-GIT-005 -> ADR-0006 -> EP-M2 -> cli_git.rs::in_place_is_satisfied_by_git
REQ-GIT-006 -> ADR-0006 -> EP-M1 -> select::policy::tests::excludes_missing_other_symlink
REQ-GIT-007 -> ADR-0006 -> EP-M1 -> select::git_ls_files::tests::maps_spawn_and_exit_failures
REQ-GIT-008 -> ADR-0006 -> EP-M2 -> feature::"Exit successfully when nothing is selected"
REQ-GIT-009 -> ADR-0006 -> EP-M2 -> feature::"Refuse to rewrite a conflicted file"
REQ-GIT-010 -> ADR-0006 -> EP-M2 -> feature::"List the selection without acting"
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
the modules alone, without reading ADR 0006.

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
- **AX-CLAP-GRAMMAR** — partially **falsified during planning**; see the note
  under `src/main.rs` in "Interfaces and dependencies". What was verified on
  clap 4.6.6: an `ArgGroup` with `multiple(false)` over `files` and `git`
  rejects `--git a.md` while still accepting `mdtablefix a.md b.md` and
  `--in-place a.md b.md`, so a multi-value positional in an exclusive group is
  safe; and `mode` requiring the `inputs` group rejects `--in-place` alone
  while accepting `--git --in-place`, `--git --check`, and `--git --diff`. What
  was **disproved**: `requires = "git"` on a bool flag, which admits
  `--list-files a.md`. The plan now uses a post-parse check for every such
  dependency. Retained below for the parts that still stand: a `clap::ArgGroup`
  with `multiple(false)` permits at
  most one member; `requires = "<group>"` demands at least one member; and an
  argument carrying `default_values` counts as present, so `requires` on such
  an argument needs `ArgMatches::value_source` rather than a plain relation.
  EP-M0 confirms all three empirically **and captures clap's exact diagnostic
  text**, because three behavioural scenarios assert on strings clap owns
  rather than strings we own. An earlier draft asserted `stderr contains
  "requires"`; clap 4.6.6 renders `the following required arguments were not
  provided:` and never emits "requires" for that error kind.

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

**INV-DEDUP** — no two selected paths denote the same file.

- Method: `rstest` cases over a fake probe returning colliding
  `FileIdentity` values, covering (a) the three-stage merge listing, (b) two
  paths differing only in ASCII case, and (c) a hard link.
- Rationale: **this is a claim about inodes, not strings.** An earlier draft
  deduplicated a `BTreeSet<Utf8PathBuf>` and asserted the result contained no
  duplicate path string — which is a property of `BTreeSet`, not of this
  design, and which cannot see the failure it was named for. On a
  case-insensitive filesystem (macOS APFS and Windows NTFS are both release
  targets, added at commit `1f64236`) `README.md` and `Readme.md` are distinct
  index entries naming one file. Both survive string dedup, both probe as
  `RegularFile`, and `rayon` then runs `format_to_string` and `rewrite_in_place`
  on them concurrently: one thread truncates while the other reads, the reader
  gets `""`, `format_to_string` returns `String::new()` for empty input, and
  the file ends up zero bytes with both threads returning `Ok(())` and the run
  exiting 0. Deduplicating on the `FileIdentity` the probe already fetched
  closes it for nothing.
- Artefact: `src/select/policy.rs` `mod tests`.
- Non-vacuity: each case asserts the collision was actually present in the
  input and that exactly one representative survives. Negative control:
  deduplicating on the path string must fail cases (b) and (c).

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
- Rationale: a finite four-valued partition. `Symlink` is the variant that
  matters and the one an earlier draft omitted: `std::fs::metadata` **follows**
  symlinks, so a tracked `src/notes.md -> lib.rs` would probe as `RegularFile`,
  pass the extension filter on the *link* name, and `open_file_parent("src")`
  plus `Dir::write("notes.md")` would resolve **within** the cap-std sandbox
  and write Markdown output over `src/lib.rs`. The capability boundary does not
  help, because its root is the file's parent directory, not the repository.
  `symlink_metadata` plus an explicit variant is the whole fix.
- Artefact: `src/select/policy.rs` `mod tests`, plus an end-to-end scenario.

**INV-NOWRITE-UNCHANGED** — `--in-place` writes a file only when the formatted
output differs from the bytes read.

- Method: `rstest` on the rewrite helper, plus an end-to-end assertion that an
  already-formatted file's modification time is unchanged after a run.
- Rationale: correctness, not politeness. It removes the truncate-then-write
  window for every unchanged file, which on a healthy repository is most of
  them; it converts the CRLF churn risk from silent to visible; and it makes
  the changed-file summary meaningful.
- Artefact: `src/main.rs` `mod tests` and `tests/cli_git.rs`.
- Non-vacuity: one case must have differing content and assert the write
  happened, so the check cannot pass by never writing.

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
selection later becomes recursive. Full reasoning belongs in ADR 0006, not
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

### EP-M0 — prototyping spike, argument grammar and BDD viability

- Outcome: throwaway evidence for AX-CLAP-GRAMMAR and that `rstest-bdd` plus
  `rstest-bdd-macros` compile and run one trivial scenario on the pinned
  toolchain.
- Acceptance evidence: `EV-M0-GRAMMAR`, a transcript covering `mdtablefix --git
  file.md` (rejected), `mdtablefix --in-place` (rejected), `mdtablefix --git
  --in-place` (accepted), `mdtablefix a.md b.md` (**still accepted** — the
  regression case, since `files` is a `Vec` positional entering an `ArgGroup`),
  `mdtablefix --md-exts md a.md` (rejected), and the **verbatim diagnostic
  text** for each rejection; and `EV-M0-BDD`, a passing one-scenario run.
- Go/no-go: if `ArgGroup` cannot express the grammar, escalate with the
  alternative (a post-parse check with hand-written diagnostics and its own
  snapshot) rather than choosing unilaterally — the diagnostics are
  user-visible. If `rstest-bdd` cannot be made to run, escalate under the
  iteration tolerance.
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
- Additional obligation, from the memory profile: **stdout mode must not retain
  the whole formatted corpus.** Today `cli.files.par_iter().map(...).collect()`
  is a hard barrier that holds every file's complete formatted output before
  printing anything. Measured on this repository, 28 Markdown files average
  17.7 KB, so a 20,000-file documentation monorepo would hold roughly 354 MB —
  fine on a workstation, fatal in a 2 GB CI container, and previously bounded
  only by `ARG_MAX`. `--git` removes that bound. Iterate `chunks(256)`
  sequentially, `par_iter()` within each chunk, and drain each chunk to a
  `BufWriter` over a single `stdout().lock()` before the next. Peak drops to a
  constant ~4.5 MB, and the `BufWriter` also collapses the current one
  `write(2)` per output line — `Stdout` is `LineWriter`-backed unconditionally,
  which at 20,000 files is roughly 8.6 million syscalls. Because
  `AX-RAYON-ORDER` is withdrawn, ordering within a chunk must be
  re-established explicitly from each unit's
  index rather than inherited from `collect`; pull request #464 introduces
  `driver::in_argument_order` for exactly this, and this plan uses it rather
  than reimplementing it.
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
  `docs/adrs/0006-git-file-selection.md` describe the feature, the boundary,
  the CRLF caveat, and the rejected alternatives.
- Acceptance evidence: `EV-M3-DOCS` — `make markdownlint` and, if a Mermaid
  diagram was added, `make nixie` both pass.
- Conformance check: every Decision log entry appears in ADR 0006 or a
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

    /// Iterates the extensions in sorted order, without dots.
    pub fn iter(&self) -> impl Iterator<Item = &str>;
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
/// On Unix this is `(st_dev, st_ino)`; elsewhere it is the canonicalized path.
/// Deduplication keys on this, not on the path string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileIdentity(/* platform-specific */);

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

In `src/main.rs`, the `Cli` struct gains:

```rust
#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
#[command(group(
    clap::ArgGroup::new("inputs").args(["files", "git"]).multiple(false)
))]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", requires = "inputs")]
    in_place: bool,
    /// Select Markdown files tracked by Git beneath the current directory
    #[arg(long = "git")]
    git: bool,
    /// Also select untracked files that Git does not ignore
    #[arg(long = "include-untracked")] // see the note on `requires` below
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
    /// Print the selected paths and exit without reading or writing them
    #[arg(long = "list-files")] // see the note on `requires` below
    list_files: bool,
    /// Rewrite files containing conflict markers during a merge or rebase
    #[arg(long = "allow-conflicted")] // see the note on `requires` below
    allow_conflicted: bool,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}
```

`value_delimiter` plus `value_parser` rather than a hand-parsed
`Option<String>`: clap drops `default_values` entirely when any occurrence is
supplied, giving replacement semantics for free; both `--md-exts md,mdc` and
`--md-exts md --md-exts mdc` work; an invalid extension becomes a clap error
with exit status 2 and a usage footer, matching every other argument mistake
rather than exiting 1 through `anyhow`; and `--help` renders the default set
automatically.

**`requires = "git"` does not work here and must not be used.** Empirical
result on clap 4.6.6, with `git` a member of the `inputs` group and `files` a
`Vec` positional: `--list-files` alone is correctly rejected, but
`--list-files a.md` is **accepted**, silently running a `--git`-only flag with
no `--git`. The same holds when exclusivity is expressed with
`conflicts_with = "files"` instead of a group, so group membership is not the
cause; a bool flag's `requires` on another bool flag is simply not dependable
once a positional is present. The `default_values` on `--md-exts` are a second
instance of the same class of problem, since a defaulted argument always counts
as present.

Express all four dependencies as an explicit post-parse check instead, emitting
a real clap error so the exit status stays 2 and the usage footer is preserved:

```rust
impl Cli {
    /// Parses and enforces the dependencies `clap` cannot express here.
    fn parse_validated() -> Self {
        let cli = Self::parse();
        for (name, present) in [
            ("--include-untracked", cli.include_untracked),
            ("--list-files", cli.list_files),
            ("--allow-conflicted", cli.allow_conflicted),
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

`--md-exts` needs the same treatment, keyed on
`ArgMatches::value_source(..) != Some(ValueSource::DefaultValue)` rather than
on a bool. EP-M0 must confirm all of this, including that the resulting exit
status is 2.

The composition root returns a type, not an overloaded emptiness sentinel:

```rust
/// What `main` should act on.
enum Inputs {
    /// Read standard input. Only when no positional files and no `--git`.
    Stdin,
    /// Act on these paths. May be empty, which is success and not stdin.
    Files(Vec<camino::Utf8PathBuf>),
}

fn resolve_inputs(cli: &Cli) -> anyhow::Result<Inputs>;
```

An earlier draft returned `Vec<PathBuf>` and left `main` branching on
`is_empty()`. That conflates "the user named no files, so read stdin" with
"`--git` legitimately matched nothing", so `mdtablefix --git --in-place` in a
repository with no Markdown would block on a TTY forever — and under
`--in-place` would then print formatted stdin, silently the wrong mode.
REQ-GIT-008 exists because of this. Carrying `Utf8PathBuf` rather than
`PathBuf` also avoids round-tripping through a lossy type only for
`open_file_parent` to re-validate it.

`anyhow` remains correct in `main.rs`, which is the application boundary
`AGENTS.md:266` describes.

New `[dependencies]`:

```toml
thiserror = "2"
```

New `[dev-dependencies]`:

```toml
rstest-bdd = "0.5"
rstest-bdd-macros = "0.5"
googletest = "0.14"
pretty_assertions = "1.4"
```

`rstest-bdd-macros` is listed explicitly and deliberately. `rstest-bdd` 0.5.0
does **not** re-export the macros — verified against the vendored source, whose
`lib.rs` re-exports `context`, `registry`, `pattern` and others but no
`rstest_bdd_macros` — so every consumer writes
`use rstest_bdd_macros::{given, scenario, then, when};`. The upstream README's
two-line install block is incomplete.

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
```

`LC_ALL` and `LANGUAGE` are belt and braces only. Per AX-GIT-NLS, no assertion
may depend on git's message text regardless.

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

  Scenario: Reject combining --git with explicit file arguments
    When I run mdtablefix with "--git notes.md"
    Then the command exits with status 2

  Scenario: Reject --list-files without --git
    When I run mdtablefix with "--list-files notes.md"
    Then the command exits with status 2
```

Three assertion choices are deliberate. `stderr contains "git ls-files"` names
**our** wrapper's wording, never git's, per AX-GIT-NLS. The clap rejections
assert on the **exit status** rather than on message text, because an earlier
draft asserted `stderr contains "requires"` and clap 4.6.6 renders `the
following required arguments were not provided:` for that error kind — the
scenario would have failed on day one. `--list-files` gives the selection an
honest oracle; an earlier draft asserted dedup by grepping a conflict marker
out of a concatenated content dump, which was a symptom of the missing
affordance.

Step definitions share state through an `rstest` fixture that builds the
repository in a `tempfile::TempDir` and returns a handle carrying the directory
and the last command's output. No globals and no `static`: they would make the
scenarios order-dependent. `standard input was not read` is asserted by running
the command with a closed or empty stdin and a timeout.

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
Error: running `git ls-files`

Caused by:
    fatal: not a git repository (or any of the parent directories): .git
exit=1
```

Expected: a non-zero exit and a diagnostic naming the cause. Note the shape:
`fn main() -> anyhow::Result<()>` prints through `Termination`, which renders
`Error: {e:?}` — capital `E`, with an indented `Caused by:` chain. An earlier
draft's transcript showed a lowercase colon-joined `error:` line, which nothing
in the binary can produce. The **second** line comes from git and is
version- and locale-dependent, so no test asserts on it; snapshots of failure
messages are driven through `GitLsFiles::with_program` against a fixture
program emitting fixed bytes, so they are a function of our code.

```console
$ mdtablefix --git notes.md ; echo "exit=$?"
error: the argument '--git' cannot be used with '[FILES]...'
exit=2
```

Expected: clap rejects the combination before any file is touched. This wording
is clap's, so the test asserts the exit status; EP-M0 records the verbatim text
for the documentation.

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
  and strip control characters before printing. State both halves in ADR 0006.

## Idempotence and recovery

Every step is re-runnable. `mdtablefix --git --in-place` is idempotent on its
own output, which INV-NOWRITE-UNCHANGED strengthens into "a second run performs
no writes at all". The gates are read-only apart from build artefacts.
`cargo insta reject` undoes a snapshot review.

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

- [ ] Stage A: re-establish the reference-command transcripts on this machine.
- [ ] EP-M0: confirm AX-CLAP-GRAMMAR, including the `a.md b.md` regression case
      and the `default_values` plus `requires` wrinkle; capture verbatim clap
      diagnostics.
- [ ] EP-M0: prove one `rstest-bdd` scenario runs on the pinned toolchain.
- [ ] Stage B: add the five dependencies and write the feature file.
- [ ] Stage B: write the red unit and property tests for EP-M1.
- [ ] EP-M1: implement `extensions`, `policy`, `conflict`, `git_ls_files`, and
      `fs_probe`; discharge INV-NUL-SPLIT, INV-EXT-SOUND, INV-EXT-COMPLETE,
      INV-DEDUP, INV-ORDER-DET, INV-PROBE-EXCLUSIONS, INV-CONFLICT-GUARD.
- [ ] EP-M1: add `cargo test --doc` to the `test` target; add `make mutants`
      and `mutants.toml`; reach zero survivors in the two named files.
- [ ] EP-M2: wire the five new flags and `resolve_inputs` into `main`.
- [ ] EP-M2: implement the chunked stdout drain and INV-NOWRITE-UNCHANGED.
- [ ] EP-M2: land the scenarios and the `--help` snapshot.
- [ ] EP-M3: write ADR 0006 and update `README.md`, `docs/users-guide.md`,
      `docs/architecture.md`, `docs/developers-guide.md`, `docs/contents.md`.
- [ ] Reconcile Decision log and Surprises with ADR 0006, then set Status.

## Surprises & discoveries

- Observation: `git ls-files -t` does not reliably flag a tracked file deleted
  from the working tree; it reported `H`, not `R`, because the `R` tag requires
  `--deleted`.
  Evidence: probing a repository with a committed then deleted `a.md` produced
  `H a.md`.
  Impact: status tags cannot substitute for a filesystem probe. This is why the
  design uses `PathProbe` rather than parsing `-t` output.

- Observation: `rstest-bdd` 0.5.0 does not re-export its procedural macros.
  Evidence: its `lib.rs` re-exports `context`, `registry`, `pattern`,
  `localization` and others, but nothing from `rstest_bdd_macros`.
  Impact: `rstest-bdd-macros` is a required second dev-dependency. The upstream
  README's install block is incomplete.

- Observation: `tests/table/` is not compiled by any Cargo target. No top-level
  `tests/*.rs` declares `mod table` or a `#[path]` to it.
  Evidence: an exhaustive grep of `tests/` returned no match; the only
  `mod table` in the repository is `src/lib.rs`'s unrelated production module.
  Impact: do not add tests to `tests/table/`; they would never run. Do not fix
  it here either — out of scope.

- Observation: `make test` runs `cargo test --all-targets`, which excludes
  doctests.
  Evidence: the `test` target in `Makefile`.
  Impact: any `# Examples` block is unverified by every current gate. Stage C
  adds `cargo test --doc`.

## Decision log

Entries are pointers; the reasoning lives in the body sections named. ADR 0006
is the durable record, and EP-M3 reconciles this log into it.

- Decision: spawn `git ls-files -z` rather than link `git2` or `gix`.
  Rationale: equivalence by construction rather than by reimplementation; no
  Git-operating dependency; inherits every Git configuration input for free.
  libgit2 diverges from Git on nested `.gitignore` negation, so `git2` would
  make equivalence approximate. The stronger argument, which ADR 0006 should
  lead with, is that a walker-based approach such as the `ignore` crate
  **structurally cannot see force-added ignored files**, whereas `--cached`
  gets them right by construction. Cost: `git` on `PATH`, turned into an
  actionable message by REQ-GIT-007.
  Date/Author: 2026-09-09, planning agent, confirmed by the requester.

- Decision: `--git` selects tracked files only; untracked selection is opt-in
  behind `--include-untracked`.
  Rationale: `--others` is exactly the set of files Git cannot restore. A user
  with `$HOME` under version control who runs `--git --in-place --wrap` would
  otherwise reformat every Markdown file in their home directory with no undo.
  Making the recoverable set the default bounds the blast radius; the full
  reference-command equivalence remains available in one extra flag.
  Date/Author: 2026-09-09, requester, on a reviewer pre-mortem.

- Decision: narrow to Markdown extensions, defaulting to `md`, `mdc`,
  `markdown`, with `--md-exts` to override by replacement, and no effect on
  positional paths.
  Rationale: `git ls-files` reports every file in the repository, and
  `mdtablefix` has never filtered by extension, so an unfiltered run would
  rewrite `.rs` and `.toml` files. This is a safety requirement, hence
  CON-SAFE-001 and the bidirectional INV-EXT-SOUND/COMPLETE. Requiring `--git`
  is deliberate: silently discarding a file the user named would be wrong.
  Date/Author: 2026-09-09, requester and planning agent.

- Decision: deduplicate on file identity, not on the path string; sort the
  result.
  Rationale: see INV-DEDUP. The identity is free because the probe already
  fetches the metadata.
  Date/Author: 2026-09-09, planning agent, on a reviewer finding.

- Decision: `symlink_metadata` and an explicit `PathKind::Symlink`, excluded.
  Rationale: see INV-PROBE-EXCLUSIONS. CON-SAFE-001 is stated in terms of
  inodes written rather than paths selected for the same reason.
  Date/Author: 2026-09-09, planning agent, on a reviewer finding.

- Decision: do not pass `--full-name`; `--git` is subtree-scoped, and the tool
  does not announce the scope.
  Rationale: this is the reference command's own behaviour, so preserving it is
  what equivalence means, and it lets a user format one subtree without extra
  arguments. Three reviewers argued for announcing the resolved scope on
  stderr, on the grounds that a flag named `--git` reads as "the repository";
  the requester chose silence. Document the scoping prominently in the users'
  guide, since it is the behaviour most likely to surprise. Adding a
  `--repo-root` flag later is additive; changing the default would not be.
  Date/Author: 2026-09-09, requester.

- Decision: refuse to rewrite conflict-marked files during an in-progress Git
  operation, with `--allow-conflicted` to override.
  Rationale: see INV-CONFLICT-GUARD. Two-layer detection — repository state
  first, then file content — keeps false positives away from documents that
  merely discuss conflict markers.
  Date/Author: 2026-09-09, requester, on a reviewer pre-mortem.

- Decision: add `--list-files`.
  Rationale: two independent needs converge on it. Without `--in-place`,
  `--git` otherwise concatenates every selected file to stdout, which is an
  unlabelled blob nobody wants and the cause of the memory profile EP-M2 has to
  fix; and the behavioural scenarios need an honest oracle for "which files
  were selected", which an earlier draft faked by grepping a conflict marker
  out of that blob. It costs about ten lines. If scope must be cut, this is the
  first candidate — but the scenarios would need reworking.
  Date/Author: 2026-09-09, planning agent, on reviewer findings.

- Decision: keep the selection module tree private to the binary crate; add no
  public library API.
  Rationale: one in-package consumer, and `AGENTS.md:268` forbids exporting an
  opaque error type from a library. Publishing nine items on a crates.io crate
  to serve one caller is how a crate arrives at 1.0 with a surface nobody
  chose. Testability is unaffected: unit and property tests live in
  `#[cfg(test)] mod tests` inside the binary, exactly as `src/main.rs` already
  does. Cost: evidence commands read `cargo test --bin mdtablefix` rather than
  `--lib`.
  Date/Author: 2026-09-09, planning agent, on reviewer findings.

- Decision: use `thiserror` domain enums, not `anyhow`, inside `src/select/`.
  Rationale: `AGENTS.md:264` mandates it and `AGENTS.md:268` forbids the
  alternative. An earlier draft banned `thiserror` under a self-issued
  constraint, which is a plan overriding a binding repository rule — and it was
  vendoring `thiserror` transitively through `rstest-bdd` anyway. `anyhow`
  remains correct in `main.rs`, the application boundary.
  Date/Author: 2026-09-09, planning agent, on a reviewer finding.

- Decision: delete the `RepositoryFileSource` port; keep only `PathProbe`.
  Rationale: see Architectural boundaries.
  Date/Author: 2026-09-09, planning agent, on three concurring reviewers.

- Decision: replace hand-applied negative controls with `cargo-mutants`.
  Rationale: see the note under the Verification plan.
  Date/Author: 2026-09-09, planning agent, on a reviewer finding.

- Decision: `std::fs::symlink_metadata` in `AmbientPathProbe` is a deliberate,
  narrow exception to `AGENTS.md:232`.
  Rationale: classification needs the real path, and `cap_std` offers no
  ambient stat. The exception covers metadata only; contents still flow through
  the capability boundary, which is why CON-CAP-001 is worded as it is.
  Date/Author: 2026-09-09, planning agent.

- Decision: consider and reject the broader redesign in which positional
  arguments accept directories and git-awareness becomes a property of path
  expansion rather than a mode flag.
  Rationale: a reviewer showed this is what comparable tools do — `ruff`,
  `dprint`, `prettier`, `markdownlint-cli2` and `typos` all take positional
  paths and consult `.gitignore` — and it would remove the `--git`-versus-files
  exclusion, the `ArgGroup`, and AX-CLAP-GRAMMAR entirely. It is a genuinely
  better long-term shape. It is also a behaviour change to an existing
  argument: `mdtablefix somedir/` is an error today and would become a
  recursive rewrite. That is a different feature from the one requested, and
  bundling it would widen the blast radius of a change whose whole risk profile
  is unintended writes. Record it in ADR 0006 as the recommended successor.
  Date/Author: 2026-09-09, planning agent, on a reviewer alternative.

- Decision: the roadmap instruction is **not applicable**.
  Rationale: see Conformance basis. Do not create a roadmap entry to tick off.
  Date/Author: 2026-09-09, requester.

## Outcomes & retrospective

To be completed at EP-M3. Before setting Status to COMPLETE, reconcile every
Surprise and Decision against ADR 0006 and the component documents. Do not mark
COMPLETE while any deviation remains unrecorded.

Two items an earlier draft listed as follow-up work are already owned
elsewhere, and must **not** be reopened here: atomic `--in-place` writes are
issue #465, and CRLF and byte-order-mark preservation is part of pull
request #464. Confirm at closure that neither was reimplemented in
`src/select/`.

## Artefacts and notes

The reference-command transcripts and the acceptance transcripts are the
primary artefacts. Add, as work proceeds: EP-M0's verbatim clap diagnostics;
the red and green output for each milestone; the `make mutants` survivor
report; the accepted `--help` snapshot; and the final gate run. Keep them
short.

## Revision note

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
