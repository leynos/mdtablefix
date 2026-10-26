# Adopt `ortho_config` v0.10.0 for the `mdtablefix` command-line interface

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: IN PROGRESS — 2026-10-26, on branch
`issue-666-adopt-ortho-config-v0-10-0-for-the-mdtablefix-cli`, whose base
commit is `4a9bdf0e7b061d772ba4d11668e935e1c38e8300` ("Chore(deps): bump rstest
from 0.27.0 to 0.28.0 (#667)"). Stage 1 (import the authoritative
documentation) and Stage 2 (inventory and baseline) are discharged below; Stage
3 (record the configuration design) is the next milestone, EP-M0.

The issue that governs this plan is
[leynos/mdtablefix#666](https://github.com/leynos/mdtablefix/issues/666): adopt
the published `ortho_config` v0.10.0 release for the `mdtablefix` command-line
interface, and implement and validate the adoption rather than stopping at a
recommendation, a design note, or a dependency edit.

## Purpose / big picture

`mdtablefix` today is configured only by its command line. A user who runs
`mdtablefix --git --in-place --wrap --renumber --breaks --ellipsis --fences` in
`make fmt` types the same nine switches in every repository, and a team that
wants a shared house style has no way to record it once. After this change the
same run, with no configuration present anywhere, behaves exactly as it does
now, and a user who wants different defaults can write a `.mdtablefix.toml`
file, or set `MDTABLEFIX_WRAP=true`, or both, and have those settings apply to
every invocation in that project.

The user-visible behaviour is:

- A repository that has never heard of `mdtablefix` configuration sees no
  change at all: same flags, same defaults, same output, same exit statuses,
  same streams.
- A repository that adds a `.mdtablefix.toml` at its working-directory root,
  or sets `MDTABLEFIX_*` variables, or both, gets those settings as the
  starting point for every run; an explicit command-line flag still wins.
- A malformed or unreadable configuration file produces a clear diagnostic on
  standard error and exit status 2, distinct from the exit status 1 that
  reports formatting drift, and distinct again from the silent success of a run
  with no configuration file present.

The adoption is deliberately narrow. The library (`src/lib.rs` and its modules)
stays free of `ortho_config` entirely, so its public surface, its compilation
unit, and its minimum supported Rust version (MSRV) are untouched. Only the
binary-only modules that implement the command line change.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- The library must not depend on `ortho_config`, directly or transitively, in
  its public API. `src/lib.rs` and every module it declares (`breaks`,
  `code_emphasis`, `ellipsis`, `fences`, `footnotes`, `headings`, `io`, `lists`,
  `process`, `report`, `table`, `textproc`, `wrap`) must not import it. Only
  the binary-only modules (`command`, `driver`, `git_inputs`, `metrics`,
  `select`, plus any module this plan adds) may.
- Behaviour with no configuration present must be unchanged: the same flags,
  the same defaults, the same standard-output machine contract, the same
  diagnostics and summary on standard error, and the same exit statuses 0, 1,
  and 2.
- `tests/snapshots/cli_git_help.snap` must pass byte-for-byte, unmodified. It
  is the pinned contract for `--help`, and an added visible flag or a changed
  usage line breaks it. A configuration-related flag, if added at all, must be
  hidden from that snapshot.
- `Cli::parse_validated()` remains the only parser of process arguments. The
  derive's generated parser must never see `std::env::args_os()`, because it
  would accept `--flag=false` for the existing plain switches, which changes
  behaviour.
- No production code may read the process environment at run time. The
  taxonomy in [ADR 0012](../adrs/0012-environment-seam-taxonomy.md) forbids it,
  and `clippy.toml` fails the build on the six `std::env` methods. This
  constraint binds `ortho_config` at least as tightly as it binds this
  repository's own code: a configuration layer that calls `std::env::vars`
  directly would trip the lint, so the environment layer must be fed from a
  value the composition root obtained through the sanctioned seam.
- Tests must not mutate the process environment. Environment-shaped test cases
  use injected sources, and binary-level checks run in child processes with
  `Command::env` and `Command::env_remove`.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` must all pass. No warning may be suppressed, no test ignored, and
  `--ignore-rust-version` must not be used.
- `src/main.rs` must stay under the 400-line file cap that `AGENTS.md` sets.
  The integration belongs in a new module.
- Every new module begins with a `//!` module comment; every public item has
  rustdoc. Documentation prose uses en-GB-oxendict spelling.
- No merge and no release publication. The delivery is a draft pull request.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Scope: a net change of more than 1,200 lines across more than 12 files, or
  any change outside `src/command*.rs`, `src/main.rs`, a new `src/config.rs` (or
  `src/command/config.rs`), `Cargo.toml`, `Cargo.lock`, `tests/`, `docs/`, and
  the Makefile, stops and escalates.
- Interface: if any public library signature must change, stop and escalate.
  The library's surface is out of scope for this task.
- Dependencies: `ortho_config = "0.10.0"` and a direct `serde` dependency are
  authorised by the issue. Any further new dependency stops and escalates.
- Iterations: if a gate still fails after three fix attempts for the same
  defect, stop and escalate with the captured log.
- Time: no wall-clock limit is imposed. A milestone that stalls for more than
  four hours of active work on one defect stops and escalates.
- Ambiguity: where two readings of the issue would produce materially
  different user-visible behaviour, stop and present both with trade-offs.
- Host: if `/tmp` or the working filesystem fills, stop and report.

## Risks

Known uncertainties that might affect the plan. Identify these upfront and
update as work proceeds. Each risk should note severity, likelihood, and
mitigation or contingency.

- Risk: the derive emits `#[allow(...)]` attributes in its generated code, and
  this workspace denies `clippy::allow_attributes` and
  `clippy::allow_attributes_without_reason`. Severity: medium. Likelihood: low.
  Mitigation: EP-M0's compile spike settles it before any design document is
  written. Upstream's own workspace denies the same lints while its macros emit
  `#[allow(dead_code, reason = ...)]`, which strongly suggests macro-expanded
  attributes are exempt, but the spike proves it on this exact compiler.
- Risk: `ortho_config`'s environment merging reads the process environment
  when the derive's generated path is used, which would fail `make lint` against
  `clippy::disallowed_methods`. Severity: high. Likelihood: low. Mitigation:
  the plan never calls the derive's load entry points. It calls
  `merge_from_layers` with hand-composed layers, and the environment layer is
  built from an injected `ScanEnvSource`, so no generated code reads the
  environment.
- Risk: adding `ortho_config` introduces a large transitive dependency graph
  (figment, `serde_json`, `toml`, `uncased`, `xdg`, `ortho_l10n`) that could
  break the Windows CI job or the build-standard contract tests. Severity:
  medium. Likelihood: low. Mitigation: EP-M1 runs `make typecheck` and the
  `--target x86_64-pc-windows-msvc` cross-check immediately after the
  dependency edit, before any code uses it.
- Risk: a host that has an ambient `~/.config/mdtablefix/config.toml`, a
  `~/.mdtablefix.toml`, or `MDTABLEFIX_*` variables would perturb the existing
  CLI tests once discovery is wired in. Severity: medium. Likelihood: low.
  Mitigation: verified absent on this host on 2026-10-26. The existing CLI
  tests are child processes launched with `Command::cargo_bin`, whose
  environment inherits the parent's, so a stray ambient variable in a
  developer's shell could still bite. EP-M3 adds a test that proves a run with
  an explicitly empty injected environment still behaves identically, and the
  binary-level tests pass `env_remove` for every `MDTABLEFIX_*` key they
  consider.
- Risk: configuration discovery makes the tool's behaviour depend on the
  working directory, which could surprise a user who runs it from a
  subdirectory. Severity: low. Likelihood: medium. Mitigation: the design pins
  discovery to the working directory only, with no upward walk and no Git-root
  stop, and the user's guide states it plainly. The alternative — walking to
  the repository root — is explicitly rejected in ADR 0014 because it makes the
  same command behave differently depending on where it is typed.
- Risk: `--md-exts` is both a value-carrying flag and the input to a
  cross-argument validation (`--md-exts` requires `--git`). A configuration
  file that sets the extension list would make `--md-exts` "present" from
  clap's point of view only if clap saw it, which it will not; but the reverse
  mistake — treating a configured list as a typed flag, or a typed flag as a
  configured one — is easy. Severity: medium. Likelihood: medium. Mitigation:
  the design carries an explicit "was the flag typed on the command line" bit
  alongside the resolved value, so the `--git` requirement is judged on what
  the user typed and the extension set is judged on the merged value.
  `cli_default_as_absent` supplies the derive-side half of the same
  distinction. EP-M2 tests both directions.
- Risk: the `ortho_config` guides describe v0.10.0, and some of their text
  says "planned" or "not yet available". Copying such a statement into this
  repository's user's guide would document a feature that does not exist.
  Severity: low. Likelihood: medium. Mitigation: the imported guides are
  reference material with a provenance paragraph, not user's-guide text, and
  the version boundary is recorded in `Artefacts and notes`. This repository's
  own user's-guide section is written against what is actually wired in and
  proven by a test.
- Risk: the `ortho_config` crate's documented MSRV is Rust 1.89.0, and this
  repository declares `rust-version = "1.89"` already, but its pinned nightly is
  `nightly-2026-03-26`. Severity: low. Likelihood: low. Mitigation: no bump is
  expected. If the dependency's own manifest requires a newer compiler, the
  application crate's MSRV may rise and the dated-nightly pin may move; both
  are explicitly authorised by the issue.

## Progress

Use a list with checkboxes to summarize granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two ("done" vs. "remaining"). This section must always reflect the
actual current state of the work.

- [x] (2026-10-25 21:10Z) Verified issue #666's text against the GitHub API as
      `leynos`, confirming the six mandated stages, the behaviour-preservation
      clause, the MSRV authorisation, the draft-PR delivery, and the
      prohibition on merging and releasing.
- [x] (2026-10-25 22:40Z) Renamed the branch from `session/e8de3267` to
      `issue-666-adopt-ortho-config-v0-10-0-for-the-mdtablefix-cli` and set it
      tracking `origin/issue-666-adopt-ortho-config-v0-10-0-for-the-mdtablefix-cli`.
- [x] (2026-10-25 23:05Z) Renamed the Lody session to
      `Adopt ortho_config v0.10.0 for the mdtablefix CLI (#666)`.
- [x] (2026-10-26 09:15Z) Stage 1, tag identity: resolved `v0.10.0` in a fresh
      clone of `leynos/ortho-config` at `/tmp/ortho-config-src`. The tag object
      is `996e5a1c9f22ace4f55bf5dcc196d2c0524262bf`; it peels to commit
      `38812c563e3953a490e3efc202609f37e21a0517`, dated `2026-10-25T02:33:53Z`,
      subject "Prepare the 0.10.0 release and test every documented example
      (#620)". There is no divergence from the survey commit the issue names;
      the tag resolves to it exactly.
- [x] (2026-10-26 09:40Z) Stage 1, documentation identity: both guides read
      byte-for-byte from the immutable commit rather than from `main`, `HEAD`,
      or a latest-release URL.
      `docs/users-guide.md` at that commit is
      `84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575`
      (1,977 lines). `docs/v0-10-0-migration-guide.md` at that commit is
      `b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf`
      (498 lines). Both figures were re-measured on 2026-10-26 and match the
      cached copies used for the design research.
- [x] (2026-10-26 10:30Z) Stage 1, reading: the users' guide was read in full
      before any design decision, and the migration guide was read for the
      v0.9-to-v0.10 delta. See `Decision log` for the two facts that changed the
      design.
- [x] (2026-10-26 11:20Z) Stage 2, baseline identity: base commit
      `4a9bdf0e7b061d772ba4d11668e935e1c38e8300`; compiler
      `rustc 1.96.0-nightly (80d0e4be6 2026-03-25)`, `cargo 1.96.0-nightly
      (e84cb639e 2026-03-21)`, toolchain pin `nightly-2026-03-26`;
      workspace `version = "0.6.1"`, `edition = "2024"`,
      `rust-version = "1.89"`.
- [x] (2026-10-26 11:55Z) Stage 2, CLI inventory: the surface is
      `src/command.rs`'s `Cli` and `FormatOpts`. Recorded in full under
      `Context and orientation`.
- [x] (2026-10-26 12:20Z) Stage 2, caller inventory: the Makefile's `fmt`
      and `check-fmt` recipes, `.github/workflows/ci.yml`'s
      `install-mdtablefix` step, and the pinned help snapshot. Recorded under
      `Context and orientation`.
- [x] (2026-10-26 12:35Z) Stage 2, existing-artefact check: no prior issue-666
      branch, pull request, or adoption work exists on the remote. `origin` is
      an HTTPS URL, so every push uses the scrubbed-environment explicit-SSH
      URL flow.
- [x] (2026-10-26 12:50Z) Stage 2, ambient-configuration sweep: this host has
      no `~/.config/mdtablefix`, no `~/.mdtablefix.toml`, no
      `/etc/xdg/mdtablefix`, no `.mdtablefix.toml` in the working tree, and no
      `MDTABLEFIX_*` variables other than `LODY_GITHUB_REPO_FULL_NAME`, which
      does not match the prefix.
- [ ] EP-M0: compose the ExecPlan's design constraints into a throwaway
      compile spike that proves the derive survives this workspace's lint
      configuration and that `merge_from_layers` is reachable. Not started.
- [ ] EP-M1: import the two guides, record their provenance, and add both to
      `docs/contents.md`. Not started.
- [ ] EP-M2: write ADR 0014 and the design-document section; update
      `docs/contents.md` and the two stale sentences in
      `docs/execplans/check-option.md`. Not started.
- [ ] EP-M3: add the dependencies, the configuration module, and the wiring;
      keep the library and the pinned help snapshot untouched. Not started.
- [ ] EP-M4: the test suite — precedence tiers, injected sources, unhappy
      paths, and the no-configuration regression proof. Not started.
- [ ] EP-M5: user's and developer's guide updates, all gates green, draft pull
      request. Not started.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: the derive macro always emits its own hidden `clap` parser and
  never emits `impl CommandFactory` for the public configuration type. The
  consequence is that `SubcmdConfigMerge`, `load_and_merge_subcommand`, and
  every sibling entry point are unusable here, because they all require
  `T: OrthoConfig + Serialize + Default + CommandFactory`. Evidence:
  `ortho_config_macros/src/derive/generate/ortho_impl.rs` emits the
  `OrthoConfig` impl delegating to the generated CLI struct's
  `load_from_iter*`, and `ortho_config_macros/src/derive/generate/structs.rs`
  emits the hidden `clap::Parser` derive on a private mirror struct whose every
  field is `Option<T>`. No `CommandFactory` impl for the public type appears
  anywhere in the generated tokens. Impact: the integration seam is
  `merge_from_layers`, which the derive emits as an unconditioned inherent
  `pub fn` taking an iterator of `MergeLayer`. The plan is built on that
  method, with layers composed by hand from `ConfigFilePolicy`, an injected
  `ScanEnvSource`, and the existing `Cli` value.
- Observation: the derive's generated boolean command-line flags accept an
  optional `=<BOOL>` value, so `--wrap=false` parses as "explicitly false"
  rather than as an error. Evidence: the upstream test suite pins this in
  `ortho_config/tests/trybuild/boolean_cli_override.rs` and again in
  `documentation_examples/boolean_override.rs`. Impact: routing `mdtablefix`'s
  existing argv through the generated parser would change behaviour, because
  `mdtablefix --wrap=false` is today a `clap` parse error with exit status 2.
  This is the decisive reason to keep `Cli::parse_validated()` as the sole
  argument parser.
- Observation: the generated `const _` guard names
  `serde::de::DeserializeOwned` with the crate spelled unqualified, so `serde`
  must be a direct dependency of any crate that derives `OrthoConfig`. Evidence:
  `ortho_config_macros/src/derive/generate/ortho_impl.rs`, the `_assert_deser`
  guard emitted at the end of the impl block. Impact: `Cargo.toml` needs
  `serde` with the `derive` feature beside `ortho_config`, even though no code
  in this repository names `serde` directly. `ortho_config` does re-export its
  own derive macros, so `ortho_config_macros` need not be declared separately.
- Observation: `ortho_config`'s struct and field attribute parsers silently
  discard unknown keys (`discard_unknown`). Evidence:
  `ortho_config_macros/src/derive/parse/struct_attrs.rs` and `field_attrs.rs`
  accept and ignore unrecognised attribute arguments. Impact: a typo in
  `#[ortho_config(...)]` fails silently rather than at compile time. ADR 0014
  records the lenient policy, and the test suite keeps a compile-fail or a
  comment-level witness that a mistyped key is not caught.
- Observation: `ortho_config` v0.10.0 does not build with
  `default-features = false` (leynos/ortho-config#627), and the issue mandates
  default features enabled anyway. Evidence: the issue's own text, and
  `ortho_config/Cargo.toml`'s `default = ["serde_json", "toml"]` with
  unconditional use of `serde_json` in the declarative module. Impact: the
  dependency declaration is a plain caret requirement with default features.
  TOML is therefore the only format supported initially, which is the design
  decision recorded in ADR 0014.
- Observation: this host has no ambient `mdtablefix` configuration, so wiring
  discovery in will not by itself perturb the existing CLI tests. Evidence: a
  sweep on 2026-10-26 found no `~/.config/mdtablefix`, `~/.mdtablefix.toml`,
  `/etc/xdg/mdtablefix`, `.mdtablefix.toml` in the working tree, and no
  `MDTABLEFIX_*` variables. Impact: the baseline CLI gates can be re-run
  unchanged before and after discovery is wired in, and any failure is
  attributable to the change rather than to the host.

## Decision log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design choices.
For an architecture deviation, name the affected upstream identifiers, impacts,
options, required upstream-document changes, and approving authority. Set the
plan status to `BLOCKED` until the deviation is accepted.

- Decision: keep `Cli::parse_validated()` as the only parser of process
  arguments, and never call a derive-generated `load_from_iter*` entry point.
  Rationale: the generated parser would accept `--wrap=false` and every other
  `<bool>` spelling for the eight existing plain switches, and would add
  `--config-path` to `--help`. Both change behaviour for a user with no
  configuration present, which the issue forbids. It would also make the
  generated parser read the real process environment, which ADR 0012 forbids.
  Date/Author: 2026-10-26, planning agent.
- Decision: integrate through `ortho_config::MergeComposer` plus the
  derive-emitted `merge_from_layers`, with file layers resolved by
  `ConfigFilePolicy` and the environment layer built from an injected
  `ScanEnvSource`. Rationale: this is the only seam that satisfies both the
  behaviour constraint and ADR 0012. It keeps argv parsing and environment
  access in this repository's own composition root, where the sanctioned seams
  live, and uses `ortho_config` for what it is good at: layer ordering,
  merging, and reporting which source supplied a value. Date/Author:
  2026-10-26, planning agent.
- Decision: the configuration file is `.mdtablefix.toml`, discovered in the
  working directory only, with `~/.config/mdtablefix/config.toml`,
  `~/.mdtablefix.toml`, and the XDG system directories as lower-precedence
  predecessors, and `MDTABLEFIX_CONFIG_PATH` as an explicit selector that
  short-circuits discovery. Rationale: a working-directory project file is the
  convention `rustfmt.toml` and `.editorconfig` share, and it is the one
  location a user can predict without reading documentation. An upward walk to
  a Git root was rejected because it makes behaviour depend on where inside a
  tree a command is typed. Date/Author: 2026-10-26, planning agent.
- Decision: the environment prefix is `MDTABLEFIX_`, with `__` for nesting and
  comma-separated lists for vector-valued keys such as `MDTABLEFIX_MD_EXTS`.
  Rationale: this is what
  `CsvEnv::prefixed("MDTABLEFIX_").uppercase(true).split("__")` generates by
  default, so the hand-composed layer and any future derive-based path agree.
  Comma splitting is harmless for a list of extensions, and it lets a user set
  a one-line override in a shell. Date/Author: 2026-10-26, planning agent.
- Decision: precedence is defaults < configuration files < environment <
  command line, and the command line's *default* values count as absent.
  Rationale: this is the order the issue mandates and the order every
  reasonable configuration system uses. `cli_default_as_absent` is the
  `ortho_config` mechanism for the second half: a value the user did not type
  must not outrank a configured one. The hand-composed path applies the same
  rule by treating clap's `ValueSource::DefaultValue` as "not supplied".
  Date/Author: 2026-10-26, planning agent.
- Decision: no subcommand namespaces, no profiles and `--profile`, no
  localization of help or parse failures, and no Cargo-subcommand invocation
  form. Rationale: `mdtablefix` has no subcommands, so a namespace would be
  invented structure; the issue makes profiles and localization conditional on
  a need this tool does not have; and the binary is not named
  `cargo-mdtablefix` and is not invoked through Cargo. Date/Author: 2026-10-26,
  planning agent.

## Outcomes & retrospective

Summarize outcomes, gaps, and lessons learned at major milestones or at
completion. Compare the result against the original purpose. Note what would be
done differently next time. Before marking the plan `COMPLETE`, reconcile every
implementation discovery with the upstream artefacts listed in
`Conformance basis`: update a falsified Terms of Reference assumption and
impact-check the design; update the technical design or ADR for an
architectural change; or record a purely mechanical difference in
`Decision log`. An upstream change or deviation must not remain unrecorded or
unaccepted.

Filled in at each milestone. At the time of writing no milestone is complete.

## Context and orientation

`mdtablefix` is a Rust 2024 workspace with two packages: the root package,
which builds both the library `mdtablefix` and the binary `mdtablefix`, and
`test-macros`, a small proc-macro package used by the test suite. The root
package declares `version = "0.6.1"`, `rust-version = "1.89"`, and a
`nightly-2026-03-26` toolchain pin in `rust-toolchain.toml`.

The library is pure. `src/lib.rs` declares `breaks`, `code_emphasis`,
`ellipsis`, `fences`, `footnotes`, `headings`, `io`, `lists`, `process`,
`report`, `table`, `textproc`, and `wrap` publicly, and keeps `classify`,
`classify_kernel`, `frontmatter`, `html`, `list_interrupt`, and `reflow`
private or crate-visible. Nothing in that list may learn about configuration.

The binary is assembled in `src/main.rs`, which is capped at 400 lines and is
already near that cap. `src/main.rs` declares the binary-only modules `command`,
`driver`, `git_inputs`, `metrics`, and `select`. `command` holds the
command-line surface; `driver` holds the modes, the exit statuses, and the
per-file analysis loop; `git_inputs` holds the `--git` selection; `metrics`
holds the `metrics` facade; `select` holds the file-selection tree.

### The command-line surface, in full

`src/command.rs` declares:

```rust
#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables", long_about = None)]
#[command(group(clap::ArgGroup::new("inputs").args(["files", "git"]).multiple(false)))]
#[command(group(clap::ArgGroup::new("mode").multiple(false).requires("inputs")))]
pub struct Cli {
    #[arg(long = "in-place", group = "mode")] in_place: bool,
    #[arg(long = "check", group = "mode")] check: bool,
    #[arg(long = "diff", group = "mode")] diff: bool,
    #[arg(long = "list-files", group = "mode")] list_files: bool,
    #[arg(long = "git")] git: bool,
    #[arg(long = "include-untracked")] include_untracked: bool,
    #[arg(long = "md-exts", value_name = "EXT", value_delimiter = ',',
          default_value = "md,mdc,markdown", value_parser = parse_extension)]
    md_exts: Vec<String>,
    #[arg(long = "allow-conflicted")] allow_conflicted: bool,
    #[command(flatten)] pub opts: FormatOpts,
    pub files: Vec<PathBuf>,
}
```

`FormatOpts` is the eight-flag rule set flattened into `Cli`: `--wrap`,
`--renumber`, `--breaks`, `--ellipsis`, `--fences`, `--footnotes`,
`--code-emphasis`, and `--headings`, each a plain `bool`.
`From<FormatOpts> for Options` carries them into the library.

`Cli::parse_validated()` parses argv, then enforces one cross-argument rule that
`clap` cannot express on its own:

```rust
for (name, present) in git_only_flags(&cli, &matches) {
    if present && !cli.git {
        Self::command()
            .error(ErrorKind::MissingRequiredArgument, format!("{name} requires --git"))
            .exit();
    }
}
```

The four names are `--include-untracked`, `--allow-conflicted`, `--list-files`,
and `--md-exts`. The last is special: it counts as present only when
`matches.value_source("md_exts") != Some(ValueSource::DefaultValue)`, so the
compiled-in default `md,mdc,markdown` does not require `--git` but a typed
`--md-exts` does.

The getters `mode()`, `selects_from_git()`, `includes_untracked()`,
`allows_conflicted()`, and `extensions()` are the whole of what the rest of the
binary asks of the command line. `src/git_inputs.rs` calls `extensions()` at
line 140, `includes_untracked()` at lines 141 and 199, and
`allows_conflicted()` at line 195.

### The behaviour that must be preserved

`src/driver/reporting.rs` defines
`Mode::{Print, InPlace, Check, Diff, ListFiles}` and
`ExitStatus::{Success, Drift, Error}`, which map to exit codes 0, 1, and 2. The
standard-output stream carries machine-readable output only: formatted
documents in `Print` mode, selected paths in `ListFiles` mode, report lines in
`Check` mode, and unified diffs in `Diff` mode. Diagnostics and the summary go
to standard error.

`tests/snapshots/cli_git_help.snap` pins `--help` byte-for-byte, including the
usage line `Usage: mdtablefix [OPTIONS] [FILES]...`, the `[FILES]...` argument
description, every option from `--in-place` through `--headings`, `-h, --help`,
`-V, --version`, and the `--md-exts <EXT>` default rendering
`[default: md,mdc,markdown]`. `tests/cli_git.rs` around line 340 produces it by
running the built binary with `--help`, normalising `mdtablefix.exe` to
`mdtablefix`, and asserting with `insta`.

### Every caller of the command line

- `Makefile` line 15 sets `MDTABLEFIX ?= mdtablefix`, line 17 sets
  `MDTABLEFIX_SELECT = --git --include-untracked`, and line 18 sets
  `MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences`. The
  `fmt` recipe at line 72 runs
  `$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)` and the
  `check-fmt` recipe at line 77 runs
  `$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)`.
- `.github/workflows/ci.yml` installs `mdtablefix` at a pinned version for its
  `make check-fmt` job. It never passes a configuration flag.
- No sibling repository invokes `mdtablefix` with a configuration file.

### The ortho_config facts that shape the design

`ortho_config` v0.10.0 is the released version of a layered configuration
framework. Its pieces, as they bear on this plan, are:

- `ConfigDiscovery` and its builder: the named-candidate discovery engine.
  `ConfigDiscovery::builder(app_name)` defaults `config_file_name` to
  `config.toml`, the dotfile to `.<app_name>.<ext>` for the chosen extension,
  and the project file to the dotfile's name. Building it with no explicit
  project roots invokes a working-directory resolver exactly once; adding a
  project root suppresses that resolver. It does not walk upward and does not
  stop at a Git root.
- `ConfigFilePolicy`, built `from_builder`, with `.selectors([...])`,
  `.explicit_mode(ExplicitMode)`, `.automatic_mode(AutomaticMode)`, and
  `.scope_order([...])`. `resolve_layers()` returns a `FileLayerOutcome`. Under
  `AutomaticMode::FirstWins` the candidates are walked flat in preference order
  and the returned `origins` list is empty; under `StackScopes` the scopes are
  reverse-walked so the most-preferred candidate wins as the last layer.
- `ConfigPathSelector::cli(path)` and `ConfigPathSelector::env("NAME")` add
  explicit-path rungs ahead of automatic discovery. A selector match
  short-circuits automatic discovery entirely.
- `ExplicitMode::RequiredExclusive` makes a selected file that cannot be read a
  fatal error; `ExplicitMode::Optional` downgrades it to a reportable one.
- `CsvEnv::prefixed("MDTABLEFIX_")` reads a closed set of named variables, with
  `.uppercase(true)` and `.split("__")` matching the derive's own defaults.
  `.with_source(SharedScanEnvSource)` is how a test injects a `MapEnv` instead
  of the real process environment.
- `MergeComposer` with `push_defaults`, `push_file(value, Option<Utf8PathBuf>)`,
  `push_environment`, and `push_cli`, then `.layers()`.
- `merge_from_layers`, emitted by the derive as an inherent
  `pub fn merge_from_layers<'a, I>(layers: I) -> OrthoResult<Self> where
  I: IntoIterator<Item = MergeLayer<'a>>`.
  No `CommandFactory` bound.
- `OrthoError` with variants `CliParsing`, `File { path, source }`,
  `DefaultValueConversion`, `Gathering`, `Merge`, `Validation`,
  `UnknownProfile`, `InvalidProfileName`, `ReservedProfileName`,
  `ProfileForbiddenKey`, and `Aggregate`, plus `is_display_request`.
- The derive's struct attributes for this plan's needs are `prefix` and
  `discovery(app_name, env_var, ...)`; its field attributes are `default`,
  `skip_cli`, `cli_default_as_absent`, `merge_strategy`, and `rename`.

Two more facts matter enough to restate outside the decision log. The derive
emits `#[derive(clap::Parser, serde::Serialize, Default)]` on a hidden mirror
struct whose fields are all `Option<T>`, so the caller's struct does not need
to be constructible from command-line arguments — but `serde` must be a direct
dependency because the generated code names it. And the derive does not emit a
`clap::Command` for the public type, so nothing in this plan can ask
`ortho_config` to describe its own command line.

## Conformance basis

Treat this plan as a lightweight architecture contract. Name the exact Terms of
Reference revision, technical-design revision, ADRs, governing standards, and
relevant requirement or design-element identifiers. If an upstream artefact
does not exist, say so; do not invent one.

The upstream artefact is
[leynos/mdtablefix#666](https://github.com/leynos/mdtablefix/issues/666), read
from the GitHub API on 2026-10-25. It does not carry numbered requirement
identifiers, so this plan supplies stable ones:

```plaintext
TOR-GOAL-666-1  Adopt ortho_config v0.10.0 for the mdtablefix CLI and validate
                the adoption end to end.
                -> EP-M0, EP-M3, EP-M4 -> make test; draft PR
TOR-REQ-666-1   Import the authoritative documentation from tag v0.10.0's
                commit, byte-for-byte, with provenance.
                -> EP-M1 -> docs/ortho-config-users-guide.md; sha256sum check
TOR-REQ-666-2   Inventory the CLI and establish a baseline before design.
                -> Stage 2 (discharged) -> this document's Progress section
TOR-REQ-666-3   Record the configuration design in the design document and an
                ADR before implementing.
                -> EP-M2 -> docs/adrs/0014-configuration-layering.md
TOR-REQ-666-4   Implement the integration without changing behaviour when no
                configuration is present.
                -> EP-M3 -> tests/snapshots/cli_git_help.snap unchanged;
                             tests/cli_git.rs green
TOR-REQ-666-5   Keep reusable library crates free of OrthoConfig.
                -> EP-M3 -> cargo tree --invert ortho_config shows the binary
                             modules only; src/lib.rs unchanged
TOR-REQ-666-6   Test every precedence tier with injected sources and prove the
                no-configuration path is unchanged.
                -> EP-M4 -> tests/cli_config*.rs
TOR-REQ-666-7   Document the user-facing configuration surface and the internal
                integration boundary.
                -> EP-M5 -> docs/users-guide.md; docs/developers-guide.md
TOR-CON-666-1   No production process-environment reads (ADR 0012).
                -> EP-M3, EP-M4 -> make lint (clippy::disallowed_methods)
TOR-CON-666-2   No merge and no release publication.
                -> EP-M5 -> the deliverable is a draft pull request
```

The governing standards inside this repository are
[ADR 0012](../adrs/0012-environment-seam-taxonomy.md) (environment seam
taxonomy), [ADR 0009](../adrs/0009-check-and-diff-reporting.md) (the
exit-status contract), [ADR 0010](../adrs/0010-git-file-selection.md) (the
`--git` surface), the
[documentation style guide](../documentation-style-guide.md), and `AGENTS.md`.
The design document that EP-M2 updates is
[docs/architecture.md](../architecture.md).

## Verification plan

Co-design verification with the implementation rather than adding it after the
implementation structure is fixed. State every invariant over inputs, states,
orderings, or transitions that the planned implementation introduces or
preserves. State every lemma or intermediate contract required to connect those
invariants to the required behaviour.

### Invariants

- INV-NOCONFIG: for every argument vector that `Cli::parse_validated` accepts
  today, when no configuration file is discoverable and no `MDTABLEFIX_*`
  variable is set, the process writes the same standard output, the same
  standard error, and the same exit status as the base commit.
- INV-PRECEDENCE: for every settings key `k`, the resolved value is the first
  defined value in the order typed command-line flag (or an explicitly
  configured `--config-path` value), then environment, then the most-preferred
  configuration file, then the compiled-in default. "Typed" excludes a clap
  default.
- INV-TYPED: the `--git` requirement for `--include-untracked`,
  `--allow-conflicted`, `--list-files`, and `--md-exts` is judged on whether
  the user typed the flag, never on whether a configuration source supplied a
  value for the corresponding setting.
- INV-EXIT: exit status 0 means no drift and no error, 1 means drift in
  `--check` or `--diff` mode, and 2 means an error; a configuration error is
  always 2 and never 1.
- INV-STREAM: machine-readable output goes only to standard output;
  diagnostics, including configuration diagnostics, go only to standard error.
- INV-ENV: no production code reads the process environment through any of the
  six methods `clippy.toml` disallows.
- INV-LIB: no module reachable from `src/lib.rs` depends on `ortho_config`.

### Lemmas

- LEM-MERGE-ORDER: `MergeComposer`'s layer order is precedence order, and
  `merge_from_layers` folds layers left to right so the last writer wins.
  Proven upstream by the `MergeComposer` documentation test; this plan relies
  on it as an axiom and adds a test that pins the observable consequence.
- LEM-FIRSTWINS: under `AutomaticMode::FirstWins`, the candidate walk returns
  at most one layer, the most-preferred candidate that exists, so there is no
  ordering ambiguity to resolve.
- LEM-CLAP-DEFAULT: clap's `ValueSource::DefaultValue` is exactly the
  condition under which a flag was not typed, and it is available for every
  flag this plan consults.
- LEM-CONFIG-DISTINCT: a missing configuration file is not an error, a selected
  but missing file is an error under `RequiredExclusive`, and a malformed file
  is an error under either mode. The three cases are distinguishable by their
  resolved value and their error rendering.

### Obligations

- Obligation: INV-NOCONFIG.
  Method: parameterized test over the existing CLI suite plus a new
  differential case. Rationale: the existing suite already pins the observable
  behaviour of every mode; adding a new case that runs one representative
  invocation through both the base binary and the candidate binary, with an
  explicitly empty injected environment, catches a regression the per-feature
  tests might miss. Domain: the four file modes, `--git` selection with and
  without `--include-untracked`, `--md-exts` typed and defaulted, `--help`, and
  `--version`, in a temporary directory with no configuration file. Artefact:
  `tests/cli_config.rs`, plus the unchanged
  `tests/snapshots/cli_git_help.snap`. Evidence: `make test` passes with the
  snapshot unmodified; reverting the wiring makes the new differential case
  fail. Non-vacuity: the differential case asserts a non-empty standard output
  and a specific exit status for each invocation, so an empty-output run cannot
  pass by accident. The negative control is the base binary, which the test
  builds from `HEAD`'s tree only if the harness supports it; otherwise the
  control is the pre-change snapshot and the pre-change test suite, both of
  which must be unchanged.
- Obligation: INV-PRECEDENCE.
  Method: parameterized `rstest` table over source combinations, using injected
  sources only. Rationale: the partition is finite and small; a table makes
  each cell's meaning explicit and makes a missing combination visible. Domain:
  five keys (`wrap`, `renumber`, `headings`, `md_exts`, `git`) across five
  source combinations (defaults only, file only, environment only, file and
  environment, all three including the command line), with the file supplied
  both through discovery and through an explicit `MDTABLEFIX_CONFIG_PATH`.
  Artefact: `tests/cli_config.rs` with a `MapEnv` and a temporary TOML file.
  Evidence: the table fails before the wiring lands, because the configuration
  module does not exist; after the wiring, every cell passes. Non-vacuity: each
  cell asserts the *resolved* value, not merely that parsing succeeded, and the
  file-only and environment-only cells differ in value from each other and from
  the default so a layer silently dropped would change an assertion.
- Obligation: INV-TYPED.
  Method: parameterized test plus a child-process check. Rationale: the
  interesting cases are exactly two — a config-supplied `md_exts` with no typed
  `--md-exts`, and a typed `--md-exts` with no configuration — and both are
  cheap to enumerate. Domain: `--git` present and absent, crossed with
  `md_exts` supplied by configuration or by argv. Artefact:
  `tests/cli_config.rs`, with the child-process half in a
  `tests/cli_config_binary.rs` using `assert_cmd`. Evidence: the
  config-supplied case succeeds without `--git`; the typed case exits 2 with a
  message naming `--md-exts` and `--git`. Non-vacuity: the typed case asserts
  both the exit status and the message fragment, so a run that ignored the flag
  entirely would fail.
- Obligation: INV-EXIT and INV-STREAM.
  Method: child-process checks in `assert_cmd`. Rationale: the exit status and
  the stream split are process-level contracts; no in-process test can observe
  them faithfully. Domain: a malformed configuration file, an unreadable one, a
  selected but missing one, a valid one that sets `wrap`, and no configuration
  at all. Artefact: `tests/cli_config_binary.rs`. Evidence: the malformed and
  unreadable cases exit 2 with the diagnostic on standard error and nothing on
  standard output; the valid case exits 0 and formats as configured; the
  no-configuration case exits 0 with the base output. Non-vacuity: every case
  asserts standard output and standard error separately, so a diagnostic
  printed to the wrong stream fails.
- Obligation: INV-ENV.
  Method: the existing `make lint` gate, which denies
  `clippy::disallowed_methods`, plus the existing
  `tests/env_access_enforcement.rs` proof that the lint fires. Rationale: the
  policy is already enforced mechanically on every target, including the
  binary; a new environment read fails the gate. Domain: the whole workspace
  after the dependency and code changes. Artefact: `clippy.toml` and
  `Cargo.toml`'s workspace lint table, both unchanged. Evidence: `make lint`
  passes; introducing a `std::env::var` call in the new module makes it fail.
  Non-vacuity: `tests/env_access_enforcement.rs` runs Clippy over a fixture
  that calls all six methods, so the gate is proven to fire.
- Obligation: INV-LIB.
  Method: a repository test that reads the manifest and the module graph.
  Rationale: `cargo tree --invert` is not available inside a test, but the
  manifest's dependency list and the absence of `ortho_config` in any library
  module's source are both cheap to assert, and a future contributor adding an
  import would fail the test. Domain: `Cargo.toml`'s `[dependencies]` and every
  file under `src/` that is reachable from `src/lib.rs`. Artefact:
  `tests/cli_config_dependency_contract.rs`. Evidence: the test passes after
  EP-M3; adding `use ortho_config::` to `src/wrap.rs` makes it fail.
  Non-vacuity: the test asserts that `ortho_config` *is* present in the
  manifest, so it cannot pass on a tree where the dependency was never added.

### Axioms

- AXIOM-UPSTREAM-MERGE: `MergeComposer::layers()` yields layers in the order
  they were pushed, and `merge_from_layers` applies later layers over earlier
  ones. Trusted from upstream's documented interface and its own documentation
  test.
- AXIOM-CLAP-SOURCE: `clap`'s `ArgMatches::value_source` reports
  `DefaultValue` exactly for values it filled from a default.
- AXIOM-DERIVE-LINT: macro-expanded `#[allow]` attributes are not subject to
  this workspace's `clippy::allow_attributes` restriction. Discharged or
  falsified by EP-M0's compile spike rather than assumed.
- AXIOM-FILESYSTEM: a file that exists and is readable can be read; a file
  that does not exist cannot.

### Non-vacuity and negative controls

Every obligation above names both a witness and a fault that must be rejected.
The suite as a whole carries two standing negative controls: EP-M0's spike,
which exists only to falsify AXIOM-DERIVE-LINT, and the base-commit gate run
recorded in `Progress`, which must be green before the change and after it.

### Residual gaps

The `clap` default-source axiom is third-party behaviour and is not verified
here beyond the tests that depend on it. Windows path handling for
`MDTABLEFIX_CONFIG_PATH` is exercised by the existing Windows CI job, not by a
local test, because this host is Linux.

## Plan of work

Describe, in prose, the sequence of edits and additions. For each edit, name
the file and location (function, module) and what to insert or change. Keep it
concrete and minimal.

Structure as stages with explicit go/no-go points where appropriate:

- Stage A: understand and propose (no code changes). Discharged by Stages 1
  and 2, recorded above.
- Stage B: red tests or the smallest verification artefacts that fail for the
  expected reason. EP-M0 is the compile spike; EP-M4's tests are the red
  artefacts for the integration and are written before the wiring where
  practical.
- Stage C: implementation developed together with its verification. EP-M3.
- Stage D: refactor, documentation, and wider validation. EP-M5.

### Stage B detail

EP-M0 is a throwaway prototype. It adds a scratch module under
`/tmp/ortho-spike` — not in this repository — containing a tiny package that
depends on `ortho_config = "0.10.0"` and `serde` with `derive`, and declares:
one `#[derive(ortho_config::OrthoConfig, serde::Deserialize, serde::Serialize)]`
struct with a `prefix`, a `default`, a `skip_cli` field, and a
`cli_default_as_absent` field; one `merge_from_layers` call; and one `clap`-free
`main`. The package copies this repository's `clippy.toml` and its
`-D warnings` invocation, and runs `cargo clippy --all-targets -- -D warnings`.
Its acceptance criterion is the opposite of the normal one: the spike
*succeeds* only if the derive compiles under the repository's lint
configuration. If it fails on an `allow` attribute or on a lint this repository
denies, the plan escalates to `BLOCKED` and the decision log records the
conflict rather than working around it.

The spike is discarded after EP-M0. Nothing in it is copied into this
repository.

### Stage C detail

EP-M1, the document import. Add `docs/ortho-config-users-guide.md` and
`docs/ortho-config-v0-10-0-migration-guide.md`, byte-identical to the blobs at
commit `38812c563e3953a490e3efc202609f37e21a0517`, each with a provenance
paragraph in the style `docs/rstest-bdd-users-guide.md` uses: the repository,
the tag, the tag object, the commit, the upstream path, and the SHA-256. Record
the same facts in a separate provenance note inside the plan's
`Artefacts and notes` section so a re-sync is a plain diff against a recorded
commit. Add both files to `docs/contents.md` under "Reference material".

EP-M2, the design record. Add `docs/adrs/0014-configuration-layering.md` with
the `Status`, `Date`, `Context`, `Decision`, `Consequences`, and `References`
sections that `docs/adrs/0012-environment-seam-taxonomy.md` uses. Its decision
covers the configurable-versus-CLI-only split, the file name and discovery
locations, the TOML-only format, the `MDTABLEFIX_` prefix and CSV list
encoding, the precedence order including the absent-default rule, the
per-setting "was it typed" bit, the unknown-key policy, and the explicit
non-decisions (no subcommands, no profiles, no localization, no
Cargo-subcommand form). Add a section to `docs/architecture.md` after "Git file
selection" describing the configuration pipeline, and link it to ADR 0014. Add
the ADR to `docs/contents.md`. Rewrite the two stale sentences in
`docs/execplans/check-option.md` at lines 943–945 and 4277–4278, which assert
that this repository uses plain `clap` and not `ortho-config`.

EP-M3, the wiring. Add `ortho_config = "0.10.0"` and
`serde = { version = "1", features = ["derive"] }` to `Cargo.toml`, and update
`Cargo.lock`. Add a new binary-only module, `src/config.rs`, declared from
`src/main.rs` beside the existing `mod` lines, and add it to the module comment
that the file already carries. The module holds:

- `Settings`, the resolved configuration the driver consumes, as a plain struct
  with explicit fields and no `ortho_config` types in its signature.
- `ConfigFile`, the derive target, with `#[derive(ortho_config::OrthoConfig)]`,
  `#[ortho_config(prefix = "MDTABLEFIX")]`, and
  `#[ortho_config(discovery(app_name = "mdtablefix"))]`, whose every field is
  `Option<T>`.
- `Settings::resolve`, taking the parsed `Cli` and the `clap::ArgMatches` that
  produced it and returning `Result<Settings, ConfigError>`. It builds the
  discovery policy, composes the layers, and calls
  `ConfigFile::merge_from_layers`.
- `ConfigError`, a `thiserror` enum with variants for an unreadable selected
  file, a malformed file, an invalid value, and an environment-layer failure,
  each carrying the path or variable name that produced it.

`src/main.rs`'s `run()` changes from calling `Cli::parse_validated()` alone to
calling it and then `Settings::resolve`, mapping a `ConfigError` to
`eprintln!("mdtablefix: {}", error)` plus `ExitStatus::Error`. The rest of
`run()` reads fields from `Settings` instead of from `Cli`. `Cli` gains one
accessor, `typed_flags()`, returning the set of flags the user typed, so
`Settings` can carry the "was it typed" bit for `--md-exts`.

This edit must keep `src/main.rs` under 400 lines. If it cannot, the `run()`
decomposition moves to `src/driver/`, not to a longer `main.rs`.

EP-M4, the tests. Add `tests/cli_config.rs` for the in-process precedence and
typed-flag tables, `tests/cli_config_binary.rs` for the process-level exit and
stream contracts, and `tests/cli_config_dependency_contract.rs` for INV-LIB.
Add no new `#[allow]` and no new `#[expect]` beyond what ADR 0012 already
permits at the composition root.

EP-M5, the documentation and delivery. Add a "Configuration" section to
`docs/users-guide.md`, placed after "Selecting files from Git" and before "Exit
status", covering the file name, the discovery locations, the environment
variables, the precedence order, and the three error cases. Add a
"Configuration layering" section to `docs/developers-guide.md` near
"Environment access policy", covering the integration boundary, the
`merge_from_layers` seam, and the rule that the library never learns about
configuration. Run every gate. Commit. Push. Open the draft pull request.

## Milestones and plateaus

Define each implementation milestone as a coherent, validated repository state
that is safe to continue from if later work is postponed. A plateau requires
correctness and internal coherence, not simultaneous support for old and new
interfaces. For each milestone record:

- Identifier and outcome: EP-M0, a discarding prototype that decides whether
  the derive is viable under this repository's lints.
- Requirements and gaps: TOR-GOAL-666-1 by way of AXIOM-DERIVE-LINT.
- Acceptance evidence: the spike compiles clean under
  `cargo clippy --all-targets -- -D warnings` with this repository's
  `clippy.toml`, or the plan turns `BLOCKED`.
- Conformance check:
  - Requirements satisfied: none beyond the axiom it discharges.
  - Design still followed: no repository file changes.
  - Upstream assumptions still valid: `ortho_config` v0.10.0's derive is
    usable here.
  - No unapproved interface or dependency change: none, by construction.
  - No unapproved trust boundary: none.
  - Trace links current: yes.
- Recovery: delete `/tmp/ortho-spike`; nothing in the repository depended on
  it.
- Remaining gaps: everything after EP-M0.
- Compatibility decision: none.

- Identifier and outcome: EP-M1, the imported guides with provenance, in a tree
  where nothing else has changed.
- Requirements and gaps: TOR-REQ-666-1.
- Acceptance evidence: `sha256sum` over both files matches
  `84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575` and
  `b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf` apart from
  the added provenance paragraph; `make markdownlint` passes.
- Conformance check:
  - Requirements satisfied: TOR-REQ-666-1.
  - Design still followed: no design exists yet, and none is implied.
  - Upstream assumptions still valid: the tag resolution stands.
  - No unapproved interface or dependency change: none.
  - No unapproved trust boundary: none.
  - Trace links current: yes.
- Recovery: `git checkout` the two files and the contents index.
- Remaining gaps: design, wiring, tests, delivery.
- Compatibility decision: none.

- Identifier and outcome: EP-M2, the recorded design, in a tree where no code
  yet reads configuration.
- Requirements and gaps: TOR-REQ-666-3.
- Acceptance evidence: ADR 0014 exists, is listed in `docs/contents.md`, and is
  linked from `docs/architecture.md`; `make markdownlint` and `make nixie` pass.
- Conformance check:
  - Requirements satisfied: TOR-REQ-666-3.
  - Design still followed: the ADR records the decisions listed in
    `Decision log`.
  - Upstream assumptions still valid: the seam finding holds.
  - No unapproved interface or dependency change: none.
  - No unapproved trust boundary: the discovery locations and the environment
    prefix are named, which is the point of the milestone.
  - Trace links current: yes.
- Recovery: revert the documentation commit; no code depends on it.
- Remaining gaps: every code change.
- Compatibility decision: none.

- Identifier and outcome: EP-M3, the wiring, with the library and the help
  snapshot untouched and every gate green.
- Requirements and gaps: TOR-GOAL-666-1, TOR-REQ-666-4, TOR-REQ-666-5,
  TOR-CON-666-1.
- Acceptance evidence: `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` pass;
  `tests/snapshots/cli_git_help.snap` is byte-identical to the base commit;
  `cargo tree --invert ortho_config` names only the root package.
- Conformance check:
  - Requirements satisfied: TOR-REQ-666-4 and TOR-REQ-666-5.
  - Design still followed: ADR 0014's decisions are what the code implements.
  - Upstream assumptions still valid: `merge_from_layers` behaves as its
    documentation says.
  - No unapproved interface or dependency change: the two authorised
    dependencies are the only additions; the library's surface is unchanged.
  - No unapproved trust boundary: the environment layer is injected, and the
    file layer reads only the locations ADR 0014 names.
  - Trace links current: yes.
- Recovery: `git revert` the wiring commit. Because EP-M2 landed first and
  separately, the design record survives a revert of the code.
- Remaining gaps: the dedicated test suite and the delivery.
- Compatibility decision: none. `mdtablefix` is pre-1.0 and its only consumer
  is its own Makefile, which passes no configuration and is unaffected.

- Identifier and outcome: EP-M4, the test suite, with every obligation
  discharged.
- Requirements and gaps: TOR-REQ-666-6, TOR-CON-666-1.
- Acceptance evidence: the test count rises by the added cases, every one
  passes, and every negative control named in `Verification plan` is
  demonstrated once and recorded under `Artefacts and notes`.
- Conformance check:
  - Requirements satisfied: TOR-REQ-666-6.
  - Design still followed: the tests exercise the ADR's precedence order.
  - Upstream assumptions still valid: AXIOM-CLAP-SOURCE holds under test.
  - No unapproved interface or dependency change: none.
  - No unapproved trust boundary: none.
  - Trace links current: yes.
- Recovery: the tests are additive; reverting them cannot break the tree.
- Remaining gaps: documentation and delivery.
- Compatibility decision: none.

- Identifier and outcome: EP-M5, the documentation and the draft pull request.
- Requirements and gaps: TOR-REQ-666-7, TOR-CON-666-2.
- Acceptance evidence: the user's guide and developer's guide describe the
  shipped behaviour; all gates pass on the pushed tip; a draft pull request
  exists with `(#666)` in the title, `Closes #666` in the body, and the Lody
  session reference.
- Conformance check:
  - Requirements satisfied: TOR-REQ-666-7 and TOR-CON-666-2.
  - Design still followed: yes.
  - Upstream assumptions still valid: yes.
  - No unapproved interface or dependency change: yes.
  - No unapproved trust boundary: yes.
  - Trace links current: yes.
- Recovery: close the draft pull request without merging.
- Remaining gaps: the CodeRabbit review round, which is a delivery step rather
  than a repository state.
- Compatibility decision: none.

## Concrete steps

State the exact commands to run and where to run them (working directory). When
a command generates output, show a short expected transcript so the reader can
compare. This section must be updated as work proceeds.

All commands run from the worktree root,
`/home/leynos/.lody/repos/github---leynos---mdtablefix/worktrees/e8de3267-8e35-489c-bcde-d4343caab608`.

Provenance re-check, before and after the import:

```console
$ cd /tmp/ortho-config-src
$ git rev-parse v0.10.0 "v0.10.0^{}"
996e5a1c9f22ace4f55bf5dcc196d2c0524262bf
38812c563e3953a490e3efc202609f37e21a0517
$ git show "v0.10.0^{commit}:docs/users-guide.md" | sha256sum
84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575  -
$ git show "v0.10.0^{commit}:docs/v0-10-0-migration-guide.md" | sha256sum
b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf  -
```

The import itself, run from the worktree root:

```console
$ git -C /tmp/ortho-config-src show \
    "v0.10.0^{commit}:docs/users-guide.md" > docs/ortho-config-users-guide.md
$ git -C /tmp/ortho-config-src show \
    "v0.10.0^{commit}:docs/v0-10-0-migration-guide.md" \
    > docs/ortho-config-v0-10-0-migration-guide.md
$ wc -l docs/ortho-config-users-guide.md docs/ortho-config-v0-10-0-migration-guide.md
  1977 docs/ortho-config-users-guide.md
   498 docs/ortho-config-v0-10-0-migration-guide.md
```

The provenance paragraph is then prepended with an editor, and the checksum
check is re-run over the remaining body:

```console
$ tail -n +11 docs/ortho-config-users-guide.md | sha256sum
84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575  -
```

Gates, each logged under a `/tmp` file named for the action and the branch, so
the output can be reviewed after the run rather than scrolled through. The
same recipe shape is repeated for `lint`, `test`, `markdownlint`, and `nixie`:

```console
$ make check-fmt 2>&1 | tee /tmp/check-fmt-mdtablefix-issue-666.out
   Finished ...
```

The jump-start clause applies: full commit-gate runs are delegated to the
`scrutineer` subagent, which runs them sequentially and returns a bounded
report rather than the full logs.

The push uses the scrubbed-environment explicit-SSH URL flow, because `origin`
is HTTPS:

```console
$ GIT_SSH_COMMAND=/usr/bin/ssh /usr/bin/git push \
    git@github.com:leynos/mdtablefix.git \
    HEAD:refs/heads/issue-666-adopt-ortho-config-v0-10-0-for-the-mdtablefix-cli
$ GIT_SSH_COMMAND=/usr/bin/ssh /usr/bin/git fetch \
    git@github.com:leynos/mdtablefix.git \
    "+refs/heads/issue-666-adopt-ortho-config-v0-10-0-for-the-mdtablefix-cli:refs/remotes/origin/issue-666-adopt-ortho-config-v0-10-0-for-the-mdtablefix-cli"
$ git rev-parse HEAD
<the pushed commit>
```

## Validation and acceptance

Describe how to start or exercise the system and what to observe. Phrase
acceptance as behaviour, with specific inputs and outputs.

A user who has no configuration file and no `MDTABLEFIX_*` variable can run

```console
$ mdtablefix --check --git --include-untracked --wrap --renumber --breaks --ellipsis --fences
0 files left unchanged.
```

in this repository and see the same output and the same exit status as before
the change. The strongest witness is that `make check-fmt` — which runs exactly
that command — still reports no drift.

A user with a `.mdtablefix.toml` containing

```toml
wrap = true
renumber = true
```

can run `mdtablefix --in-place` and see paragraphs wrapped and ordered lists
renumbered without typing either flag. Adding `MDTABLEFIX_BREAKS=true` to the
environment also normalises thematic breaks; typing `--breaks=false` on the
command line overrides the variable and leaves them alone.

A user with `MDTABLEFIX_CONFIG_PATH=/nonexistent/config.toml` sees

```plaintext
mdtablefix: <diagnostic naming the path>
```

on standard error and exit status 2, with nothing on standard output. A user
with a malformed file sees a diagnostic naming the file and the parse failure,
and exit status 2.

A user with neither still sees exit status 0 for a clean run and exit status 1
from `--check` or `--diff` when a file would change.

Quality criteria (what "done" means):

- Tests: `make test` passes, and the new configuration tests fail before EP-M3
  and pass after it.
- Verification: INV-NOCONFIG, INV-PRECEDENCE, INV-TYPED, INV-EXIT, INV-STREAM,
  INV-ENV, and INV-LIB are each discharged by the artefact named in
  `Verification plan`.
- Lint/typecheck: `make lint` and `make typecheck` pass with warnings denied.
- Performance: no benchmark applies; the configuration resolution happens once
  per process and does not touch the per-file analysis path.
- Security: `make lint`'s `clippy::disallowed_methods` proves no new process
  environment read; no credential or network surface is introduced.

Quality method (how we check):

- `scrutineer` runs the five gates sequentially and returns the bounded
  report; failures cite a `/tmp` log.

## Idempotence and recovery

If steps can be repeated safely, say so. If a step is risky, provide a safe
retry or rollback path. Keep the environment clean after completion.

Every step is idempotent. The document import is a byte-for-byte copy from an
immutable commit, so re-running it produces the same bytes; the provenance
paragraph is the only hand-written part and is checked by the body checksum.
The dependency edit is a single manifest line plus a lockfile update, and
re-running `cargo update -p ortho_config` is harmless. The code edits are
ordinary file writes under version control; each milestone is its own commit,
so a bad step is `git revert`ed rather than repaired in place. The spike lives
in `/tmp/ortho-spike` and is deleted at the end of EP-M0, leaving nothing in
the repository. No destructive operation is planned.

## Artefacts and notes

Include the most important transcripts, diffs, or snippets as codefenced
examples. Keep them concise and focused on what proves success.

The tag resolution, re-verified on 2026-10-26:

```plaintext
tag object        996e5a1c9f22ace4f55bf5dcc196d2c0524262bf
peeled commit     38812c563e3953a490e3efc202609f37e21a0517
commit date       2026-10-25T02:33:53Z
commit subject    Prepare the 0.10.0 release and test every documented example (#620)
```

The two imported documents' identity:

```plaintext
upstream path     docs/users-guide.md
sha256            84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575
line count        1977

upstream path     docs/v0-10-0-migration-guide.md
sha256            b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf
line count        498
```

The version boundary, recorded so no reader mistakes an upstream plan for a
shipped feature:

- Both guides describe `ortho_config` v0.10.0 and nothing later.
- Upstream text that says "not yet available" or "planned" describes work that
  is not in v0.10.0 and is therefore not available to this repository.
- `ortho_config` v0.10.0 does not build with `default-features = false`
  ([leynos/ortho-config#627](https://github.com/leynos/ortho-config/issues/627)),
  so the dependency carries default features.
- The v0.10.0 MSRV floor is Rust 1.89.0, which this repository's
  `rust-version = "1.89"` already meets.

The baseline identity:

```plaintext
base commit       4a9bdf0e7b061d772ba4d11668e935e1c38e8300
rustc             1.96.0-nightly (80d0e4be6 2026-03-25)
cargo             1.96.0-nightly (e84cb639e 2026-03-21)
toolchain pin     nightly-2026-03-26
package version   0.6.1
edition           2024
rust-version      1.89
```

## Interfaces and dependencies

Be prescriptive. Name the libraries, modules, and services to use and why.
Specify the types, traits/interfaces, and function signatures that must exist
at the end of the milestone. Prefer stable names and paths such as
`crate::module::function` or `package.submodule.Interface`.

The manifest additions are exactly:

```toml
ortho_config = "0.10.0"
serde = { version = "1", features = ["derive"] }
```

`ortho_config` is a caret requirement with default features, as the issue
mandates and as its own feature table requires. `serde` is a direct dependency
because the derive emits code naming `serde::Serialize` and
`serde::de::DeserializeOwned`; it is used only by the configuration module.

In the new module `src/config.rs`, the following types must exist at the end of
EP-M3:

```rust
/// The configuration file's schema, as the derive sees it.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize, ortho_config::OrthoConfig)]
#[ortho_config(prefix = "MDTABLEFIX")]
#[ortho_config(discovery(app_name = "mdtablefix"))]
pub struct ConfigFile {
    /// Whether to wrap paragraphs, when a source supplies it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    // ... one Option-valued field per configurable setting
}

/// The settings the driver consumes, after every layer is merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The resolved formatting rule set.
    pub opts: command::FormatOpts,
    /// The resolved extension list for `--git` selection.
    pub md_exts: select::ExtensionFilter,
    /// Whether `--git` selection was requested.
    pub git: bool,
    /// Whether untracked files should be included.
    pub include_untracked: bool,
    /// Whether conflicting files should be rewritten.
    pub allow_conflicted: bool,
    /// The mode the command line selected.
    pub mode: driver::Mode,
    /// The positional file arguments.
    pub files: Vec<std::path::PathBuf>,
}

/// Everything that can go wrong while resolving configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A selected configuration file could not be read.
    #[error("cannot read configuration file {path}: {source}")]
    Unreadable { path: camino::Utf8PathBuf, source: std::io::Error },
    /// A configuration source supplied a value that cannot be used.
    #[error("invalid configuration value for {key}: {message}")]
    InvalidValue { key: String, message: String },
    /// The layered merge rejected the combination of sources.
    #[error("cannot resolve configuration: {0}")]
    Merge(#[from] std::sync::Arc<ortho_config::OrthoError>),
}
```

`Settings::resolve` must take the parsed `Cli` and the `clap::ArgMatches` that
produced it, because `ArgMatches` is the only place where "was this flag typed"
is recorded, and must return `Result<Settings, ConfigError>` without printing
or exiting. The composition root in `src/main.rs` prints and exits, preserving
the existing arrangement in which only `main.rs` and `command::parse_validated`
touch the process boundary.

`Cli` gains exactly one method:

```rust
/// Reports whether the user typed the given flag, as distinct from clap
/// filling it from its declared default.
#[must_use]
pub fn typed(&self, flag: &str) -> bool
```

or an equivalent that returns the set `Settings` needs, so the `--md-exts`
requirement can be judged on what the user typed.

The library must not gain any reference to `ortho_config`, `serde`, or
`src/config.rs`. `src/lib.rs` is unchanged by this plan.
