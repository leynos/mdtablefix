# Add a `--git` option that selects repository files to format

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Today `mdtablefix` only formats the files a user names on the command line, so
tidying a whole repository means writing a shell pipeline and remembering to
exclude `target/`, `node_modules/`, and anything else `.gitignore` covers. Get
that pipeline wrong and the tool rewrites build output or, worse, source code.

After this change a user standing anywhere inside a Git working tree can run:

```console
mdtablefix --git --in-place --wrap
```

and every Markdown file that Git tracks, plus every untracked Markdown file
Git does not ignore, is reflowed in place. Nothing ignored is touched, nothing
non-Markdown is touched, and the selection is exactly the set that
`git ls-files --cached --others --exclude-standard` reports.

Success is observable without reading any code:

- `mdtablefix --git --in-place` inside a repository reformats the tracked and
  untracked Markdown files and leaves `.gitignore`d Markdown and `.rs` sources
  byte-identical.
- `mdtablefix --git --md-exts mdc,markdown --in-place` reformats only files
  with those extensions.
- `mdtablefix --git` outside a Git repository exits non-zero and prints a
  diagnostic naming the problem, without modifying anything.
- `mdtablefix --git some-file.md` is rejected by argument parsing, because the
  two ways of choosing files are mutually exclusive.

## Context and orientation

`mdtablefix` is a single Rust workspace: a library crate rooted at
`src/lib.rs` and a binary crate rooted at `src/main.rs`, plus a small
proc-macro helper crate at `test-macros/`. The package version is `0.5.1`, so
it is pre-1.0 and carries no source-compatibility obligations.

Read these before starting, in this order:

- `AGENTS.md` — the repository's binding style, testing, and commit rules.
- `docs/contents.md` — the documentation index; it names every document below.
- `docs/architecture.md` — the processing pipeline and the `rayon` concurrency
  model.
- `docs/developers-guide.md` — internal conventions, in particular the
  capability-scoped filesystem boundary and the observability rules.
- `docs/documentation-style-guide.md` — British English with Oxford spelling,
  prose wrapped at 80 columns, language identifiers on every fenced block,
  captioned figures and tables.
- `docs/rust-testing-with-rstest-fixtures.md` — the fixture-first testing
  style this repository follows.
- `docs/rust-doctest-dry-guide.md` — how to write the `# Examples` blocks the
  new public API needs.

Load these agent skills while working: `rust-router` (then `rust-unit-testing`
for the test shapes, `rust-errors` for the error type, and `arch-crate-design`
for the module boundary), `hexagonal-architecture` for the port and adapter
split, `proptest` for the property tests, `arch-decision-records` for the new
ADR, and `en-gb-oxendict` for prose.

### How file selection works today

`src/main.rs` is 303 lines. Its `Cli` struct (lines 26 to 36) has one
positional field, `files: Vec<PathBuf>`, and one flag with a cross-field
constraint:

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

`main` branches on whether `cli.files` is empty. If it is, the tool reads
standard input. Otherwise it maps `cli.files.par_iter()` through
`open_file_parent`, which `src/main.rs` documents as "the only ambient
filesystem boundary for CLI file processing": it opens the file's parent
directory as a `cap_std::fs_utf8::Dir` capability and returns a relative
`camino::Utf8PathBuf`. Every later read and write goes through that
capability. `report_results` then prints per-file errors to stderr and returns
the first error, so one bad file does not abort the others.

There is no glob expansion, no directory walking, and no extension filtering
anywhere in the crate. `walkdir`, `glob`, `ignore`, `git2`, and `gix` are all
absent from `Cargo.toml`. Nothing in the repository shells out to `git`.

### Terms used in this plan

- **Tracked file** — a path recorded in Git's index. `git ls-files --cached`
  lists these, including paths whose working-tree copy has been deleted.
- **Untracked file** — a path present in the working tree but absent from the
  index. `git ls-files --others` lists these.
- **Standard exclusions** — `--exclude-standard` adds `.git/info/exclude`,
  every directory's `.gitignore`, and the user's global excludes file, which is
  what porcelain commands such as `git status` apply.
- **Gitlink** — an index entry of mode `160000` recording a submodule commit.
  It appears in `ls-files` output as a directory path, not a file.
- **Candidate** — a path emitted by the file source before this tool applies
  any policy.
- **Port** — a trait the domain defines and depends on. **Adapter** — an
  implementation of a port that touches the outside world.

### Measured behaviour of the reference command

These transcripts were captured on Git 2.x while drafting this plan. Treat
them as the specification of what "semantically equivalent" means; the
end-to-end tests re-establish them against the real `git` on the machine.

Given a repository containing a committed `tracked.md`, `sub/subtracked.md`,
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

Four facts follow, each of which drives a design decision.

1. Output is **not globally sorted**. The untracked pass runs first and the
   cached pass second, each sorted within itself. Selection must impose its own
   order to be reproducible.
2. `sub/subtracked.md` is listed even though it no longer exists on disk,
   because it is still in the index. Selection must tolerate candidates that
   are absent from the working tree.
3. Run from `sub/`, the command scopes to that subtree and prints paths
   relative to the current directory (`subuntracked.md`, `deep/deeptracked.md`,
   `subtracked.md`). `--full-name` would print repository-root-relative paths
   instead. This plan does **not** pass `--full-name`, so `--git` scopes to the
   current directory exactly as the reference command does.
4. Without `-z`, a path containing a space, a quote, or a non-ASCII byte is
   printed C-quoted, for example `"we ird\303\251\"q.md"`. With `-z` the bytes
   are emitted verbatim and NUL-terminated. `-z` is therefore mandatory, not an
   optimization.

During an unresolved merge the same path is emitted once per index stage:

```console
$ git ls-files --cached --others --exclude-standard
c.md
c.md
c.md
```

Selection must therefore deduplicate.

Outside a repository the command fails loudly:

```console
$ git ls-files -z --cached --others --exclude-standard
fatal: not a git repository (or any of the parent directories): .git
exit=128
```

## Constraints

These are hard invariants. If satisfying the objective would violate one, stop
and escalate rather than working around it.

- **CON-CAP-001** — All file reads and writes continue to flow through the
  capability-scoped boundary that `docs/developers-guide.md` mandates:
  `open_file_parent` returns a `cap_std::fs_utf8::Dir` and a relative
  `camino::Utf8Path`, and handlers must not perform ambient filesystem access
  of their own. New selection code may stat candidate paths to classify them,
  but must not read or write file contents outside that boundary.
- **CON-DEP-001** — No new runtime dependency. The selected mechanism spawns
  the `git` binary through `std::process::Command`, which is already in the
  standard library. Adding `git2`, `gix`, `ignore`, `walkdir`, or `glob` is a
  tolerance breach, not a design choice available to the implementor.
- **CON-SIZE-001** — No source file may exceed 400 lines (`AGENTS.md`).
  `src/main.rs` is already 303 lines and `src/process.rs` is 376.
- **CON-LINT-001** — `cargo clippy --workspace --all-targets --all-features
  -- -D warnings` must pass. Lints may not be silenced except as a last
  resort, and any `#[expect]` must carry a `reason`.
- **CON-OBS-001** — Library code emits `debug!` and `trace!` only, never
  `info!` or above, and tracing fields must never contain document or path
  content (`docs/developers-guide.md`, Observability).
- **CON-SAFE-001** — `--git` must never widen the set of files written beyond
  what the reference command reports intersected with the configured Markdown
  extensions. Rewriting a `.rs` file or an ignored file is a defect of the
  highest severity.
- **CON-DOC-001** — Documentation follows `docs/documentation-style-guide.md`:
  British English with Oxford spelling, prose at 80 columns, a language
  identifier on every fenced block, and a captioned figure with a
  screen-reader description preceding any Mermaid diagram.

## Tolerances (exception triggers)

Stop and escalate when any of these is reached. Do not work around them.

- **Scope** — more than 14 files touched, or more than 900 net added lines
  across the whole plan.
- **Dependencies** — any new entry under `[dependencies]`. The three planned
  `[dev-dependencies]` additions (`rstest-bdd`, `googletest`,
  `pretty_assertions`) are pre-authorized; a fourth is not.
- **Interface** — any change to an existing public library item's signature,
  or any change to the meaning of an existing CLI flag other than the
  documented `--in-place` relaxation in REQ-GIT-005.
- **File size** — `src/main.rs` reaching 380 lines. The contingency is
  recorded under Risks; take it, then continue.
- **Iterations** — a gate still failing after four fix attempts.
- **Ambiguity** — any point where two readings of this plan would produce
  materially different user-visible behaviour.
- **Semantics** — any discovered divergence between the implemented selection
  and the reference command that this plan does not already record as
  deliberate.

## Risks

- Risk: `git` is absent from `PATH` on a user's machine, so `--git` fails at
  runtime even though `mdtablefix` installed cleanly.
  Severity: medium. Likelihood: low.
  Mitigation: map `io::ErrorKind::NotFound` from the spawn to a dedicated,
  actionable message naming `git` and `PATH`. Cover it with a test that
  injects a non-existent program name through the adapter's program seam.
  Accepted trade-off: `--git` is opt-in, and a user asking for Git-based
  selection has a Git working tree, so requiring the client is reasonable.

- Risk: a repository contains a path that is not valid UTF-8, which
  `cap_std::fs_utf8` cannot represent.
  Severity: low. Likelihood: low.
  Mitigation: the adapter counts and drops such candidates rather than
  aborting, and the composition root prints one content-free stderr warning
  giving the count. This matches the existing partial-failure posture of
  `report_results`.

- Risk: `src/main.rs` exceeds the 400-line cap once the new flags and wiring
  land, breaching CON-SIZE-001.
  Severity: medium. Likelihood: medium.
  Mitigation: keep all policy and adapter code in `src/select/`, so `main.rs`
  gains only field declarations and roughly 40 lines of wiring. Contingency, to
  be taken at the 380-line tolerance: extract the composition root into
  `src/bin_support/inputs.rs` and include it from `main.rs` with
  `#[path = "bin_support/inputs.rs"] mod inputs;`, following the precedent of
  `src/process/buffer.rs`, which `src/process.rs` split out for this reason.

- Risk: `rstest-bdd` is new to this repository, so its integration may not be
  smooth on the pinned `nightly-2026-03-26` toolchain.
  Severity: medium. Likelihood: medium.
  Mitigation: EP-M0 proves one trivial scenario compiles and runs before any
  real scenario is written. If it cannot be made to work within the iteration
  tolerance, escalate; do not silently downgrade the behavioural coverage to
  plain `assert_cmd` tests.

- Risk: integration tests in `tests/` cannot observe library `tracing` events,
  because `tracing-test` is declared without the `no-env-filter` feature and
  every existing `#[traced_test]` usage lives inside `src/`.
  Severity: low. Likelihood: medium.
  Mitigation: assert tracing behaviour from `src/select/` unit tests, where the
  existing pattern already works. Only if an integration-level tracing
  assertion proves necessary, add
  `tracing-test = { version = "0.2", features = ["no-env-filter"] }` and record
  it in the Decision log.

- Risk: the extension filter is the only thing standing between `--git` and
  rewriting source files, so a defect there is destructive.
  Severity: high. Likelihood: low.
  Mitigation: verify the filter in both directions (soundness and
  completeness) rather than only checking that selected paths look right, and
  include a negative control that a one-sided or always-empty filter fails.
  See INV-EXT-SOUND and INV-EXT-COMPLETE.

## Conformance basis

There is **no Terms of Reference document and no separate technical design
document** for this feature. Do not invent one. The governing upstream
artefacts that do exist are:

- `AGENTS.md` (repository root, at commit `c792270`) — style, testing, and
  commit-gate rules.
- `docs/developers-guide.md` — the capability-scoped filesystem boundary and
  the observability rules, which supply CON-CAP-001 and CON-OBS-001.
- `docs/documentation-style-guide.md` — CON-DOC-001.
- `docs/adrs/0001` through `0005` — existing accepted decisions; none of them
  constrains file selection.
- `docs/execplans/parallel-processing-roadmap.md` — the delivered work that
  established the `rayon` file-level concurrency this plan reuses.

This plan creates the missing design record as **ADR 0006**, which becomes the
upstream artefact that later work traces to.

**Roadmap**: this repository has no general-purpose roadmap. The two roadmap
documents that exist (`docs/state-machine-abstractions-roadmap.md` and
`docs/execplans/parallel-processing-roadmap.md`) are feature-scoped and neither
mentions `--git`, `git ls-files`, or file selection. The instruction to mark a
roadmap entry as done on completion is therefore **not applicable**; see the
Decision log. Do not create a roadmap entry to satisfy it.

Requirements introduced by this plan, and their trace to milestones and
evidence:

```plaintext
REQ-GIT-001 -> ADR-0006 -> EP-M2 -> tests/git_ls_files.rs::lists_tracked_and_untracked_but_not_ignored
REQ-GIT-002 -> ADR-0006 -> EP-M1 -> src/select/policy.rs::tests::selects_only_markdown_extensions
REQ-GIT-003 -> ADR-0006 -> EP-M1 -> src/select/extensions.rs::tests::parses_extension_spec
REQ-GIT-004 -> ADR-0006 -> EP-M3 -> tests/cli_git.rs::rejects_git_with_explicit_files
REQ-GIT-005 -> ADR-0006 -> EP-M3 -> tests/cli_git.rs::in_place_is_satisfied_by_git
REQ-GIT-006 -> ADR-0006 -> EP-M1 -> src/select/policy.rs::tests::skips_missing_and_non_regular
REQ-GIT-007 -> ADR-0006 -> EP-M2 -> tests/git_ls_files.rs::reports_missing_git_and_missing_repository
CON-CAP-001 -> EP-M3 -> tests/cli_git.rs::writes_only_through_capability_boundary
CON-SAFE-001 -> EP-M3 -> git_file_selection.feature::"Reformat every ... in place"
```

The requirements themselves:

- **REQ-GIT-001** — With `--git`, the candidate set is exactly the set of paths
  that `git ls-files --cached --others --exclude-standard`, run in the current
  working directory, reports.
- **REQ-GIT-002** — Candidates are narrowed to Markdown files. The default
  extension set is `md`, `mdc`, and `markdown`, matched case-insensitively.
- **REQ-GIT-003** — `--md-exts` replaces the default extension set with a
  comma-separated list. A leading dot is optional, surrounding whitespace is
  ignored, and matching stays case-insensitive.
- **REQ-GIT-004** — `--git` and positional file arguments are mutually
  exclusive; supplying both is an argument-parsing error.
- **REQ-GIT-005** — `--in-place` is satisfied by either positional files or
  `--git`. `--in-place` alone remains an error.
- **REQ-GIT-006** — A candidate that is absent from the working tree, or that
  is not a regular file, is skipped silently rather than reported as an error.
- **REQ-GIT-007** — When `git` cannot be spawned, or exits non-zero, the tool
  exits non-zero with a diagnostic that names the cause and, where `git`
  supplied one, includes `git`'s own stderr.

## Architectural boundaries

Apply `hexagonal-architecture` here to protect one genuine boundary, not to
restructure the crate. The rest of `mdtablefix` is a pure text pipeline with no
infrastructure to isolate, and it stays exactly as it is.

The boundary worth protecting is between **what to select** (policy: which
extensions count, how duplicates collapse, what order results come back in,
what to do about a candidate that is not there) and **how candidates are
discovered** (infrastructure: spawning a subprocess, decoding a NUL-delimited
byte stream, stat-ing paths). Without the split, every policy rule can only be
tested by building a real Git repository on disk, which is slow and makes the
awkward cases — an unresolved merge, a non-UTF-8 path, a staged deletion —
tedious to reach.

Two driven ports, no more. A third would be a pattern transplant.

```rust
/// Supplies candidate paths for selection, relative to the working directory.
pub trait RepositoryFileSource {
    fn list_candidates(&self) -> anyhow::Result<GitFileListing>;
}

/// Reports what a candidate path actually is in the working tree.
pub trait PathProbe {
    fn probe(&self, path: &Utf8Path) -> PathKind;
}
```

The domain — `MarkdownExtensions` and `select_markdown_files` — depends on
`PathProbe` and on nothing else. It performs no I/O, so its tests need no
filesystem and no repository. The adapters `GitLsFiles` and `AmbientPathProbe`
implement the ports and are the only code that touches a process or the
filesystem. `src/main.rs` remains the composition root: it constructs the
adapters, hands them to the domain, and feeds the result into the existing
`rayon` pipeline.

## Verification plan

Verification is designed alongside the implementation, not bolted on. The
decomposition above exists partly because it makes each obligation below
dischargeable without a repository on disk.

### Non-trivial axioms

These are assumed, not verified. Do not write tests that attempt to verify
third-party internals; instead, exercise this repository's logic against the
real interface at the boundary.

- **AX-GIT-LSFILES** — `git ls-files -z --cached --others --exclude-standard`
  writes to stdout exactly the union of index paths and non-ignored untracked
  paths, each terminated by a NUL byte, unquoted, verbatim, and relative to the
  process working directory. Basis: the `git-ls-files` manual page and the
  transcripts captured under "Measured behaviour of the reference command".
  Boundary evidence: `tests/git_ls_files.rs` runs the real `git`.
- **AX-GIT-EXIT** — `git` exits `0` on success, including when the selection is
  empty, and non-zero with a diagnostic on stderr otherwise (`128` outside a
  repository). Boundary evidence: as above.
- **AX-CAPSTD** — `cap_std::fs_utf8::Dir::open_ambient_dir`, `read_to_string`,
  and `write` behave as documented. Already relied upon by existing code.
- **AX-RAYON-ORDER** — `par_iter().map(...).collect::<Vec<_>>()` preserves
  input order. Already relied upon by existing code and by
  `docs/architecture.md`.
- **AX-CLAP-GROUP** — a `clap::ArgGroup` with `multiple(false)` permits at most
  one member, and `requires = "<group>"` on an argument demands that at least
  one member be present. EP-M0 confirms this empirically before the design
  depends on it, because REQ-GIT-004 and REQ-GIT-005 both rest on it.

### Obligations

**INV-EXTSPEC-NORM** — parsing an extension specification is normalizing and
idempotent: `parse(s)` ignores a single leading dot, ignores surrounding
whitespace, and folds ASCII case, so `parse("md")`, `parse(".MD")` and
`parse("  .Md  ")` are all equal; and an empty segment, a segment that is only
a dot, or a segment containing a path separator or a NUL is rejected.

- Method: `rstest` parameterized tests for the accept/reject partitions and
  boundaries, plus a `proptest` for the normalization idempotence.
- Rationale: the accepting cases form a small, enumerable partition; the
  idempotence claim spans generated input and warrants a property.
- Domain: specifications built from arbitrary alphanumeric segments with
  randomized dot prefixes, case, and surrounding whitespace; plus the explicit
  rejection cases `""`, `","`, `"md,,markdown"`, `"."`, `"a/b"`.
- Artefact: `src/select/extensions.rs` `mod tests`.
- Evidence: `cargo test --lib select::extensions`. Red before the parser
  exists; green after.
- Non-vacuity: the generator must produce at least one specification per class
  — dotted, undotted, mixed case, padded — and the test asserts the classes
  were reached. Negative control: a `parse` that returns `Default::default()`
  regardless of input must fail the rejection cases and the multi-extension
  case.

**INV-EXT-SOUND** — every selected path's extension, lowercased, is a member of
the configured set.

**INV-EXT-COMPLETE** — every candidate whose lowercased extension is a member
of the configured set and which probes as `PathKind::RegularFile` appears in
the output.

- Method: one `proptest` asserting both directions over generated candidate
  lists, with a fake `PathProbe`.
- Rationale: these are the two halves of the selection contract, and stating
  only the first admits the catastrophically wrong implementation that selects
  nothing. Stating only the second admits the catastrophically dangerous
  implementation that selects everything. CON-SAFE-001 depends on both.
- Domain: candidate lists of 0 to 30 paths drawn from a pool mixing matching
  extensions, non-matching extensions (`rs`, `toml`, `png`), extensionless
  names, dotfiles, and nested directories; probe verdicts drawn from all three
  `PathKind` variants.
- Artefact: `src/select/policy.rs` `mod tests`.
- Evidence: `cargo test --lib select::policy`.
- Non-vacuity: the property classifies each generated case and the run must
  observe non-empty selections, empty selections, and at least one case where a
  matching extension is excluded solely because the probe said `Missing`.
  Negative controls, applied as deliberate mutations during EP-M1 and then
  reverted: returning `Vec::new()` must fail INV-EXT-COMPLETE, and returning
  the candidates unfiltered must fail INV-EXT-SOUND.

**INV-DEDUP** — the selected list contains no duplicate path.

- Method: `proptest` over candidate lists that are explicitly permitted to
  repeat entries, plus one `rstest` case reproducing the three-stage
  merge-conflict listing measured above.
- Rationale: duplication is not hypothetical; it is what `git ls-files`
  actually emits during an unresolved merge, and a duplicate under
  `--in-place` means formatting the same file twice concurrently.
- Domain: as INV-EXT-SOUND, with a generator biased to repeat paths.
- Artefact: `src/select/policy.rs` `mod tests`.
- Evidence: `cargo test --lib select::policy`.
- Non-vacuity: the property asserts that at least one generated case actually
  contained a duplicate candidate that survived the extension filter, so the
  check is not passing merely because duplicates never arose. Negative control:
  collecting into a `Vec` instead of a `BTreeSet` must fail this property.

**INV-ORDER-DET** — the selection is a deterministic function of the candidate
multiset: permuting the candidate list does not change the output, and the
output is sorted.

- Method: `proptest` comparing the selection of a generated list against the
  selection of a shuffled copy.
- Rationale: `git ls-files` output is not globally sorted, so without an
  imposed order the stdout concatenation and the snapshot tests would be
  unstable. This is an invariant over orderings, which is exactly what a
  property test is for.
- Domain: as INV-EXT-SOUND, with a permutation applied.
- Artefact: `src/select/policy.rs` `mod tests`.
- Evidence: `cargo test --lib select::policy`.
- Non-vacuity: the property asserts the generated permutation actually differed
  from the original in at least some cases, and that at least one output had
  two or more entries, so sortedness is a real claim. Negative control:
  preserving first-seen insertion order must fail this property.

**INV-PROBE-REGULAR** — a candidate is selected only if the probe reports
`PathKind::RegularFile`; `Missing` and `Other` are both excluded.

- Method: `rstest` parameterized over the three `PathKind` variants.
- Rationale: a finite, three-valued partition; exhaustive enumeration is
  practical and a property adds nothing.
- Domain: all three variants, each against a path with a matching extension.
- Artefact: `src/select/policy.rs` `mod tests`.
- Evidence: `cargo test --lib select::policy`.
- Non-vacuity: the matching-extension precondition is satisfied in all three
  cases, so a failure to exclude is attributable to the probe verdict alone.
  This covers REQ-GIT-006, that is, the tracked-but-deleted file and the
  submodule gitlink.

**INV-NUL-SPLIT** — splitting the adapter's NUL-delimited output is a faithful
inverse of Git's framing: for any list of non-empty byte strings none of which
contains a NUL, splitting their NUL-terminated concatenation returns that list;
and splitting an empty input returns an empty list rather than a list holding
one empty path.

- Method: `proptest` round-trip, plus `rstest` cases for the empty input, a
  single entry, a missing trailing NUL, and a non-UTF-8 entry.
- Rationale: this is the one place where a byte-level framing mistake would
  silently produce a wrong file set, and the empty-input case is the classic
  off-by-one that a naive `split(b'\0')` gets wrong by yielding `[""]`.
  CON-SAFE-001 depends on it.
- Domain: 0 to 20 byte strings of length 1 to 40 drawn from bytes excluding
  NUL, including bytes outside ASCII.
- Artefact: `src/select/git_ls_files.rs` `mod tests`.
- Evidence: `cargo test --lib select::git_ls_files`.
- Non-vacuity: the empty-list case is generated and asserted explicitly, and
  the generator produces non-UTF-8 sequences so the drop-and-count path is
  reached. Negative control: a plain `bytes.split(|b| *b == 0)` without the
  trailing-empty guard must fail the empty-input case.

**LEM-SELECT-SETEQ** — the set of files `--git` acts on equals
`{ p : p reported by the reference command, lowercased extension of p is in
the configured set, and p is a regular file in the working tree }`.

- Method: composition of INV-DEDUP, INV-EXT-SOUND, INV-EXT-COMPLETE, and
  INV-PROBE-REGULAR over the domain, discharged against the real interface by
  the behavioural scenarios and end-to-end tests in EP-M2 and EP-M3.
- Rationale: this lemma is what connects the unit-level invariants to the
  user-visible promise. Its left-hand side is only observable end to end, so it
  needs both halves: the properties fix the policy, and the repository-backed
  tests fix the axioms.
- Artefact: `tests/features/git_file_selection.feature` and
  `tests/git_ls_files.rs`.
- Evidence: `cargo test --test git_file_selection --test git_ls_files`.
- Non-vacuity: the scenario fixture deliberately contains a file in each
  equivalence class — tracked Markdown, untracked Markdown, ignored Markdown,
  tracked non-Markdown, and tracked-but-deleted Markdown — and asserts both
  that the first two changed and that the last three did not. A selection that
  is too narrow fails the first assertions; one that is too wide fails the
  second.

### Rigour not used, and why

**Bounded model checking (Kani)** is not used. The only obligation with a
plausible Kani shape is INV-NUL-SPLIT over small byte arrays, and a `proptest`
over bytes covers the same failure modes at a fraction of the cost. This
repository has no Kani harness infrastructure, and `src/fences_properties.rs`
already records that judgement for an analogous parsing obligation: "Kani is
deliberately not used: the project has no Kani dev-dependency or harness
infrastructure." Introducing it here would breach the dependency tolerance for
no additional assurance.

**Deductive proof (Verus)** is not used. LEM-SELECT-SETEQ is the one lemma this
change introduces, and it is a set comprehension over a finite filter with no
recursion, no unbounded arithmetic, and no inductive data structure — the
bidirectional properties INV-EXT-SOUND and INV-EXT-COMPLETE discharge it over
generated input, and the negative controls show they can fail. Against that,
Verus requires its own pinned toolchain, which would conflict with this
repository's `rust-toolchain.toml` pin of `nightly-2026-03-26`, and would
require rewriting the selection functions in Verus's `spec`/`exec` subset. A
proof of this lemma would also be close to a restatement of the filter
predicate, which the ExecPlan skill explicitly disallows as vacuous. If a later
change makes selection recursive or introduces ordering arithmetic, revisit
this judgement and record the change here.

## Milestones and plateaus

Each milestone ends in a coherent, validated repository state. There is no
compatibility machinery anywhere in this plan: the package is pre-1.0, `--git`
is a new flag with no existing consumers, and the one change to an existing
interface (the `--in-place` requirement, REQ-GIT-005) strictly widens what is
accepted, so no caller breaks and no shim is needed.

### EP-M0 — prototyping spike, argument grammar and BDD viability

- Outcome: throwaway evidence that `clap::ArgGroup` behaves as AX-CLAP-GROUP
  assumes and that `rstest-bdd` compiles and runs one trivial scenario on the
  pinned toolchain.
- Requirements: de-risks REQ-GIT-004, REQ-GIT-005, and the behavioural
  coverage that LEM-SELECT-SETEQ depends on.
- Acceptance evidence: `EV-M0-GRAMMAR`, a transcript showing
  `mdtablefix --git file.md` rejected, `mdtablefix --in-place` rejected, and
  `mdtablefix --git --in-place` accepted at parse time; and `EV-M0-BDD`, a
  passing one-scenario `rstest-bdd` run.
- Go/no-go: if `ArgGroup` does not give the required grammar, escalate with the
  alternative (a manual post-parse check in `main`, with hand-written
  diagnostics and their own snapshot test) rather than choosing it unilaterally
  — the diagnostics are user-visible. If `rstest-bdd` cannot be made to run,
  escalate under the iteration tolerance.
- Conformance check: no requirement is discharged here; nothing is promised to
  a user yet.
- Recovery: `git checkout -- .`; the spike is additive and discardable.
- Remaining gaps: everything.
- Compatibility decision: none required.

### EP-M1 — selection policy in the library

- Outcome: `mdtablefix::select` exposes `MarkdownExtensions`, `PathKind`,
  `PathProbe`, and `select_markdown_files`, fully tested with no filesystem and
  no repository. The CLI is unchanged and the tool behaves exactly as before.
- Requirements: REQ-GIT-002, REQ-GIT-003, REQ-GIT-006.
- Acceptance evidence: `EV-M1-POLICY` — `cargo test --lib select` passes,
  including INV-EXTSPEC-NORM, INV-EXT-SOUND, INV-EXT-COMPLETE, INV-DEDUP,
  INV-ORDER-DET, and INV-PROBE-REGULAR, each having been observed red first.
- Conformance check: the domain module imports nothing from `std::process`,
  `std::fs`, or `cap_std`; `select_markdown_files` takes its probe as a
  parameter; no public item outside `select` changed; trace links current.
- Recovery: the module is additive and unreferenced by the binary; revert the
  commit to return to the previous plateau.
- Remaining gaps: no candidate source, no CLI flag.
- Compatibility decision: none required (pre-1.0, new module).

### EP-M2 — adapters

- Outcome: `GitLsFiles` and `AmbientPathProbe` implement the ports.
  `GitLsFiles` spawns `git ls-files -z --cached --others --exclude-standard`,
  splits the NUL-delimited output, drops and counts non-UTF-8 paths, and maps
  spawn and exit failures to actionable errors. The CLI is still unchanged.
- Requirements: REQ-GIT-001, REQ-GIT-007.
- Acceptance evidence: `EV-M2-ADAPTER` — `cargo test --lib select::git_ls_files
  --test git_ls_files` passes, including INV-NUL-SPLIT and a test that builds a
  real repository in a `tempfile::TempDir` containing one tracked, one
  untracked, one ignored, and one deleted-but-tracked file and asserts the
  candidate set.
- Conformance check: CON-CAP-001 holds, because the adapter stats but never
  reads or writes file contents; CON-DEP-001 holds, because `Cargo.toml`
  `[dependencies]` is unchanged; error messages match the strings the
  documentation will quote.
- Recovery: revert the commit; EP-M1's plateau is intact.
- Remaining gaps: no CLI flag.
- Compatibility decision: none required.

### EP-M3 — command-line surface and end-to-end behaviour

- Outcome: `--git` and `--md-exts` exist, are documented in `--help`, are
  mutually exclusive with positional files, satisfy `--in-place`, and drive the
  existing `rayon` pipeline. The feature is fully usable.
- Requirements: REQ-GIT-004, REQ-GIT-005, and end-to-end discharge of
  LEM-SELECT-SETEQ and CON-SAFE-001.
- Acceptance evidence: `EV-M3-CLI` — `cargo test --test cli_git --test
  git_file_selection` passes and the `--help` snapshot is accepted;
  `EV-M3-DEMO`, the transcript under "Validation and acceptance".
- Conformance check: `src/main.rs` is under 400 lines; `open_file_parent`
  remains the only ambient read/write boundary; no runtime dependency added;
  every requirement above is now discharged with named evidence.
- Recovery: revert the commit; EP-M2's plateau is intact and the binary loses
  only the new flags.
- Remaining gaps: documentation.
- Compatibility decision: none required. REQ-GIT-005 widens an existing
  constraint, so no previously valid invocation becomes invalid.

### EP-M4 — documentation and decision record

- Outcome: `README.md`, `docs/users-guide.md`, `docs/architecture.md`,
  `docs/developers-guide.md`, `docs/contents.md`, and the new
  `docs/adrs/0006-git-file-selection.md` describe the feature, the boundary,
  and the rejected alternatives.
- Requirements: closes the documentation obligations in `AGENTS.md` and
  CON-DOC-001; publishes ADR 0006 as the upstream artefact this plan promised.
- Acceptance evidence: `EV-M4-DOCS` — `make markdownlint` and, if a Mermaid
  diagram was added, `make nixie` both pass.
- Conformance check: every design decision in the Decision log appears either
  in ADR 0006 or in a component document; `docs/contents.md` indexes the new
  ADR.
- Recovery: documentation-only; revert freely.
- Remaining gaps: none. Set Status to COMPLETE only after reconciling the
  Decision log and Surprises with ADR 0006.
- Compatibility decision: none required.

## Interfaces and dependencies

New library module tree, all under `src/select/`, with `src/select.rs` as the
module root. Every module opens with a `//!` comment, as `AGENTS.md` requires,
and every public item carries Rustdoc with an `# Examples` block written per
`docs/rust-doctest-dry-guide.md`.

In `src/select/extensions.rs`:

```rust
/// A case-insensitive set of Markdown file extensions, stored without dots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownExtensions(std::collections::BTreeSet<String>);

impl Default for MarkdownExtensions {
    /// Returns the default set: `md`, `mdc`, and `markdown`.
    fn default() -> Self;
}

impl MarkdownExtensions {
    /// Parses a comma-separated specification such as `"md, .mdown"`.
    ///
    /// # Errors
    ///
    /// Returns [`ExtensionSpecError`] when the specification is empty, holds an
    /// empty or dot-only segment, or holds a segment containing a path
    /// separator or a NUL byte.
    pub fn parse(spec: &str) -> Result<Self, ExtensionSpecError>;

    /// Reports whether `path` ends in one of these extensions.
    #[must_use]
    pub fn matches(&self, path: &camino::Utf8Path) -> bool;
}

/// The reason an extension specification was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionSpecError {
    Empty,
    EmptySegment { position: usize },
    InvalidSegment { segment: String },
}
```

`ExtensionSpecError` implements `std::fmt::Display` and `std::error::Error` by
hand. Do not add `thiserror`; CON-DEP-001 forbids it and the enum has three
variants.

In `src/select/policy.rs`:

```rust
/// What a candidate path turned out to be in the working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// A regular file that can be read and rewritten.
    RegularFile,
    /// Absent from the working tree, for example a staged deletion.
    Missing,
    /// Present but not a regular file, for example a submodule gitlink.
    Other,
}

/// Reports what a candidate path actually is. Implemented by adapters.
pub trait PathProbe {
    fn probe(&self, path: &camino::Utf8Path) -> PathKind;
}

/// Narrows candidates to the deduplicated, sorted set of Markdown files that
/// exist in the working tree.
#[must_use]
pub fn select_markdown_files<P>(
    candidates: &[camino::Utf8PathBuf],
    extensions: &MarkdownExtensions,
    probe: &P,
) -> Vec<camino::Utf8PathBuf>
where
    P: PathProbe + ?Sized;
```

`select_markdown_files` filters by extension first, inserts survivors into a
`std::collections::BTreeSet` — which discharges INV-DEDUP and INV-ORDER-DET
structurally rather than by a separate sort-and-dedup step — and only then
probes, so the filesystem is touched once per distinct Markdown candidate and
never for a `.rs` file.

In `src/select/source.rs`:

```rust
/// Candidate paths from a repository, with a count of paths that could not be
/// represented as UTF-8.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitFileListing {
    pub paths: Vec<camino::Utf8PathBuf>,
    pub skipped_non_utf8: usize,
}

/// Supplies candidate paths, relative to the working directory.
pub trait RepositoryFileSource {
    /// # Errors
    ///
    /// Returns an error when the underlying source cannot be consulted.
    fn list_candidates(&self) -> anyhow::Result<GitFileListing>;
}
```

In `src/select/git_ls_files.rs`:

```rust
/// Lists candidates by running `git ls-files`.
#[derive(Debug, Clone)]
pub struct GitLsFiles {
    program: std::ffi::OsString,
}

impl GitLsFiles {
    /// Uses `git` from `PATH`.
    #[must_use]
    pub fn new() -> Self;

    /// Uses a specific program. This is the seam that lets tests drive the
    /// failure paths without a real Git installation.
    #[must_use]
    pub fn with_program(program: impl Into<std::ffi::OsString>) -> Self;
}

impl RepositoryFileSource for GitLsFiles { /* ... */ }

/// Splits a NUL-terminated byte stream into UTF-8 paths, counting those that
/// are not valid UTF-8 rather than failing.
pub(crate) fn split_nul_delimited(bytes: &[u8]) -> GitFileListing;
```

The adapter runs exactly:

```plaintext
git ls-files -z --cached --others --exclude-standard
```

with the process's own working directory inherited, which is what makes
`--git` subtree-scoped per fact 3 above. It does not pass `--full-name`, does
not pass `--deduplicate` (the domain deduplicates, and relying on Git for it
would leave INV-DEDUP untested), and passes no pathspec.

In `src/select/fs_probe.rs`:

```rust
/// Probes the real working tree.
#[derive(Debug, Clone, Copy, Default)]
pub struct AmbientPathProbe;

impl PathProbe for AmbientPathProbe { /* std::fs::metadata */ }
```

`src/lib.rs` gains `pub mod select;` and re-exports nothing at the crate root,
keeping the new surface namespaced.

`src/main.rs` gains, in `Cli`:

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
    /// Select every Markdown file Git tracks, plus untracked files Git does
    /// not ignore, beneath the current directory
    #[arg(long = "git")]
    git: bool,
    /// Comma-separated file extensions to select under `--git`
    #[arg(long = "md-exts", value_name = "EXTS", requires = "git")]
    md_exts: Option<String>,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}
```

and one composition-root function that resolves the input list before the
existing `par_iter()` branches:

```rust
/// Resolves the files to act on: the positional list, or the Git selection.
fn resolve_input_files(cli: &Cli) -> anyhow::Result<Vec<PathBuf>>;
```

New `[dev-dependencies]` only, with explicit version ranges as `AGENTS.md`
requires:

```toml
rstest-bdd = "0.5"
googletest = "0.14"
pretty_assertions = "1.4"
```

`[dependencies]` is unchanged. That is the point of the chosen mechanism.

Assertion style, per `docs/rust-testing-with-rstest-fixtures.md` and the
`rust-unit-testing` skill: use `pretty_assertions::assert_eq` for structural
comparisons of path vectors, and `googletest` matchers (`assert_that!`,
`expect_that!`) where a matcher reads better than an equality — for example
`expect_that!(selected, unordered_elements_are![...])` and
`expect_that!(message, contains_substring("not a git repository"))`. Put
`#[googletest::gtest]` **before** `#[rstest]` on any test that uses
`expect_that!`.

## Behavioural specification

Create `tests/features/git_file_selection.feature` with exactly this content,
and bind it from `tests/git_file_selection.rs`. Keep the two synchronized: if
a scenario changes, change this plan too.

```gherkin
Feature: Select files from a Git repository

  As a maintainer of a Markdown-heavy repository
  I want mdtablefix to act on the repository's own Markdown files
  So that I do not have to enumerate them by hand or risk touching ignored files

  Background:
    Given a Git repository containing a committed file "docs/guide.md" with a broken table
    And a committed file "src/lib.rs" with a broken table
    And an untracked file "notes.md" with a broken table
    And an ignored file "build/out.md" with a broken table

  Scenario: Reformat every tracked and untracked Markdown file in place
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And the file "docs/guide.md" has a reflowed table
    And the file "notes.md" has a reflowed table
    And the file "build/out.md" is unchanged
    And the file "src/lib.rs" is unchanged

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

  Scenario: Skip a tracked file that was deleted from the working tree
    Given the file "docs/guide.md" is deleted from the working tree
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And the file "notes.md" has a reflowed table

  Scenario: List a conflicted file exactly once
    Given an unresolved merge conflict in the tracked file "docs/guide.md"
    When I run mdtablefix with "--git"
    Then the command succeeds
    And stdout contains the line "<<<<<<< HEAD" exactly once

  Scenario: Scope the selection to the current directory
    When I run mdtablefix from "docs" with "--git --in-place"
    Then the command succeeds
    And the file "docs/guide.md" has a reflowed table
    And the file "notes.md" is unchanged

  Scenario: Report a clear error outside a Git repository
    Given the working directory is not inside a Git repository
    When I run mdtablefix with "--git"
    Then the command fails
    And stderr contains "not a git repository"

  Scenario: Reject an unusable extension specification
    When I run mdtablefix with "--git --md-exts md,,markdown"
    Then the command fails
    And stderr contains "empty extension"

  Scenario: Reject combining --git with explicit file arguments
    When I run mdtablefix with "--git notes.md"
    Then the command fails
    And stderr contains "cannot be used with"

  Scenario: Reject --md-exts without --git
    When I run mdtablefix with "--md-exts md notes.md"
    Then the command fails
    And stderr contains "requires"
```

The step definitions share state through an `rstest` fixture that builds the
repository in a `tempfile::TempDir` and returns a handle carrying the temporary
directory and the last command's output. Do not use a global or a `static`;
`docs/rust-testing-with-rstest-fixtures.md` mandates fixture-based injection,
and a shared mutable global would make the scenarios order-dependent.

## Plan of work

### Stage A — understand and propose, no code changes

Read the documents and load the skills listed under "Context and orientation".
Re-run the reference-command transcripts under "Measured behaviour" against
the `git` on this machine and confirm they still hold. If any differs, stop:
the axioms have changed and the plan needs revising before code is written.

Validation: the transcripts match. Proceed.

### Stage B — red tests and the behavioural specification

Add the three `[dev-dependencies]`. Write the feature file
`tests/features/git_file_selection.feature` exactly as specified above, plus
the `tests/git_file_selection.rs` bindings. Then write the failing unit and
property tests for EP-M1 and EP-M2 before any production code exists. Confirm
each fails for the intended reason, which for the property tests means a
compilation failure naming the missing item, not a spurious pass.

Do not use `#[ignore]` to park a red test. Nothing in this plan is delivered
with an expected-failure marker still in place.

Validation: `make test` fails, and every failure names a missing item or a
missing behaviour from this plan. No unrelated test regresses.

### Stage C — implementation, milestone by milestone

Build EP-M1, then EP-M2, then EP-M3, each in its own commit, each turning its
own red tests green with the smallest change that does so. Run the negative
controls named in the Verification plan as deliberate temporary mutations, in a
scratch working copy, confirm the properties reject them, then discard the
mutation. Record in Progress that each control was exercised.

Validation: at each milestone boundary, `make check-fmt`, `make typecheck`,
`make lint`, and `make test` all pass, run sequentially, never in parallel.

### Stage D — refactor, documentation, and wider validation

Deliver EP-M4. Re-read `src/select/` against the refactoring heuristics in
`AGENTS.md`, splitting any function that has grown long or any file that has
approached the 400-line cap, as a separate commit after the functional one.

Validation: the full gate set plus `make markdownlint`, and `make nixie` if a
Mermaid diagram was added.

## Concrete steps

Run everything from the repository root. Per `AGENTS.md`, capture long output
through `tee` and read the log afterwards rather than relying on the terminal.

Confirm the starting state:

```console
$ git branch --show-current
git-option
$ git status --short
```

Expected: no output from `git status --short`.

Re-establish the axioms (Stage A):

```console
$ git ls-files -z --cached --others --exclude-standard | tr '\0' '\n' | head -3
.github/dependabot.yml
.github/workflows/ci.yml
.gitignore
```

Expected: NUL-delimited paths, exit status 0.

Run a single gate and keep the log:

```console
make test 2>&1 | tee "/tmp/test-mdtablefix-$(git branch --show-current).out"
```

Expected on success, at the tail of the log: `test result: ok.` for every test
binary and no `warning:` lines, because `make test` sets
`RUSTFLAGS="-D warnings"`.

Run the focused suites during Stage C:

```console
cargo test --lib select 2>&1 \
  | tee "/tmp/unit-select-$(git branch --show-current).out"
cargo test --test git_ls_files --test cli_git --test git_file_selection 2>&1 \
  | tee "/tmp/integration-git-$(git branch --show-current).out"
```

Review a `--help` snapshot change deliberately rather than accepting it
blindly:

```console
cargo insta review
```

Prefer delegating full gate runs to the `scrutineer` subagent, which runs them
sequentially, writes each log under `/tmp`, and returns a bounded report. When
it reports a failure, read the log it cites rather than re-running the gate.

## Validation and acceptance

Acceptance is behavioural. Build a scratch repository and observe the tool.

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
$ mdtablefix --git --in-place
$ cat docs/guide.md
| A | B |   |
| 1 | 2 |   |
| 3 | 4 |   |
```

Expected: `docs/guide.md` and `notes.md` are reflowed; `build/out.md` and
`keep.rs` are byte-identical to the input; exit status 0.

```console
$ cd / && mdtablefix --git ; echo "exit=$?"
error: running `git ls-files`: fatal: not a git repository (or any of the
parent directories): .git
exit=1
```

Expected: a non-zero exit and a diagnostic naming the cause. The exact wording
is pinned by an `insta` snapshot; if it differs from the above, update both the
snapshot and this transcript in the same commit.

```console
$ mdtablefix --git notes.md ; echo "exit=$?"
error: the argument '--git' cannot be used with '[FILES]...'
exit=2
```

Expected: `clap` rejects the combination before any file is touched.

Red-Green-Refactor evidence to record in Progress as work proceeds:

- Red: `cargo test --lib select` fails with `cannot find function
  \`select_markdown_files\`` before EP-M1's production code exists.
- Green: the same command reports `test result: ok.` after the minimal
  implementation.
- Refactor: the same command still reports `test result: ok.` after cleanup,
  followed by a clean run of the four gates.

Quality criteria — what "done" means:

- Tests: `make test` passes with no warnings, and the new suites
  `cargo test --lib select`, `--test git_ls_files`, `--test cli_git`, and
  `--test git_file_selection` all pass.
- Verification: INV-EXTSPEC-NORM, INV-EXT-SOUND, INV-EXT-COMPLETE, INV-DEDUP,
  INV-ORDER-DET, INV-PROBE-REGULAR, and INV-NUL-SPLIT are discharged by the
  named artefacts, each having been observed red first and each having rejected
  its negative control. LEM-SELECT-SETEQ is discharged end to end by the
  behavioural scenarios.
- Lint and typecheck: `make check-fmt`, `make typecheck`, and `make lint` all
  pass. Run them sequentially.
- Documentation: `make markdownlint` passes, and `make nixie` passes if a
  Mermaid diagram was added.
- Performance: no threshold. Selection adds one process spawn and one `stat`
  per distinct Markdown candidate, both negligible beside the existing per-file
  read and parse.
- Security: the tool now spawns a subprocess. It passes a fixed argument
  vector with no shell interpretation and no user-controlled arguments, so
  there is no injection surface. State this explicitly in ADR 0006.

## Idempotence and recovery

Every step is re-runnable. `mdtablefix --git --in-place` is idempotent on its
own output, which the existing `run_in_place` pattern in `tests/cli.rs` already
asserts for the positional path and which the new suite asserts for `--git`.
The gates are read-only apart from `cargo` build artefacts. `cargo insta
review` is the only interactive step; `cargo insta reject` undoes it.

The one destructive operation this feature can perform is rewriting files in
place, which is exactly what `--in-place` has always done. The acceptance
transcripts above use a throwaway repository under `mktemp -d`, so no working
repository is at risk. Never run `mdtablefix --git --in-place` against this
repository while validating; the resulting diff would be indistinguishable from
intended work.

Each milestone is a single commit, so `git revert` returns to the previous
plateau without touching later work.

## Progress

- [ ] Stage A: re-establish the reference-command transcripts on this machine.
- [ ] EP-M0: prototype the `clap::ArgGroup` grammar and confirm AX-CLAP-GROUP.
- [ ] EP-M0: prove one trivial `rstest-bdd` scenario runs on the pinned
      toolchain.
- [ ] Stage B: add the three dev-dependencies and write the feature file.
- [ ] Stage B: write the red unit and property tests for EP-M1 and EP-M2.
- [ ] EP-M1: implement `MarkdownExtensions`, `PathKind`, `PathProbe`, and
      `select_markdown_files`; discharge INV-EXTSPEC-NORM, INV-EXT-SOUND,
      INV-EXT-COMPLETE, INV-DEDUP, INV-ORDER-DET, INV-PROBE-REGULAR.
- [ ] EP-M1: exercise and revert the two negative controls.
- [ ] EP-M2: implement `GitLsFiles` and `AmbientPathProbe`; discharge
      INV-NUL-SPLIT and the repository-backed candidate test.
- [ ] EP-M2: exercise and revert the INV-NUL-SPLIT negative control.
- [ ] EP-M3: wire `--git` and `--md-exts` into `Cli` and the composition root.
- [ ] EP-M3: land the behavioural scenarios and the `--help` snapshot.
- [ ] EP-M4: write ADR 0006 and update `README.md`, `docs/users-guide.md`,
      `docs/architecture.md`, `docs/developers-guide.md`, `docs/contents.md`.
- [ ] Reconcile Decision log and Surprises with ADR 0006, then set Status.

## Surprises & discoveries

- Observation: `git ls-files -t` does not reliably flag a tracked file that has
  been deleted from the working tree; it reported `H` rather than `R` for such
  a file when `--deleted` was not also passed.
  Evidence: probing a repository with a committed then deleted `a.md` produced
  `H a.md`, not `R a.md`.
  Impact: status tags cannot substitute for a filesystem probe. This is why the
  design uses `PathProbe` rather than parsing `-t` output, and why
  INV-PROBE-REGULAR exists.

- Observation: `tests/table/` is not compiled by any Cargo target. No
  top-level `tests/*.rs` declares `mod table` or a `#[path]` to it, so roughly
  ninety tests in that directory are dead from Cargo's perspective.
  Evidence: an exhaustive grep of `tests/` for `mod table` and
  `#[path = "table/` returned no match; the only `mod table` in the repository
  is `src/lib.rs`'s unrelated production module.
  Impact: do not add tests to `tests/table/`; they would never run. Do not fix
  this as part of this plan either — it is out of scope. Note it and move on.

## Decision log

- Decision: obtain the file list by spawning `git ls-files -z --cached --others
  --exclude-standard` rather than using `git2` or `gix`.
  Rationale: the requirement is semantic equivalence with that exact command,
  and spawning it achieves equivalence by construction rather than by
  reimplementation. It adds no runtime dependency, inherits every Git
  configuration input (`core.excludesFile`, `.git/info/exclude`, per-directory
  `.gitignore`, sparse checkout) for free, and avoids a C toolchain. libgit2
  currently diverges from Git on nested `.gitignore` negation — a negation in a
  child directory fails to re-include a directory excluded by a parent — so
  `git2` would make "semantically equivalent" merely approximate. The cost is
  requiring `git` on `PATH`, which is acceptable for a flag whose entire
  premise is a Git working tree, and which REQ-GIT-007 turns into an actionable
  message.
  Date/Author: 2026-09-09, planning agent, confirmed by the requester.

- Decision: narrow the candidate set to Markdown extensions, defaulting to
  `md`, `mdc`, and `markdown`, with `--md-exts` to override.
  Rationale: `git ls-files` reports every file in the repository. `mdtablefix`
  has never filtered by extension, so an unfiltered `--git --in-place` would
  rewrite `.rs`, `.toml`, and binary files. A filter is a safety requirement,
  not a convenience, which is why CON-SAFE-001 is a constraint and why
  INV-EXT-SOUND and INV-EXT-COMPLETE are verified in both directions.
  Date/Author: 2026-09-09, requester.

- Decision: `--md-exts` replaces the default set rather than adding to it, and
  requires `--git`.
  Rationale: replacement is the predictable reading of "alternative
  extensions", and a separate add-versus-replace mechanism would be
  unjustified for the size of the set. Requiring `--git` is deliberate:
  positional paths are an explicit instruction from the user, and silently
  discarding a file the user named would be wrong.
  Date/Author: 2026-09-09, planning agent.

- Decision: sort the selection and deduplicate it, diverging from the raw
  output order of the reference command.
  Rationale: `git ls-files` emits the untracked pass before the cached pass, so
  its output is not globally sorted; and during an unresolved merge it emits a
  conflicted path once per index stage. Semantic equivalence is a claim about
  the set, not the sequence. A sorted, duplicate-free order makes stdout
  concatenation reproducible, makes snapshot tests stable, and prevents
  `--in-place` from formatting the same file twice concurrently. Implemented
  structurally with a `BTreeSet` rather than as a separate sort-and-dedup pass.
  Date/Author: 2026-09-09, planning agent.

- Decision: do not pass `--full-name`, so `--git` scopes to the current
  directory and its subtree.
  Rationale: this is the reference command's own behaviour, so preserving it is
  what equivalence means. It is also the more useful default, letting a user
  format one subtree without extra arguments. Documented explicitly in the
  users' guide, because it is the behaviour most likely to surprise.
  Date/Author: 2026-09-09, planning agent.

- Decision: drop and count paths that are not valid UTF-8, warning once on
  stderr, rather than failing the run.
  Rationale: `cap_std::fs_utf8` cannot represent them and the existing CLI
  already errors per-file on non-UTF-8 paths. Aborting a whole-repository
  operation because one stray filename exists elsewhere in the tree would be
  disproportionate, and matches neither the partial-failure posture of
  `report_results` nor the user's intent. The warning carries only a count, per
  CON-OBS-001.
  Date/Author: 2026-09-09, planning agent.

- Decision: model the boundary with two driven ports and one pure policy
  function, and change nothing else in the crate.
  Rationale: the boundary worth protecting is selection policy against process
  and filesystem access; splitting it makes the awkward cases — a merge
  conflict, a staged deletion, a non-UTF-8 path — reachable in unit tests
  without building repositories on disk. The rest of `mdtablefix` is a pure
  text pipeline with no infrastructure to isolate, so applying the pattern
  further would be a transplant rather than a boundary.
  Date/Author: 2026-09-09, planning agent.

- Decision: use `proptest` rather than Kani, and no Verus proof.
  Rationale: recorded in full under "Rigour not used, and why". Revisit if
  selection later becomes recursive or acquires ordering arithmetic.
  Date/Author: 2026-09-09, planning agent.

- Decision: the instruction to mark a roadmap entry as done on completion is
  recorded as **not applicable**.
  Rationale: this repository has no general-purpose roadmap. The two roadmap
  documents that exist are feature-scoped — one for ADR 0004's state-machine
  abstractions, one for the delivered parallel-processing work — and neither
  contains an entry for `--git`, `git ls-files`, or file selection. Creating a
  roadmap solely to tick it off would be ceremony. The implementor should not
  search for an entry to mark; ADR 0006 is the durable record instead.
  Date/Author: 2026-09-09, requester.

## Outcomes & retrospective

To be completed at EP-M4. Before setting Status to COMPLETE, reconcile every
entry in Surprises & discoveries and Decision log against ADR 0006 and the
component documents: a discovery that changes the design must appear in the
ADR; a purely mechanical difference may stay here with its rationale. Do not
mark this plan COMPLETE while any deviation from it remains unrecorded.

## Artefacts and notes

The reference-command transcripts under "Measured behaviour of the reference
command" and the acceptance transcripts under "Validation and acceptance" are
the primary artefacts. Add to this section, as work proceeds:

- the red and green output for each milestone's focused test command;
- the negative-control transcripts showing each property rejecting its seeded
  fault;
- the accepted `--help` snapshot;
- the final four-gate run.

Keep them short. Include only what proves success.
