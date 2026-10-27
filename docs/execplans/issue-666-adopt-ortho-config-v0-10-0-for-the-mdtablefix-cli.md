# Adopt `ortho_config` v0.10.0 for the `mdtablefix` command-line interface

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: IN PROGRESS — 2026-10-27, on branch
`issue-666-adopt-ortho-config-v0-10-0-for-the-mdtablefix-cli`, whose base
commit is `4a9bdf0e7b061d772ba4d11668e935e1c38e8300` ("Chore(deps): bump rstest
from 0.27.0 to 0.28.0 (#667)"). Stages 1 and 2 (import the authoritative
documentation; inventory and baseline) are discharged below, and so are EP-M0
(the lint spike), EP-M1 (the document import), EP-M2 (the configuration
design), EP-M3 (the wiring), and EP-M4 (the test suite). EP-M5 — the
documentation and the draft pull request — is in progress: the documentation
half landed in `0d12ec2`, and the pull request is open as a draft. The plan
closes when the CodeRabbit review round is clear; it is never merged under this
plan.

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
  authorized by the issue. Any further new dependency stops and escalates.
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
  reference material with their provenance recorded in a separate note, not
  user's-guide text, and the version boundary is recorded in
  `Artefacts and notes`. This repository's own user's-guide section is written
  against what is actually wired in and proven by a test.
- Risk: the `ortho_config` crate's documented MSRV is Rust 1.89.0, and this
  repository declares `rust-version = "1.89"` already, but its pinned nightly is
  `nightly-2026-03-26`. Severity: low. Likelihood: low. Mitigation: no bump is
  expected. If the dependency's own manifest requires a newer compiler, the
  application crate's MSRV may rise and the dated-nightly pin may move; both
  are explicitly authorized by the issue.

## Progress

Use a list with checkboxes to summarize granular steps. Every stopping point
must be documented here, even if it requires splitting a partially completed
task into two ("done" vs. "remaining"). This section must always reflect the
actual current state of the work.

- [x] (2026-10-25 21:10Z) Verified issue #666's text against the GitHub API as
      `leynos`, confirming the six mandated stages, the behaviour-preservation
      clause, the MSRV authorization, the draft-PR delivery, and the
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
- [x] (2026-10-26 14:05Z) EP-M0, the compile spike. A throwaway crate at
      `/tmp/ortho-spike`, with `clippy.toml` and a lint table copied verbatim
      from this workspace, derives `OrthoConfig` on a schema of `Option`
      fields, composes three layers by hand, and merges them with
      `ConfigFile::merge_from_layers`. `cargo clippy --all-targets -- -D
      warnings` finished clean in 22.69s and the one unit test passed, so
      AXIOM-DERIVE-LINT is discharged by evidence rather than assumed. The
      spike was then deleted; nothing was copied into this repository.
- [x] (2026-10-26 14:40Z) EP-M1, the document import. Both guides were read
      from the immutable commit and written unmodified to
      `docs/ortho-config-users-guide.md` and
      `docs/ortho-config-v0-10-0-migration-guide.md`; their SHA-256 values
      still match the tag blobs exactly. Provenance lives in a separate
      `docs/ortho-config-v0-10-0-provenance.md`, so the copies stay
      byte-identical, and all three files are indexed from
      `docs/contents.md`.
- [x] EP-M2: write ADR 0014 and the design-document section; update
      `docs/contents.md` and the two stale sentences in
      `docs/execplans/check-option.md`. Done 2026-10-27:
      `docs/adrs/0014-configuration-layering.md` records the design,
      `docs/architecture.md` gains a "Configuration layering" section with an
      unnumbered `Figure:` caption, `docs/contents.md` lists the ADR, and both
      `docs/execplans/check-option.md` passages now carry a dated later note
      pointing at the adoption plan and the ADR. `make check-fmt` (scoped to
      the four files), `make markdownlint` (47 files, 0 issues), and
      `make nixie` all pass.
- [x] EP-M3: add the dependencies, the configuration module, and the wiring;
      keep the library and the pinned help snapshot untouched. Done
      2026-10-27. `Cargo.toml` gains `ortho_config = "0.10.0"` and
      `serde = { version = "1", features = ["derive"] }`; `src/config.rs` is
      the new binary-only module, at 394 lines against the 400-line cap, with
      every unit test deferred to EP-M4 in a `#[path]` file for that reason.
      `src/driver/run.rs` takes the execution half of `run()` so `src/main.rs`
      stays at 219 lines. Gate evidence, all run in this worktree on the same
      tree: `make typecheck` pass (48s), `make lint` pass (84s),
      `make check-fmt` pass, `make markdownlint` 47 files 0 issues,
      `make nixie` all diagrams validated, and `make test` exit 0 with
      **2872 passed, 0 failed** (901s). Acceptance evidence beyond the gates:
      `git diff --stat 4a9bdf0 -- tests/snapshots/` is empty, so the pinned
      `--help` snapshot is byte-identical to the base commit, and
      `cargo tree --invert ortho_config --depth 0` prints
      `ortho_config v0.10.0` and nothing else, so the root package is its sole
      consumer.
- [x] EP-M4: the test suite — precedence tiers, injected sources, unhappy
      paths, and the no-configuration regression proof. Done 2026-10-27, in
      three files rather than the one the plan named; the placement is
      deviation `a` in `Decision log`.
      `src/config_tests.rs` (395 lines, declared by a `#[path]` module on
      `src/config.rs`'s foot) holds the in-process tables: **57 tests**, being
      the 50-cell precedence table (5 keys by 5 source combinations by 2 file
      supplies) and seven named unhappy paths. `tests/cli_config_binary.rs`
      (422 lines) holds the process-level contracts: **14 tests**, pinning
      the no-configuration output character for character, the `--check`
      stream split, and every exit-2 configuration failure.
      `tests/cli_config_dependency_contract.rs` (189 lines) holds INV-LIB:
      **4 tests**. Both negative controls fired and were reverted; see
      `Surprises & discoveries`.
      **Later split (2026-10-27, CodeRabbit round two):** the single
      `src/config_tests.rs` above no longer exists as one file. The precedence
      table stays in `src/config_tests.rs` (217 lines), the failure cases moved
      to `src/config_error_tests.rs` (176), and the shared fixture to
      `src/config_test_support.rs` (127); `src/config.rs` itself is 336, with
      the schema in `src/config_schema.rs` (115). On the process side,
      `tests/cli_config_binary.rs` (210) now covers what a run formats and
      `tests/cli_config_failure_binary.rs` (164) covers the failures, sharing
      `tests/support/cli_config_fixture.rs` (156). The current test counts are
      **284** binary unit tests, **11 + 6** process-level tests, and **4**
      dependency-contract tests. No test was removed by the split.
      Gate evidence on the split tree, all run in this worktree: `cargo fmt
      --all` clean, `cargo clippy --workspace --all-targets --all-features --
      -D warnings` exit 0, `make check-fmt` exit 0 (47 files unchanged),
      `make markdownlint` 47 files 0 issues, `make nixie` all diagrams
      validated, and `cargo test --workspace` exit 0 with every suite green —
      **2,954 passed, 0 failed, 22 ignored** across 72 result lines, the
      ignored being doctests only.
- [x] (2026-10-27) CodeRabbit round one, addressed in `d17a309`.
- [x] (2026-10-27) CodeRabbit round two, adjudicated. Accepted: the
      malformed-discovered-file defect (fixed, with a process-level regression
      guard and its negative control), and the users' guide's flat precedence
      table (restated by scope). Rejected with reasons, all recorded in
      `Decision log`: the ADR file rename, both vendored-guide link retargets,
      and the `md_exts` validation deferral. Forced by the fix and the review:
      four file splits to stay under the 400-line cap, and
      `BINARY_ONLY` widened to five files. Documentation brought into line:
      `docs/users-guide.md` (scope column and the malformed-file sentence),
      `docs/developers-guide.md` (the five binary-only files and how the
      contract holds them), `docs/architecture.md` (the discovered-file
      failure), and `docs/adrs/0014-configuration-layering.md` (the promoted
      discovery errors, and the uniform-validation wording for the Git-only
      settings).
- [~] EP-M5: user's and developer's guide updates, all gates green, draft pull
      request. Documentation half done 2026-10-27 in `0d12ec2`; delivery
      outstanding. `docs/users-guide.md` gains a "Configuration" section
      (+84 lines) between "Selecting files from Git" and "Exit status", and
      `docs/developers-guide.md` gains a "Configuration layering" section
      (+46 lines) before "Coverage publication belongs to `main`". The section
      is longer than the plan's "three error cases" because measurement found
      four, and because the discovery list is stated as an ordered table rather
      than as prose: ten locations were probed one at a time in isolated trees,
      and every one distinguishes a configured run from an unconfigured one. The
      documentation gates on the committed tree: `make check-fmt` exit 0 (47
      files unchanged), `make markdownlint` 47 files 0 issues, `make nixie` all
      diagrams validated. Remaining: the full gate set on the final tree, the
      push, the draft pull request, and the CodeRabbit round.
- [x] (2026-10-27) CodeRabbit round three, all five findings resolved in the
      working tree. Fixed: the case-insensitive environment scrub in
      `tests/support/cli_config_fixture.rs`; the reachability half of
      `tests/cli_config_dependency_contract.rs`, now a module-graph walk in
      `tests/support/module_graph.rs` following `mod`, `#[path]` and
      `include!`. Documentation corrected: the users' guide's wrong-type
      paragraph (the diagnostic names neither file nor setting), the
      provenance note's re-sync check (both vendored files, separately), and
      this plan. The new composition root was sanctioned in `SANCTIONED_ROOTS`,
      `tests/env_access_suppressions.rs`, ADR-0012 and the developers guide in
      the same change, per that policy. Evidence: `cargo test --test
      env_access_suppressions --test cli_config_dependency_contract` gives 60
      passed, 0 failed; two mutations recorded under `Surprises &
      discoveries` prove the walker and the allowlist entry are load-bearing.
      Remaining: the full deterministic gate set, the commit, the push from a
      scrubbed environment, and CodeRabbit round four.
- [~] (2026-10-27) Full deterministic gate set on the round-three tree. First
      run: `cargo fmt --all` applied nothing, and `check-fmt`, `test` (2956
      passed), `markdownlint` (47 files, 0 issues), `nixie`, and `typecheck`
      all passed; `make lint` failed on two `-D warnings` errors in the new
      test code — `clippy::single-match-else` at `tests/support/module_graph.rs`
      and `clippy::needless-borrow` at `tests/cli_config_dependency_contract.rs`.
      Both fixed (the `match` became a let-else, the borrow was dropped); the
      affected suites were re-run and the mutation proof re-applied to the
      refactored walker. Remaining: the re-run of `make lint` and `make test`
      on the fixed tree, then the commit.
- [x] (2026-10-27) Gate set green on the fixed tree and committed as `afea508`.
      `make lint` 0 warnings, `make test` 2956 passed, `make check-fmt`,
      `make markdownlint` and `make nixie` all pass. Pushed over SSH from a
      scrubbed environment; remote head verified equal to local (`afea508`).
- [x] (2026-10-27) CodeRabbit round four on `afea508`, three findings. Accepted:
      `Settings::resolve`'s rustdoc wrongly said only a selected file or an
      empty search reports a discovery error, contradicting
      `promoted_discovery_errors`; the same claim was reported twice and both
      copies are corrected. Rejected: renaming ADR 0014 to the style guide's
      `adr-NNN` pattern, which repeats a round-two finding and is argued in
      `Decision log`. Remaining: the gates on the corrected tree, the commit,
      the push, and one further review round.
- [~] (2026-10-27) CodeRabbit round five on `1f4fe71`, two findings, both
      actioned in the working tree but **not yet compiled or gated**. Accepted:
      the users' guide used second-person pronouns ("the files you merged"),
      contrary to the documented style rule; now impersonal. Accepted: the
      module-graph walk skipped a macro invocation that could emit an unseen
      `#[path]` module. The walk now refuses a `macro_rules!` definition whose
      body writes `mod`, naming the macro rather than `macro_rules`, and refuses
      an invocation whose arguments carry a `mod` token. The rule was checked
      against the tree with a token-aware scan that strips strings and comments:
      zero invocations in `src/` carry a `mod` token, and zero `macro_rules!`
      definitions mention `mod`, so it costs nothing today. A negative control
      (`the_walk_refuses_an_unanalysable_module_declaration`) covers both the
      refusal and the non-offence of `anyhow!("no mod here")`.
      **Blocked:** compilation admission became unavailable mid-round.
      `cargo test` fails with `build-limits: the supervisor refused:
      launch-failed` even for `rustc --print`.
- [~] (2026-10-27) Admission root cause found, and it is not this worktree.
      `build-limits-status` alternates between `healthy` with slots free and
      `held 4` with other agents' groups running, so the supervisor itself is
      alive and admitting. The journal shows why a launch still fails: the
      supervisor reports `group g82025: cannot create
      buildlimits-2407454-g82025.scope: Call failed: Connection timed out`.
      That repeats every ~25 s. The supervisor's `CreateScope` call goes to the
      per-user systemd manager, which is hanging: `systemctl --user
      is-system-running` answers `degraded` at once, but `systemctl --user show`
      never returns. So each request is created, waits four 45-second
      `0 ahead` intervals, and is refused `launch-failed` — the `0 ahead` is
      honest, because the slot was free and it is the *launch* that failed, not
      the queueing. Per the compile-admission policy this is reported to the
      user, not worked around: no `RUSTC_WRAPPER` may be unset and no `rustc`
      may be called directly, and recovering the manager or the supervisor is
      the host operator's job.
      **Unblocked work done meanwhile:** `make markdownlint` (0 issues, 47
      files), `make nixie` (all diagrams valid), the Markdown half of
      `check-fmt` (`47 files left unchanged`), and the Rust half
      (`cargo fmt --all -- --check`, exact clean). The formatting check earned
      its keep: it rejected the two backslash-continued string literals the
      round-five edit introduced, which AGENTS.md forbids; both became
      `concat!()` with a named format argument, and the file is
      rustfmt-clean again. Remaining: `make lint`, `make test`, the commit,
      the push, and a further review round — all of which compile Rust.

- Decision (accepted): refuse a macro whose arguments write `mod`, rather than
  skip it. Rationale: the walker expands nothing, so an invocation emitting
  `#[path = "config_schema.rs"] mod settings;` reaches a binary-only file while
  every file the walk reads stays clean — the same hole `#[path]` resolution
  was added to close, one level deeper. The rule is a token test, not a name
  list, so it needs no maintenance as macros come and go, and it is narrow
  enough that `anyhow!("no mod here")` and `module!` are not offences. A
  `macro_rules!` definition is reported by `visit_item_macro` so the diagnostic
  names the macro instead of `macro_rules`. Residual gap, recorded rather than
  hidden: an invocation with an empty or `mod`-free body cannot be
  distinguished from any other by a walk that does not expand macros, so a
  macro defined elsewhere and invoked as `sneak!()` remains undetectable;
  closing it would need the compiler's own expansion, which this test cannot
  obtain. Date/Author: 2026-10-27, implementation agent.

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
  accept and ignore unrecognized attribute arguments. Impact: a typo in
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
- Observation: the derive's expansion names `clap` by absolute path, so a
  crate that derives `OrthoConfig` needs `clap` as a *direct* dependency with
  its `derive` feature, not merely a transitive one. Evidence: the EP-M0 spike
  failed with twelve `E0432`/`E0433` errors, including
  `cannot find attribute 'arg'`, until `clap` was added to its manifest.
  Impact: none for this repository, whose binary already depends on `clap` with
  `derive`; the requirement is recorded so the omission is not repeated if a
  second crate ever derives the macro.
- Observation: `ortho_config` re-exports `serde_json` (along with `figment`,
  `toml`, `uncased`, and `xdg`). Evidence: `ortho_config/src/lib.rs`'s
  `pub use` block, exercised by the spike's
  `use ortho_config::serde_json::json;`. Impact: the manifest addition stays at
  exactly two crates. Layer values are built with
  `ortho_config::serde_json::json!`, and `serde_json` need not be declared.
- Observation: macro-expanded `#[allow]` attributes do not trip
  `clippy::allow_attributes` or `allow_attributes_without_reason`, so the
  derive can be used under this workspace's lint table unchanged. Evidence: the
  EP-M0 spike's clean `cargo clippy --all-targets -- -D warnings` run under a
  `clippy.toml` copied verbatim. Impact: no lint exemption, no `#[expect]`, and
  no change to `Cargo.toml`'s lint table is needed for the derive itself;
  AXIOM-DERIVE-LINT is discharged.
- Observation: `AutomaticMode::FirstWins` — the framework's default — cannot
  express a working-directory file outranking a home file, because the flat
  candidate list puts every user-level location first. Evidence:
  `ConfigFilePolicy::from_builder` sets `automatic_mode: FirstWins` and
  `scope_order: [System, User, Project]`; `resolve_layers` short-circuits into
  `compose_scoped_layers_with_origins`, which for `FirstWins` returns
  `(self.compose_layers(), Vec::new())`; and `candidate_set()` orders required,
  explicit, selector, XDG, Windows, home, then project roots. Impact: the
  design uses `stack_scopes`, which is a decision the ADR records with its two
  accepted costs. This is also the only mode under which `origins()` is ever
  non-empty.
- Observation: the derive-generated parser gives every boolean switch a value
  placeholder, so `--wrap=false` would parse where this tool today rejects
  `--wrap=80`. Evidence: `tests/wrap/cli.rs` asserts the `--wrap=80` failure
  today, and the generate-side extraction reads
  `matches.value_source(...) == Some(ValueSource::CommandLine)`, which is a
  parse of an explicitly valued argument. Impact: it strengthens the decision
  to keep `Cli::parse_validated()` as the only argv parser; a `<BOOL>`
  placeholder would also break the pinned `tests/snapshots/cli_git_help.snap`.
- Observation: the generated `--config-path` flag is hidden from help by
  default, not shown. Evidence:
  `ortho_config_macros/src/derive/build/config_flag.rs` pushes `hide = true`
  whenever `config_cli_visible` is absent or false, and adds a help string only
  in the visible case. Impact: the argument for composing layers by hand rests
  on the boolean placeholder change, not on a new help line; the ADR states the
  hidden flag is accepted-but-undocumented.
- Observation: the compiled-in empty configuration does not reach the
  deserializer as `Null`. Evidence: the derive's `finish_tokens` normalizes a
  `Null` accumulator to `{}` with a doc comment naming exactly this case — "A
  layer list that supplied no values at all — which a prefixed, all-optional
  struct reaches whenever no environment variable is set and no configuration
  file exists". Impact: INV-NOCONFIG is preserved by the framework's own code
  path rather than by an empty-layer special case in this repository.
- Observation: a malformed file and an environment-layer failure both surface
  as `ConfigError::Merge`, not as a bespoke "invalid value" error. Evidence:
  `from_value_merge` is
  `serde_json::from_value(value).into_ortho_merge_json()`, and
  `OrthoError::Merge { source: Box<FigmentError> }` is the variant it yields.
  Impact: the plan's `InvalidValue` variant has no producer in this flow, and
  `Settings::resolve` should not invent one.
- Observation: `ExplicitMode::Optional` turns a malformed *selected* file into a
  `reportable_errors` entry rather than a `selected_error`, and `drain_errors`
  returns reportable errors even when layers loaded. Impact: choosing
  `RequiredExclusive` for `MDTABLEFIX_CONFIG_PATH` is what makes a broken named
  file fatal, and the ADR states that as the reason a user would name the file.
- Observation: the command line can enable a switch but not disable one, so
  the precedence order is observable in both directions only by mixing an
  environment `false` with a file `true`. Evidence: `mdtablefix --breaks=false`
  fails clap with
  `unexpected value 'false' for '--breaks' found; no more were expected`,
  because every switch is declared `#[arg(long = "...")]` with no value
  placeholder. Impact: a configured `true` is overridden by
  `MDTABLEFIX_<KEY>=false` and not by any typed flag; the precedence table
  carries that polarity for `wrap`, `renumber`, and `git` and the opposite for
  `headings`, whose file value is `false`. The plan's own acceptance text
  claimed a typed `--breaks=false` would override a variable, which was wrong
  and is corrected under `Validation and acceptance`.
- Observation: `MapEnv` is a usable injection seam for both halves of
  resolution, and one map can be shared between them. Evidence: the `EnvSource`
  and `ScanEnvSource` impls at `ortho_config/src/env_source.rs` around the
  `MapEnv` definition; `src/config_tests.rs::resolve` clones one `MapEnv` into
  an `Arc<dyn EnvSource>` and an `Arc<dyn ScanEnvSource>` and every precedence
  cell resolves. Impact: no test process environment is read or mutated, which
  is what ADR 0012's taxonomy requires and what the workspace's `clippy.toml`
  enforces.
- Observation: `ConfigDiscovery::builder` always resolves a Project root from
  `std::env::current_dir()`, and `with_project_root_resolver` is
  `#[cfg(test)] pub(crate)` upstream, so no consumer can substitute one.
  Evidence: the builder's field list and the method's visibility; the first
  draft of `src/config_tests.rs` carried an `environment.insert("PWD", ...)`
  that had no effect. Impact: an in-process cell asserting a compiled-in
  default also relies on this worktree's root holding no `.mdtablefix.toml`,
  which is the Project scope's only candidate. The test module's doc comment
  states that dependency, and a file appearing there would fail the cell loudly
  rather than silently. This is the one residual dependency EP-M4 could not
  remove. Correction (2026-10-27, CodeRabbit round two): this entry originally
  also named `mdtablefix/config.toml` at the root. `project_file_name` defaults
  to the dotfile name, so the nested spelling is never a Project candidate;
  measured with a valid file at `cwd/mdtablefix/config.toml` and nothing else,
  the run emitted its unconfigured output, and a malformed file there was not
  reported at all.
- Observation: `CsvEnv::prefixed` takes its prefix literally, so the trailing
  underscore is part of the argument. Evidence: `ortho_config/src/env.rs`'s
  `prefixed` constructor stores the string unmodified, and a draft that passed
  `"MDTABLEFIX"` matched nothing. Impact: `ENV_PREFIX` in `src/config.rs` holds
  `"MDTABLEFIX_"` and its doc comment says why, since the `prefix` attribute
  beside it spells the bare name.
- Observation: the environment value dialect is not uniform, and the failure
  mode for a list written without brackets is a type error rather than a parse
  error. Measured: `[txt,xml]`, `[md]`, and `["txt"]` all parse as list-literal
  strings, and a bare `rst,xml` also becomes a sequence; but `rst xml` is a
  plain string, which fails `Vec<String>` with
  `invalid type: string "rst xml", expected a sequence` and exit status 2.
  `[true]` and `true,false` fail a boolean the same way. Evidence:
  `tests/cli_config_binary.rs::an_untypable_environment_value_exits_two`, which
  pins both the status and the phrase. Impact: no extra validation is needed
  for the common spellings, and the diagnostic a user sees for the common
  mistake already says what is wrong.
- Observation: clap answers `--version` and `--help` before `Settings::resolve`
  is ever reached. Evidence: two draft tests in `tests/cli_config_binary.rs`
  drove `--version` to avoid needing a positional file, and both passed with
  exit 0 even though the configuration they supplied was invalid. Impact: every
  test asserting a configuration-resolution outcome now runs a real formatting
  job, which is why `an_extension_the_parser_rejects_names_its_key` and
  `a_configured_extension_list_is_not_a_typed_flag` feed `RAGGED` on standard
  input. Both doc comments state the reasoning, so the vacuous spelling is not
  reintroduced.
- Observation: the one `MapEnv` seam made both negative controls cheap, and
  both fired. Reversing the layer order in `merged_config` — pushing the
  environment before `policy.resolve_layers()` — failed exactly the ten
  `FileAndEnv` cells of the precedence table and no others (47 passed, 10
  failed), which is the shape a real precedence defect would take. Appending
  `const _PROBE: &str = "ortho_config";` to `src/wrap.rs` failed
  `no_library_source_names_the_framework` with
  `src/wrap.rs names ortho_config but is not one of the binary-only sources` (3
  passed, 1 failed). Both edits were reverted and `git diff` confirmed clean.
  Impact: the suites are proved to detect the defects they exist for, which is
  the non-vacuity evidence the Verification plan asks for.
- Observation: a wrong-typed value has no path in its diagnostic, unlike every
  other configuration failure. Measured on 2026-10-27:
  `mdtablefix: cannot resolve configuration: Failed to merge CLI with
  configuration: invalid type: string "yes", expected a boolean`,
  exit status 2, no path anywhere in the line. The reason is ordering rather
  than a missing field: the provider reads and merges the file before serde
  checks a type, so by the time the mismatch is known the value is a plain JSON
  node and its provenance is gone. Impact: the users' guide states that this
  one failure names neither the file nor the setting, and tells a user to
  search the merged files and variables for the offending value. The first
  draft of that section said the diagnostic named the file and the parse
  failure; measurement caught the error, and the bullet was split in two so the
  TOML-parse failure — which *does* name the path and the line and column — is
  not described in the same sentence as the type failure.
- Observation: the ten discovery locations all behave as documented, but only
  when probed one at a time. A first sweep that wrote every location into a
  single tree proved nothing: the file left by an earlier case was still there
  for the "no configuration" case, which then rendered a configured result.
  Rewritten as a case function that removes and recreates the tree per case,
  all ten discriminated correctly against the no-configuration output. Impact:
  the users' guide's discovery table rests on measurement rather than on
  reading the upstream candidate list, and the process suite's fixtures all
  point every base directory into a temporary tree for the same reason.
- Observation (CodeRabbit round two): the framework's optional-candidate rule
  hid a real defect, and the defect was invisible to every test written before
  it because each test gave the broken file *alone*. A malformed automatic
  candidate reports only when no layer loaded (`drain_errors` appends
  `ignorable_errors` under `if self.layers.is_empty()`), so the pre-existing
  error table proved the broken-alone case and never the broken-beside-a-good
  one. Measured in isolated trees: `xdg/mdtablefix/config.toml` valid plus
  `.mdtablefix.toml` broken exited 0 and formatted. Impact: the regression
  guard has two halves — the failing case and a negative control proving a
  promotion that reported every candidate would fail — and the users' guide and
  ADR now state that a discovered malformed file is fatal.
- Observation: `ortho_config`'s Project scope cannot be exercised in-process.
  `ConfigDiscovery::builder` always resolves a Project root from
  `std::env::current_dir()`, and `with_project_root_resolver` is
  `#[cfg(test)] pub(crate)` upstream. Impact: every assertion about the project
  scope — its precedence over the user scope, and the malformed-file case —
  lives in a child-process test whose working directory is the fixture root.
  The in-process tables name this dependency in their module documentation
  rather than pretending it does not exist.
- Observation: `--version` and `--help` never reach `Settings::resolve`; clap
  answers them first. A test that used either to observe a configuration
  outcome would pass whether or not the code under test were correct. Impact:
  the configured-extension test formats a standard input instead, and its
  documentation says why.
- Observation (CodeRabbit round three): the environment scrub in
  `tests/support/cli_config_fixture.rs` matched a fixed list of upper-case
  names, but OrthoConfig upper-cases a name before comparing it. Measured on
  2026-10-27: `MDTABLEFIX_ELLIPSIS=true`, `mdtablefix_ellipsis=true` and
  `MdTaBlEfIx_ElLiPsIs=true` each enabled the transformation, and a lower-case
  `mdtablefix_config_path` selected its file through the discovery path too.
  Impact: the scrub now enumerates the process environment and removes every
  name carrying the `mdtablefix_` prefix in any case, so a spelling the fixture
  has never heard of is still removed; any future key is covered without
  editing the fixture.
- Observation (CodeRabbit round three): a `#[path = "config.rs"] mod settings;`
  in `src/lib.rs` reaches a binary-only file without writing any text the
  dependency contract's scan could recognize. Impact: the reachability half is
  now a module-graph walk in `tests/support/module_graph.rs`, following `mod`,
  `#[path]` and `include!`, with an unresolvable declaration treated as an
  error rather than a skipped edge.
- Observation: the first mutation tried for that walk — appending
  `#[path = "config.rs"] mod settings;` to `src/lib.rs` — never reached the
  test, because `src/config.rs` names `crate::command`, `crate::select` and
  `crate::driver`, which only the binary root declares, so the *library target*
  failed to compile with `E0432`/`E0433` first. That proves the bypass is
  blocked but not that the walker fires. Re-running with
  `#[path = "config_schema.rs"]`, which has no `crate::` paths, made the
  library compile and the contract fail with
  `src/lib.rs reaches src/config_schema.rs, which belongs to the binary` while
  the textual half stayed green — so the structural half is load-bearing, not
  decorative.
- Observation (CodeRabbit round three): the wrong-type diagnostic names neither
  the file nor the setting. `wrap = "yes"` gives
  `invalid type: string "yes", expected a boolean` and exit 2, identically from
  a selected file, from a discovered file, and from `MDTABLEFIX_WRAP=yes`,
  because the merge precedes type checking and provenance is lost. Impact: the
  users' guide and this plan now say so and tell the reader to search the
  merged files and variables for the offending value, rather than promising a
  location the framework cannot supply.

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
- Correction to the entry above (2026-10-27): the intended precedence is not
  what the framework's default mode delivers, and the mode has to be chosen
  explicitly. `ConfigFilePolicy::from_builder` defaults to
  `AutomaticMode::FirstWins`, and `compose_scoped_layers_with_origins`
  implements it as `(self.compose_layers(), Vec::new())` — a flat scan of
  `candidate_set()`, whose order puts `$XDG_CONFIG_HOME`, the XDG system
  directories, and every home location ahead of the project root. A user with
  `~/.mdtablefix.toml` would therefore never see a working-directory file at
  all, and `origins()` would always be empty. The design therefore sets
  `automatic_mode = "stack_scopes"` with
  `scope_order = ["system", "user", "project"]`. `compose_scope` walks a
  scope's candidates with `.enumerate().rev()` over
  `.filter(|candidate| candidate.in_scope(scope))`, so within a scope the
  least-preferred location applies first and the most preferred last, while
  scopes apply system before user before project. The working-directory file
  wins, and a project file still overrides a user file. Two accepted costs
  follow: several files may contribute at once, and an automatic candidate that
  fails to load is only reported when no layer loaded (`drain_errors` appends
  `ignorable_errors` only when `layers.is_empty()`). Evidence:
  `ortho_config/src/discovery/scoped.rs`, `.../policy.rs::drain_errors`,
  `.../candidates/mod.rs::candidate_set`, and the vendored guide at lines
  512-543, which states the same rationale upstream. Date/Author: 2026-10-27,
  implementation agent.
- Amendment to the entry above (2026-10-27, CodeRabbit round two): only the
  first of the two costs stays accepted. Several files contributing at once is
  still the design. The second cost — an automatic candidate that fails to load
  being reported only when no layer loaded — is refused, because it lets a
  malformed project file fall back silently to the user file beneath it. See
  the promoted-discovery-error decision below.
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
- Decision: the imported guides stay byte-identical to the tag blobs, and
  their provenance lives in `docs/ortho-config-v0-10-0-provenance.md` rather
  than in a header inside each guide, departing from the
  `docs/rstest-bdd-users-guide.md` precedent. Rationale: the issue requires
  provenance *and* an auditable identity for each imported file, and those two
  requirements conflict the moment text is prepended, because the hash then
  covers the added paragraph rather than upstream's document. A separate note
  satisfies both: `sha256sum` of the copy matches the recorded upstream blob,
  and the re-sync procedure is a plain diff. Date/Author: 2026-10-26,
  implementation agent.
- Decision: ADR 0014 mirrors ADR 0012's shape — a bare
  `# Configuration layering` title, `Status`, `Date`, prose `Context`,
  `Decision` with `###` subsections, bulleted `Consequences`, and bulleted
  `References` — rather than the template
  `docs/documentation-style-guide.md:355-455` prescribes, and it lives in
  `docs/adrs/` with a four-digit prefix. Rationale: EP-M2 asks for ADR 0012's
  sections, and all thirteen existing records follow the local form; a bare
  title and a missing `## Context and problem statement` heading are the
  established convention here. The deviation is recorded inside the ADR's
  `## Context` rather than left silent, and changing the convention is a
  repository-wide decision this plan does not take. Date/Author: 2026-10-27,
  implementation agent.
- Decision: the new `docs/architecture.md` section carries an unnumbered
  `Figure:` caption, not `_Figure 6:_`. Rationale:
  `docs/execplans/git-option.md` line 378 reads "…\"Atomic in-place writes\",
  with Figure 6", so inserting a sixth numbered figure ahead of that section
  would silently misname it. `docs/architecture.md` already has two unnumbered
  `Figure:` precedents (the wrap-tokenizer flowchart and the `wrap_text`
  sequence). Renumbering was the rejected alternative;
  `docs/execplans/check-option.md:4641-4642` records an earlier renumbering as
  the option not taken. Date/Author: 2026-10-27, implementation agent.
- Decision: no `merge_strategy` attribute appears on the `md_exts` field of the
  configuration schema. Rationale:
  `ortho_config_macros/src/derive/build/override/mod.rs:76` rejects the
  attribute on any field that is not `Vec<_>` or `BTreeMap<_, _>`, so
  `Option<Vec<String>>` — which every schema field must be, so that an absent
  key stays distinguishable from an empty list — cannot carry it. No attribute
  is needed: `merge_value` replaces the whole value for any non-object, so a
  higher layer's array replaces the lower layer's, which is the precedence rule
  INV-PRECEDENCE requires. Date/Author: 2026-10-27, implementation agent.
- Decision (EP-M3 adjustment `a`): `Cli::parse_validated()` now returns
  `(Self, clap::ArgMatches)` rather than `Self`. Rationale: `ArgMatches` is the
  only record of where each value came from, and `Settings::resolve` needs it
  to judge a flag by whether it was typed. The alternative — parsing twice —
  would run clap twice and could report a parse error after configuration had
  already been read. It remains the only parser,
  `Self::command().get_matches()` is still called exactly once, and the four
  `--git` requirement checks stay in the same place and in the same order, so
  argv behaviour and the `--help` snapshot are unchanged. Date/Author:
  2026-10-27, implementation agent.
- Decision (EP-M3 adjustment `b`): `command::FormatOpts` gains `Debug`,
  `PartialEq`, and `Eq`. Rationale: the plan's `Settings` shape derives
  `Debug, Clone, PartialEq, Eq`, and `Settings` holds a `FormatOpts`. The three
  derives are behaviour-neutral — `clap::Args` reads only field names, types,
  and attributes — and they let a test compare resolved settings as values.
  Date/Author: 2026-10-27, implementation agent.
- Decision (EP-M3 adjustment `c`): the execution half of `run()` moves to a new
  `src/driver/run.rs`, declared as `mod run` from `src/driver.rs`. Rationale:
  `src/main.rs` was 390 lines against a 400-line cap, and the configuration
  wiring adds an import, a resolution step, and error mapping. The plan
  authorizes exactly this move ("the `run()` decomposition moves to
  `src/driver/`, not to a longer `main.rs`"). `src/main.rs` is now 219 lines
  and holds only the composition root: `failed_run`, selection, `run`, and
  `main`. `failed_run` stays at the crate root because
  `src/metrics_tests.rs:254` names it as `crate::failed_run`;
  `report_selection` stays because `src/main_git_selection_tests.rs` names it
  through `super`; and the three document-boundary helpers are re-exported from
  `driver` under `#[cfg(test)]` so `src/main_tests.rs` keeps its
  `super::{format_stdin, open_file_parent, render_stdin_output}` imports
  unmodified. Date/Author: 2026-10-27, implementation agent.
- Decision (EP-M3 adjustment `d`): `ConfigError` has two variants,
  `InvalidValue { key, message }` and `Merge(#[from] Arc<OrthoError>)`, rather
  than the plan's four. Rationale: `OrthoError` has no unreadable-file or
  invalid-value variant, and both a malformed file and a failed environment
  extraction arrive as `OrthoError::File` or `OrthoError::Merge` after the
  framework has already read the file; a `Unreadable` variant would have to
  pre-read every candidate to earn its name, duplicating work the framework
  does and inventing a distinction it does not promise to keep. `InvalidValue`
  is kept because this module *does* have a producer for it: an `md_exts` value
  from a file or the environment never passed through clap's `value_parser`, so
  `extension_filter` parses it and reports the key. The path a selected file
  error names is inside the wrapped error's own message, so the diagnostic
  still says which file to fix. Date/Author: 2026-10-27, implementation agent.
- Decision (EP-M3 adjustment `e`): the typed-flag accessor is a private
  `typed_bool(matches, id, value)` plus `is_typed(matches, id)` in
  `src/config.rs`, not a `Cli` method. Rationale: the plan offers
  `typed(&self, flag: &str)`, `typed_flags()`, "or an equivalent"; the
  provenance itself lives in `ArgMatches`, and `Cli` deliberately does not hold
  it, so a `Cli` method would have to be handed the matches it was built from
  and would add a field or a parameter to the command-line type for
  configuration's benefit. The two helpers read
  `matches.value_source(id) == Some(ValueSource::CommandLine)`, the same test
  the framework's generated loader uses (`build_cli_push_tokens`). `Cli` keeps
  its four accessors for what the command line *said*, and configuration is the
  only reader of what the user *typed*. Date/Author: 2026-10-27, implementation
  agent.
- Decision: `Settings::resolve` is split into `resolve` and
  `resolve_with_sources`. Rationale: EP-M4 must drive discovery, the selector,
  and the merge layer from one fixed map without touching the process
  environment, and ADR 0012 requires the sources to be injected rather than
  read. `resolve` names `process_env_source()` and `process_scan_env_source()`
  once, at the composition root; `resolve_with_sources` takes the two shared
  traits, so a test passes one `MapEnv` twice. Date/Author: 2026-10-27,
  implementation agent.
- Decision: `Settings::resolve` clears `git` whenever positional arguments are
  present. Rationale: ADR 0014's rule that "a typed path always means a file
  run" needs a single place to live, and `Settings` is where both inputs are
  known.
  `git: config.git.unwrap_or(cli.selects_from_git()) && cli.files.is_empty()`
  states it once, so `select_inputs` in `src/main.rs` has no second copy of the
  rule to drift from. Date/Author: 2026-10-27, implementation agent.
- Decision (EP-M4 deviation `a`, recorded as required by the plan's deviation
  rule): the in-process precedence and typed-flag tables live in
  `src/config_tests.rs`, declared from `src/config.rs` by
  `#[cfg(test)] #[path = "config_tests.rs"] mod tests;`, rather than in the
  `tests/cli_config.rs` the Verification plan's `Artefact` fields name.
  Affected identifiers: INV-PRECEDENCE and INV-TYPED, and the `Artefact`
  columns of their obligations. Rationale: `mod config` is declared only from
  `src/main.rs`, so `Settings::resolve_with_sources` is reachable from a binary
  unit test and from nowhere a `tests/*.rs` target can see — an integration
  test links `src/lib.rs`, which by design never names the module. The options
  were to promote the module into the library (which INV-LIB forbids), to
  duplicate the resolver in a test-only crate (which would test a copy), or to
  put the tests where the code lives. The third is the only one that exercises
  production code, and it costs exactly one `#[path]` declaration on
  `src/config.rs`'s foot. Impacts: none on requirements or on architecture;
  `docs/architecture.md` already states that the configuration module belongs
  to the binary. No upstream document change is required, so the plan stays
  `ACTIVE` rather than `BLOCKED`. Date/Author: 2026-10-27, implementation agent.
- Decision (EP-M4 deviation `b`): the process-level contracts live in
  `tests/cli_config_binary.rs` rather than in a `src/main_tests.rs` case.
  Rationale: exit status and stream split are observable only from a child
  process, and an in-process test would share the harness's environment,
  including `assert_cmd`'s own setting of the working directory. The file spawns
  `Command::cargo_bin("mdtablefix")`, so it drives the same binary a user
  runs. Date/Author: 2026-10-27, implementation agent.
- Decision (EP-M4 deviation `c`): INV-LIB is checked textually by
  `tests/cli_config_dependency_contract.rs` rather than by a Cargo feature or a
  module-graph assertion. Rationale: `ortho_config` is a normal dependency of
  the package, so Cargo cannot separate the two targets; the boundary that
  survives is that no file reachable from `src/lib.rs` names the framework. The
  test walks `src/`, reports any file containing `ortho_config`, and asserts
  every such file is one of `src/config.rs` and `src/config_tests.rs`. A
  textual check also covers an `include!` target and a path spelled inside a
  macro. Date/Author: 2026-10-27, implementation agent.
- Decision (CodeRabbit round two, accepted): a discovered file that exists but
  cannot be loaded is a fatal diagnostic even when another scope loaded.
  Rationale: the framework classifies an automatic candidate as optional and
  `drain_errors` appends `ignorable_errors` only when no layer loaded, so a
  malformed project file beside a valid user file left a run silently
  formatting under the user's rules. That contradicts the users' guide, this
  plan's own stated behaviour
  (`A malformed or unreadable configuration file
  produces a clear diagnostic on standard error and exit status 2`),
  and the ADR's purpose. Measured before the fix in an isolated tree: broken
  `.mdtablefix.toml` beside a valid `xdg/mdtablefix/config.toml` exited 0 and
  formatted. After: exit 2, `cannot resolve configuration` plus the TOML parse
  diagnostic naming the file. Absent-candidate and valid-beside-valid runs are
  unchanged, and a broken file alone still reports once rather than twice. The
  fix is `promoted_discovery_errors` in `src/config.rs`, guarded by
  `layers_are_empty()` so the all-empty case is not double-reported.
  Date/Author: 2026-10-27, implementation agent.
- Decision (CodeRabbit round two, accepted): the users' guide's location table
  is presented by scope, not as a flat preference list. Rationale: the table
  was labelled "most preferred first" with the working-directory file last,
  which reads as if the project file were the least influential. Measured truth
  is the reverse — with a user file setting `ellipsis = true` and a project
  file setting `ellipsis = false`, the run emits `a...b`, so the project file
  wins. The table now carries a scope column and the prose says the project
  file is the most influential, which is also what ADR 0014 already stated.
  Date/Author: 2026-10-27, implementation agent.
- Decision (CodeRabbit round two, rejected): do not rename
  `docs/adrs/0014-configuration-layering.md` to the style guide's
  `adr-NNN-short-description.md` form. Rationale: ADR 0014 records the
  deviation and its reason at lines 48-56 — every record in `docs/adrs/`
  follows the local form, and "changing the convention is a repository-wide
  decision and is not made here." Renaming one file would leave that paragraph
  false and split the directory across two conventions mid-adoption.
  Date/Author: 2026-10-27, implementation agent.
- Decision (CodeRabbit round two, rejected twice): do not retarget the links
  inside `docs/ortho-config-users-guide.md` or
  `docs/ortho-config-v0-10-0-migration-guide.md`. Rationale:
  `docs/ortho-config-v0-10-0-provenance.md` states the invariant — both files
  are byte-for-byte copies of their tag blobs, with SHA-256 values recorded per
  file, "this note is kept beside them rather than inside them, precisely so
  they stay byte-identical." Editing a link is an edit to a vendored artefact,
  and the provenance check would then fail. Date/Author: 2026-10-27,
  implementation agent.
- Decision (CodeRabbit round two, rejected): do not make `md_exts` uniquely
  lenient when a positional path is present. Rationale: measured behaviour is
  that every configured setting is validated regardless of the resolved mode,
  and the four Git-only settings block a positional run uniformly — `wrap`,
  `git`, `include_untracked`, and `md_exts` all behave alike. The two variants
  of the finding (validate no non-Git setting, or validate nothing when a
  positional path is given) contradict each other, so neither can be the
  intended rule. ADR 0014 now records the uniform rule and its reason: a typo
  should surface on every invocation, not only on some. Date/Author:
  2026-10-27, implementation agent.
- Decision (CodeRabbit round two, forced by the fix): the round-two work pushed
  three files past the 400-line cap, so each was split along a seam rather than
  trimmed. `src/config.rs` 435 → 336 by extracting `src/config_schema.rs` (115
  lines: the constants and `ConfigFile`); `src/config_tests.rs` 403 → 217, with
  the failure cases in `src/config_error_tests.rs` (176) and the shared fixture
  in `src/config_test_support.rs` (127); `tests/cli_config_binary.rs` 422 →
  210, with the failing cases in `tests/cli_config_failure_binary.rs` (164) and
  the fixture in `tests/support/cli_config_fixture.rs` (156). No test was
  dropped. The dependency contract's `BINARY_ONLY` list grew from two files to
  five, and `docs/developers-guide.md` now names all five. Date/Author:
  2026-10-27, implementation agent.
- Decision: a shared test-support module may define only items every including
  binary uses. Rationale: each `tests/*.rs` file compiles alone, so
  `#[path = "support/x.rs"] mod x;` is recompiled per binary, and an item one
  binary never touches is `dead_code` under `-D warnings`. `CLEAN` was defined
  in `tests/support/cli_config_fixture.rs` and was dead in
  `cli_config_failure_binary.rs`; it now lives in `cli_config_binary.rs`, its
  only consumer. The rule is stated in the fixture's module documentation so
  the next shared item is placed correctly. Date/Author: 2026-10-27,
  implementation agent.

### CodeRabbit round three

Five findings. Three were accepted and fixed after each was reproduced; two
were the same defect reported twice and are closed by one change.

- Decision (accepted): scrub the child environment by case-insensitive prefix
  rather than by a list of spellings. Rationale: the provider upper-cases a
  name before matching it, measured three ways, so a list can only cover the
  readings it enumerates. A prefix scan also covers every key the schema gains
  later. The cost is one `std::env::vars_os()` call at a genuine composition
  root, which is why the site was added to `SANCTIONED_ROOTS` and argued in
  ADR-0012 and the developers guide in the same change, as that policy
  requires. Date/Author: 2026-10-27, implementation agent.
- Decision (accepted, in a stronger form than reported): replace the textual
  reachability check with a module-graph walk. Rationale: the finding named
  `#[path]`, but `include!` reaches a file just as surely, and a walk that
  ignored it would leave the same hole under another spelling. The walker
  follows `mod`, `#[path]` and `include!`, resolves the two base directories
  the Rust Reference distinguishes, and fails on a declaration whose candidates
  all miss. Non-vacuity comes from a negative control asserting both spellings
  resolve to the same target, and from the `config_schema.rs` mutation recorded
  under Surprises. Date/Author: 2026-10-27, implementation agent.
- Decision (accepted, documentation only): state that the wrong-type
  diagnostic reports the mismatch and nothing else. Rationale: measured
  identical text and exit status from a selected file, a discovered file and an
  environment variable, so the framework cannot name the source. The users'
  guide tells the reader how to locate the value rather than promising a
  location the code cannot supply. Date/Author: 2026-10-27, implementation
  agent.
- Decision (accepted, documentation only): record the re-sync check for each
  vendored file separately in `docs/ortho-config-v0-10-0-provenance.md`.
  Rationale: the two copies come from different upstream files, so a matching
  hash for one says nothing about the other, and a single combined check would
  let the second drift unnoticed. Date/Author: 2026-10-27, implementation agent.
- Decision (duplicate): the two findings about the reachability check describe
  the same defect — one named the missing resolution rule, the other the
  declaration form that hides it. Both are closed by the walk, and the negative
  control covers each case the pair named. Date/Author: 2026-10-27,
  implementation agent.
- Decision (policy, recorded here because it changed the allowlist): add
  `command` in `tests/support/cli_config_fixture.rs` to `SANCTIONED_ROOTS`.
  Rationale: it is the composition root for each child process the
  process-level configuration tests spawn, and it has no seam because the
  property under test is that the ambient environment does not reach the child;
  handing it a scrubbed copy would let the fixture stop testing exactly that.
  ADR-0012, the developers guide and `tests/env_access_suppressions.rs` were
  updated in the same change, as the policy text requires. Mutation proof:
  removing the entry fails
  `a_sanctioned_expect_is_not_an_offence::case_3_cli_config_fixture` with an
  index-out-of-bounds panic, because the case indexes the array it was removed
  from. Date/Author: 2026-10-27, implementation agent.

### CodeRabbit round four

Three findings, run against the head after round three landed. One was a real
defect reported twice; the third repeats a round-two finding already rejected.

- Decision (accepted, and the duplicate closed with it): correct
  `Settings::resolve`'s rustdoc. It claimed that "only a selected file or an
  empty search" reports a discovery error, but `promoted_discovery_errors`
  deliberately promotes an existing-but-unloadable automatic candidate even
  when another layer loaded — that promotion is the round-two fix, so the
  sentence contradicted the code beneath it. It now says what the module does:
  an absent candidate is never a failure, an unloadable one is reported. The
  two findings are the same claim at two lengths. Date/Author: 2026-10-27,
  implementation agent.
- Decision (rejected, for the second time): do not rename
  `docs/adrs/0014-configuration-layering.md` or move it into `docs/`. The same
  finding was raised and rejected in round two; the reasoning has not changed.
  `docs/documentation-style-guide.md` does prescribe
  `adr-NNN-short-description.md` in `docs/`, but all fourteen records in
  `docs/adrs/` use a four-digit prefix and a bare title, and ADR 0014 states
  that deviation and its reason in its own Context section: following the local
  form is the consistent choice, and changing the convention is a
  repository-wide decision outside this issue's scope. Renaming one record
  would leave it the only file in the directory spelled differently from its
  thirteen siblings and would break every existing cross-reference to it.
  Date/Author: 2026-10-27, implementation agent.

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

Filled in at each milestone.

EP-M0 and EP-M1 are complete. The spike settled the question it was built to
settle — the derive is usable under this workspace's lints — and cost an hour
rather than the days a design built on an unproven macro would have cost if the
axiom had been false. It also produced two facts the design would otherwise
have discovered the hard way: `clap` must be a direct dependency of any
deriving crate, and `ortho_config` re-exports `serde_json`, so the manifest
addition is exactly two crates. The one deviation from the plan as written is
the location of the imported guides' provenance: a header inside each file
would have broken the byte-identity the same plan demanded, so the note lives
beside them. That is recorded in `Decision log`.

EP-M2 to EP-M4 are complete, and together they retire the plan's central
assumption: the framework can be adopted without its generated parser.
`MergeComposer` plus the derive-emitted `merge_from_layers` gives the layer
composition the design wanted, and the command line keeps
`Cli::parse_validated` as its only parser, so the `--help` snapshot is
byte-identical to the base commit. Three adjustments were needed and all three
are in `Decision log`: `parse_validated` returns the `ArgMatches` beside the
`Cli` (adjustment `a`), `FormatOpts` gains three derives so settings compare as
values (adjustment `b`), and the execution half of `run()` moves to
`src/driver/run.rs` to stay under the 400-line cap (adjustment `c`). EP-M4 then
found one deviation worth naming: the in-process tables had to live in
`src/config_tests.rs`, declared by a `#[path]` module from the binary-only
`src/config.rs`, because `mod config` is never declared from `src/lib.rs` and a
`tests/` target could not reach it (deviation `a`). What would be done
differently: the discovery probe should have rebuilt its tree per case from the
start, and every claim about a diagnostic should have been produced by running
the binary before it was written down — both are recorded under
`Surprises & discoveries` because both cost a false pass.

EP-M5 is in progress. The guides are written and their claims measured; the
draft pull request is open. The outstanding work is the CodeRabbit round.

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
running the built binary with `--help`, normalizing `mdtablefix.exe` to
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
                -> EP-M4 -> src/config_tests.rs; tests/cli_config_binary.rs;
                             tests/cli_config_dependency_contract.rs
TOR-REQ-666-7   Document the user-facing configuration surface and the internal
                integration boundary.
                -> EP-M5 -> docs/users-guide.md; docs/developers-guide.md
                             (written 2026-10-27, commit 0d12ec2)
TOR-CON-666-1   No production process-environment reads (ADR 0012).
                -> EP-M3, EP-M4 -> make lint (clippy::disallowed_methods)
TOR-CON-666-2   No merge and no release publication.
                -> EP-M5 -> the deliverable is a draft pull request; the
                             branch is never merged into main
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
  Method: a child-process case pinning the whole observable result, plus the
  unchanged existing suites. Rationale: the existing suite already pins the
  observable behaviour of every mode, and the one case the configuration module
  could disturb is the invocation that supplies nothing at all. Domain: the
  default file mode reading standard input, in a temporary directory whose
  configuration base directories all point into a temporary tree, with an
  explicitly empty selected file so discovery cannot reach the host's own
  configuration. Artefact: `tests/cli_config_binary.rs`'s
  `a_run_with_no_configuration_formats_as_it_always_did`, plus the unchanged
  `tests/snapshots/cli_git_help.snap` and the unchanged existing suites.
  Evidence: `cargo test --workspace` passes with the snapshot unmodified; the
  case asserts standard output equal to the expected rendering character for
  character, empty standard error, and exit status 0. Non-vacuity: the expected
  output is the aligned three-line table `CLEAN`, the input is the ragged
  three-line table `RAGGED`, and the two differ in every cell, so a run that
  echoed its input, produced nothing, or aligned differently would fail.
  Reverting the wiring is not a meaningful control here — the module is what
  supplies the merge — so the control is the pre-change output, recorded in the
  test as the constant `CLEAN`. Deviation from the planned differential case:
  the base binary is not built, because the case pins the exact bytes instead,
  which is the same claim without a second compilation. Recorded in
  `Decision log` deviation `a`.
- Obligation: INV-PRECEDENCE.
  Method: parameterized `rstest` table over source combinations, using injected
  sources only. Rationale: the partition is finite and small; a table makes
  each cell's meaning explicit and makes a missing combination visible. Domain:
  five keys (`wrap`, `renumber`, `headings`, `md_exts`, `git`) across five
  source combinations (defaults only, file only, environment only, file and
  environment, all three including the command line), with the file supplied
  both through discovery and through an explicit `MDTABLEFIX_CONFIG_PATH`.
  Artefact: `src/config_tests.rs` with a `MapEnv` and a temporary TOML file —
  not `tests/cli_config.rs`, because `mod config` is binary-only; see
  `Decision log` deviation `a`. Evidence: the table fails before the wiring
  lands, because the configuration module does not exist; after the wiring, all
  50 cells pass. Non-vacuity: each cell asserts the *resolved* value, not
  merely that parsing succeeded, and the file-only and environment-only cells
  differ in value from each other and from the default so a layer silently
  dropped would change an assertion. Both controls were run: reversing the
  layer order in `merged_config` failed exactly the ten `FileAndEnv` cells and
  no others, and a probe naming the framework in `src/wrap.rs` failed INV-LIB's
  detector; both were reverted.
- Obligation: INV-TYPED.
  Method: parameterized test plus child-process checks. Rationale: the
  interesting cases are exactly two — a config-supplied `md_exts` with no typed
  `--md-exts`, and a typed `--md-exts` with no configuration — and both are
  cheap to enumerate. Domain: `--git` present and absent, crossed with
  `md_exts` supplied by configuration or by argv. Artefact: the in-process half
  in `src/config_tests.rs`, the child-process half in
  `tests/cli_config_binary.rs` using `assert_cmd`. Evidence: the
  config-supplied case succeeds without `--git`; the typed case exits 2 with a
  message naming `--md-exts` and `--git`. Non-vacuity: the typed case asserts
  both the exit status and the message fragment, so a run that ignored the flag
  entirely would fail. Both child-process cases deliberately format standard
  input rather than asking for `--version`, because clap answers a version
  request before `Settings::resolve` is reached and that spelling would pass
  whether or not the rule were correct.
- Obligation: INV-EXIT and INV-STREAM.
  Method: child-process checks in `assert_cmd`. Rationale: the exit status and
  the stream split are process-level contracts; no in-process test can observe
  them faithfully. Domain: a malformed configuration file, an unreadable one, a
  selected but missing one, a valid one that sets `wrap`, and no configuration
  at all. Artefact: `tests/cli_config_failure_binary.rs` now holds the failing
  cases and `tests/cli_config_binary.rs` the passing ones, split on 2026-10-27;
  see the EP-M4 progress entry. Evidence: the malformed,
  malformed-discovered-beside-a-valid-file, and unreadable cases exit 2 with
  the diagnostic on standard error and nothing on standard output; the valid
  case exits 0 and formats as configured; the no-configuration case exits 0
  with the base output. Non-vacuity: every case asserts standard output and
  standard error separately, so a diagnostic printed to the wrong stream fails.
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
  this workspace's `clippy::allow_attributes` restriction. **Discharged**
  2026-10-26 by EP-M0's compile spike, whose
  `cargo clippy --all-targets -- -D warnings` run was clean under a verbatim
  copy of `clippy.toml`.
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

The module-graph walk expands no macros, so a macro defined in another file and
invoked with a body that writes no `mod` token remains invisible to it: the
`#[path]` a `sneak!()` expands to is written at the definition, not the
invocation, and a walk that read every `macro_rules!` body would still miss one
compiled from a dependency. The walk refuses what it can see — a definition
whose body writes `mod`, and an invocation whose arguments do — and its
diagnostic says the construct is unanalysable rather than silently passing it.
Closing the gap completely would require the compiler's own expansion list, as
`cargo`'s dep-info provides for `include!` targets; the test suite has no
supported way to read that during a unit test, so the residual risk is recorded
here rather than claimed away. It is bounded: the library names no framework
today, the textual half of the contract would catch an ordinary `use`, and the
walk refuses every `mod`-writing macro it finds in the tree.

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
- Stage D: refactor, documentation, and wider validation. EP-M5. The
  documentation half landed in `0d12ec2`; the wider validation is the full gate
  set run on the final tree, and the delivery is the push and the draft pull
  request.

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
commit `38812c563e3953a490e3efc202609f37e21a0517`. Provenance goes in a third
file, `docs/ortho-config-v0-10-0-provenance.md`, carrying the repository, the
tag, the tag object, the commit, each upstream path, and each SHA-256, plus the
re-sync recipe and the version boundary. Keeping it beside the copies rather
than inside them is what preserves byte-identity: a prepended provenance
paragraph changes the file, and so changes its hash. Add all three files to
`docs/contents.md` under "Reference material".

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

EP-M4, the tests. Add `src/config_tests.rs` for the in-process precedence and
typed-flag tables, `tests/cli_config_binary.rs` for the process-level exit and
stream contracts, and `tests/cli_config_dependency_contract.rs` for INV-LIB.
Add no new `#[allow]` and no new `#[expect]` beyond what ADR 0012 already
permits at the composition root. (As built, the in-process tables went to
`src/config_tests.rs`, declared by a `#[path]` module on `src/config.rs`,
rather than to `tests/cli_config.rs`: `mod config` is binary-only, so a
`tests/` target cannot reach `Settings::resolve_with_sources`. See
`Decision log` deviation `a`.)

EP-M5, the documentation and delivery. As built on 2026-10-27 (`0d12ec2`), the
"Configuration" section of `docs/users-guide.md` sits after "Selecting files
from Git" and before "Exit status" and covers what may be configured (settings,
never the mode or the file arguments), a TOML example, an ordered discovery
table, the precedence order with the layering rule, the twelve `MDTABLEFIX_*`
variables, the value dialect, the four failures worth recognizing, unknown-key
tolerance, and the enable-but-not-disable asymmetry. It covers four failures
rather than the three the plan named, because measurement found a fourth: a
value of the wrong type is reported by the merge, after the file has been read,
so its diagnostic does not repeat the path — the users' guide says so, and a
user who reads only the quoted text should know which setting to look at. The
discovery list became a table rather than prose because ten distinct locations
are involved, and each was probed in an isolated tree before the table was
written. The "Configuration layering" section of `docs/developers-guide.md`
sits as the plan asked, immediately after the "Environment access policy"
section and its "Environment variables in subprocess tests" and "Stub scripts
in subprocess tests" subsections, and before "Coverage publication belongs to
`main`"; it covers the binary-only module and the test that guards it, why a
Cargo feature cannot express the boundary, the hand-composed layers, the
`resolve` / `resolve_with_sources` split, the two provenance rules, and the
residual dependency on the working directory. Run every gate. Commit. Push.
Open the draft pull request. Do not merge.

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
  `clippy.toml`, or the plan turns `BLOCKED`. Discharged on 2026-10-26:
  `Finished dev profile in 22.69s` with no diagnostics, and the spike's one
  unit test passed.
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
  `b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf` exactly,
  with no text added; `make markdownlint` passes. Discharged on 2026-10-26:
  both hashes matched, and the commit `c63e18f` carries the two guides, the
  provenance note, and the contents index.
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
  - No unapproved interface or dependency change: the two authorized
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
- Acceptance evidence: 75 tests added — 57 in `src/config_tests.rs`, 14 in
  `tests/cli_config_binary.rs`, 4 in
  `tests/cli_config_dependency_contract.rs` — every one passing, with both
  negative controls demonstrated once and recorded under
  `Surprises & discoveries`.
- Conformance check:
  - Requirements satisfied: TOR-REQ-666-6.
  - Design still followed: the tests exercise the ADR's precedence order.
  - Upstream assumptions still valid: AXIOM-CLAP-SOURCE holds under test.
  - No unapproved interface or dependency change: none.
  - No unapproved trust boundary: the tests read only temporary trees the
    fixture creates, and no process environment.
  - Trace links current: the two obligations that named `tests/cli_config.rs`
    now name `src/config_tests.rs`; see `Decision log` deviation `a`.
- Recovery: the tests are additive; reverting them cannot break the tree.
- Remaining gaps: documentation and delivery.
- Compatibility decision: none.

- Identifier and outcome: EP-M5, the documentation and the draft pull request.
- Requirements and gaps: TOR-REQ-666-7, TOR-CON-666-2.
- Acceptance evidence: the user's guide and developer's guide describe the
  shipped behaviour; all gates pass on the pushed tip; a draft pull request
  exists with `(#666)` in the title, `Closes #666` in the body, and the Lody
  session reference. **Documentation lands first**, as its own commit, so the
  guides can be reviewed on their own and the delivery commit carries only the
  push and the pull request. Done 2026-10-27 in `0d12ec2`:
  `docs/users-guide.md` gains a "Configuration" section between "Selecting
  files from Git" and "Exit status", and `docs/developers-guide.md` gains
  "Configuration layering" between the "Environment access policy" subsections
  and "Coverage publication belongs to `main`". Documentation gates on that
  commit: `make check-fmt` exit 0 (47 files unchanged), `make markdownlint` 47
  files 0 issues, `make nixie` all diagrams validated. The four claims the
  guides make about failure behaviour were each produced by running the binary,
  and the two that needed more than one run — the typed-value diagnostic that
  omits the path, and the discovery order, which needed an isolated tree per
  location — are recorded under `Surprises & discoveries`. Remaining for this
  milestone: the full gate set on the final tree, the push, and the draft pull
  request.
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

Nothing is then edited into either file: the checksum check re-runs and must
produce the same hashes, and the provenance, re-sync recipe, and version
boundary are written to `docs/ortho-config-v0-10-0-provenance.md` instead:

```console
$ sha256sum docs/ortho-config-users-guide.md \
    docs/ortho-config-v0-10-0-migration-guide.md
84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575  docs/ortho-config-users-guide.md
b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf  docs/ortho-config-v0-10-0-migration-guide.md
```

Gates, each logged under a `/tmp` file named for the action and the branch, so
the output can be reviewed after the run rather than scrolled through. The same
recipe shape is repeated for `lint`, `test`, `markdownlint`, and `nixie`:

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
environment also normalizes thematic breaks.

Measured on 2026-10-27, the override direction is asymmetric, and this plan
originally overstated it. Every switch this tool declares is a bare presence
flag, so the command line can say `true` and cannot say `false`:
`--breaks=false` is a clap parse error,
`unexpected value 'false' for '--breaks' found; no more were expected`, exit
status 2. A file or variable that enables a transformation is therefore turned
*off* at the environment layer, which can express `false` —
`MDTABLEFIX_BREAKS=false` beside `breaks = true` resolves to `false`, which is
the `FileAndEnv` cell of the precedence table for the three switches of that
polarity — and not from the command line. This is a property of the existing
flag shape, not of the adoption: the flags were presence flags before this
change and the issue requires that they stay so.

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

Quality method (how it is checked):

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
