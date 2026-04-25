# Add a CLI option matrix test harness

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

After this change, `mdtablefix` will have an integration test harness that
checks important combinations of command-line options together instead of only
checking most flags in isolation. A maintainer will be able to add a new CLI
flag, fixture, or interaction case in one place and rely on harness tests to
prove that the case catalogue still covers the intended option combinations.

Observable success means `cargo test --test cli_matrix` runs the matrix through
the real `mdtablefix` binary with `assert_cmd`, snapshots each case with
`insta`, and fails clearly when a fixture is missing, a case identifier is
duplicated, a non-`.dat` fixture is used, or the selected case set no longer
covers the required option pairs. Each logical option combination is exercised
twice against the same fixture: once by writing formatted output to stdout and
once with the repository's existing `--in-place` flag. The harness also treats
`--wrap` as a required paired variant because earlier `--wrap` and non-`--wrap`
runs have regressed independently. The developer guide will explain how the
harness works, why fixtures use `.dat`, and how to update snapshots
intentionally.

This plan is draft-only until approved. It does not implement the harness or
edit `docs/developers-guide.md` yet.

## Constraints

- Keep public CLI flags and the public library API stable.
- Exercise the real binary using `assert_cmd`; do not replace the CLI matrix
  with direct calls into `process_stream_inner`.
- Use `insta` snapshots for matrix outputs so large expected results do not
  become hard-to-review string literals.
- Keep matrix input fixtures in `.dat` files under `tests/data/cli-matrix/` so
  `make fmt` and Markdown formatters do not rewrite them.
- For every logical transform combination in the matrix, run both stdout
  formatting and `--in-place` formatting against equivalent temporary-file
  input. Treat stdout formatting as the ordinary file-to-stdout path, not as a
  substitute for exercising `--in-place`.
- For every selected combination of non-wrap transform flags, run both the
  `--wrap` and no-`--wrap` variants. This is mandatory regression coverage, not
  an optional pairwise side effect.
- Add tests for the matrix harness itself. The harness must not rely on review
  discipline alone to preserve fixture naming, case uniqueness, or
  combinatorial coverage.
- Do not grow `tests/cli.rs`. It is already 392 lines and the repository file
  limit is 400 lines for code files.
- Keep every new Rust source file below 400 lines. Split helpers into a second
  test support module if the matrix file approaches that limit.
- Keep CLI output on stdout and diagnostics on stderr. The snapshots must make
  those channels distinct.
- Use deterministic case ordering. No randomization, fuzzing seed, or
  time-dependent case generation is allowed in this harness.
- Update `docs/developers-guide.md` during implementation so future
  maintainers know how to use and extend the matrix harness.

## Tolerances (exception triggers)

- Scope: if implementation needs changes to more than 8 files or roughly 450
  net lines of Rust code, stop and re-evaluate the harness shape.
- Dependencies: if adding `insta` requires a Rust version newer than the
  repository's `rust-version = "1.89"`, stop and choose a compatible version or
  ask for direction.
- Interfaces: if the matrix requires changing any CLI flag name, CLI exit-code
  rule, or public library function signature, stop and escalate.
- Coverage: if pairwise coverage across the seven non-wrap transform flags
  cannot be achieved in 24 or fewer base rows before `--wrap` and execution-mode
  expansion, stop and document why a larger matrix is justified.
- Expansion: if the generated matrix exceeds 96 physical command runs after
  expanding each base row into `--wrap` and no-`--wrap` variants and then into
  stdout and `--in-place` modes, stop and re-evaluate the base row count.
- Snapshot churn: if one fixture change rewrites more than 30 snapshots, stop
  and consider whether the fixture is too broad or the matrix should be split.
- Validation: if `make lint` or `make test` still fails after three focused fix
  cycles, stop and capture the failing command, log path, and likely cause in
  `Decision Log`.

## Risks

- Risk: a full Cartesian product of the eight transform flags, wrapped and
  unwrapped variants, and paired stdout and `--in-place` execution would create
  hundreds of cases and make snapshot review noisy. Severity: high.
  Likelihood: high. Mitigation: use a curated base matrix with generated
  `--wrap` and execution-mode expansion, plus harness tests that prove every
  required option pair and expansion rule remains covered.

- Risk: a single overpacked fixture could make failures hard to diagnose.
  Severity: medium. Likelihood: medium. Mitigation: use a small set of named
  `.dat` fixtures, each aimed at a cluster of behaviours such as tables and
  prose, fences and ellipses, frontmatter and breaks, or footnotes and wrapping.

- Risk: `insta` snapshots can accidentally bless behavioural regressions if
  accepted mechanically. Severity: medium. Likelihood: medium. Mitigation:
  document the update workflow in `docs/developers-guide.md`, keep snapshot
  names stable and descriptive, and snapshot stdout, stderr, exit status, and
  in-place file content in a labelled envelope.

- Risk: `--in-place` has different command-line validity rules from stdin
  mode because Clap requires a file path when `--in-place` is set. Severity:
  medium. Likelihood: high. Mitigation: model execution mode explicitly in the
  harness and add harness tests that reject invalid `--in-place` cases.

- Risk: the harness might accidentally validate stdout mode and assume
  `--in-place` behaves the same, missing file rewrite regressions. Severity:
  high. Likelihood: medium. Mitigation: generate stdout and `--in-place`
  physical runs from every logical combination and add a harness test proving
  that the expansion is complete.

- Risk: pairwise coverage could technically include `--wrap` without ensuring
  every important non-wrap combination is tested both with and without
  wrapping. Severity: high. Likelihood: medium. Mitigation: keep `--wrap` out
  of the base combination generator, expand each base row into a wrapped and an
  unwrapped variant, and add a harness test that proves every non-wrap
  signature has both variants.

- Risk: `make fmt` runs Markdown formatting and could modify Markdown fixtures
  if they are stored as `.md` or `.txt` files. Severity: medium. Likelihood:
  high. Mitigation: store matrix fixtures with a `.dat` extension and add a
  harness test that rejects any case fixture whose path does not end in `.dat`.

## Progress

- [x] (2026-04-25 14:19Z) Verified the working tree is on branch
  `cli-matrix-testing` and is clean before drafting the plan.
- [x] (2026-04-25 14:19Z) Loaded the `leta`, `execplans`,
  `rust-router`, `domain-cli-and-daemons`, and `commit-message` skills needed
  for planning, Rust CLI orientation, and the eventual plan commit.
- [x] (2026-04-25 14:19Z) Reviewed `src/main.rs`, `tests/cli.rs`,
  `tests/common/mod.rs`, `Cargo.toml`, `Makefile`, and
  `docs/developers-guide.md`.
- [x] (2026-04-25 14:19Z) Confirmed the CLI transform switches are `--wrap`,
  `--renumber`, `--breaks`, `--ellipsis`, `--fences`, `--footnotes`,
  `--code-emphasis`, and `--headings`, with `--in-place` as an execution mode
  flag that requires file input.
- [x] (2026-04-25 14:34Z) Updated the draft plan so every logical combination
  runs in both stdout and `--in-place` modes and every non-wrap flag
  combination runs both with and without `--wrap`.
- [ ] Await approval to implement the harness, fixtures, snapshots, and
  developer-guide documentation.

## Surprises & discoveries

- Observation: `grepai` returned no useful semantic hits for this repository,
  even after using the `get-project` value `mdtablefix`. Impact: exploration
  used `leta` for symbols and targeted exact searches for test dependencies,
  CLI flags, and fixture references.

- Observation: `tests/cli.rs` is 392 lines long. Impact: the implementation
  must use a new integration test target such as `tests/cli_matrix.rs` rather
  than extending the existing CLI test file.

- Observation: `assert_cmd` is already present in `Cargo.toml`, but `insta` is
  not. Impact: implementation will need a small dev-dependency addition and a
  `Cargo.lock` update.

- Observation: `make fmt` runs both `cargo fmt --all` and `mdformat-all`.
  Impact: input fixtures must not use Markdown file extensions, and the harness
  should enforce the `.dat` fixture convention directly.

- Observation: the repository flag is named `--in-place`; the requested
  `--inplace` behaviour maps to that existing flag rather than a new CLI
  spelling. Impact: the plan uses `--in-place` consistently and does not add an
  alias.

- Observation: previous regressions have appeared only when comparing wrapped
  and unwrapped executions. Impact: the matrix must not rely on incidental
  pairwise coverage for `--wrap`; it must generate explicit wrapped and
  unwrapped variants for each selected non-wrap combination.

## Decision log

- Decision: use a curated pairwise matrix rather than a full Cartesian product.
  Rationale: the purpose is to catch interactions while keeping snapshot review
  tractable. Harness tests will make the selected cases auditable by verifying
  pair coverage. Date/Author: 2026-04-25 / Droid.

- Decision: create a new integration test target, `tests/cli_matrix.rs`,
  instead of modifying `tests/cli.rs`. Rationale: `tests/cli.rs` is already
  close to the 400-line limit, and the matrix harness is conceptually distinct
  from the existing focused CLI regressions. Date/Author: 2026-04-25 / Droid.

- Decision: snapshot a labelled result envelope rather than only stdout.
  Rationale: the CLI contract includes status, stdout, stderr, and, for
  `--in-place`, rewritten file content. Keeping those fields labelled avoids
  confusing data output with diagnostics. Date/Author: 2026-04-25 / Droid.

- Decision: keep fixtures in `tests/data/cli-matrix/*.dat`. Rationale: this
  satisfies the user requirement that matrix fixtures are not formatted by
  `make fmt`, while keeping test data near the existing integration fixtures.
  Date/Author: 2026-04-25 / Droid.

- Decision: model the catalogue as base combinations over the seven non-wrap
  transform flags, then expand each base combination into both `--wrap` states
  and both file execution modes. Rationale: this guarantees the historically
  sensitive wrapped/unwrapped comparison and ensures every combination is
  tested through both stdout formatting and `--in-place`. Date/Author:
  2026-04-25 / Droid.

## Outcomes & retrospective

No implementation has been started. This section must be updated after the
approved plan is executed with the files changed, validation results, and any
lessons from maintaining the matrix.

## Context and orientation

The CLI entry point is `src/main.rs`. `Cli` defines `--in-place` and delegates
the formatting switches to `FormatOpts`. `FormatOpts` currently exposes eight
independent transform switches:

- `--wrap`
- `--renumber`
- `--breaks`
- `--ellipsis`
- `--fences`
- `--footnotes`
- `--code-emphasis`
- `--headings`

`process_lines` in `src/main.rs` first protects leading YAML frontmatter, then
passes the body to `process_stream_inner`. The `--renumber` and `--breaks`
transforms run afterwards in the CLI layer. This means the matrix must include
cases where CLI-only transforms interact with transforms handled inside the
library pipeline.

The matrix should treat `--wrap` specially. The base catalogue covers the seven
non-wrap transform flags: `--renumber`, `--breaks`, `--ellipsis`, `--fences`,
`--footnotes`, `--code-emphasis`, and `--headings`. The harness then generates
two logical variants from every base row, one without `--wrap` and one with
`--wrap`. It then generates two physical command executions from each logical
variant: ordinary file-to-stdout formatting and `--in-place` formatting against
an equivalent temporary file.

Current integration helpers live in `tests/common/mod.rs` and are re-exported
through `tests/prelude/mod.rs`. They already provide `run_cli_with_args` and
`run_cli_with_stdin`, both built on `assert_cmd`. The new matrix can reuse the
same import pattern but should run its paired stdout and `--in-place`
executions through temporary files so both physical modes operate on equivalent
file input. It will need richer output capture than the existing helpers
provide because `insta` snapshots should include all relevant process outputs.

Existing large fixtures live under `tests/data/`. The matrix should add a
subdirectory, `tests/data/cli-matrix/`, to keep fixture ownership obvious and
to make the `.dat` convention easy to enforce.

## Plan of work

Stage A adds dependencies and static fixture loading. Add `insta = "1"` to
`[dev-dependencies]` in `Cargo.toml` and update `Cargo.lock` by running the
normal Cargo test command later in the workflow. Create
`tests/data/cli-matrix/` with a small fixture set using `.dat` extensions.
Start with fixtures for:

- a combined table and prose document that can exercise `--wrap`,
  `--renumber`, `--ellipsis`, `--code-emphasis`, and normal table reflow;
- a fence-heavy document that can exercise `--fences`, `--ellipsis`, and
  `--renumber` ordering;
- a footnote-heavy document that can exercise `--footnotes`, `--wrap`, and
  list renumbering;
- a frontmatter and thematic-break document that can exercise `--headings`,
  `--breaks`, and frontmatter preservation.

Stage B builds the matrix harness in `tests/cli_matrix.rs`. Define small data
types for non-wrap transform flags, wrap variant, execution mode, fixture path,
base matrix case, logical matrix case, and physical command case. Keep the base
case catalogue as deterministic static data, for example `BASE_MATRIX_CASES`.
Each base case must have a stable, filesystem-safe identifier, a fixture path,
and the non-wrap transform flags to pass to the CLI. The harness must expand
each base case into wrapped and unwrapped logical variants, then expand each
logical variant into file-to-stdout and `--in-place` physical runs. The command
runner must use `assert_cmd::Command::cargo_bin("mdtablefix")`, copy the
fixture into a temporary file for both physical modes, assert success for
success cases, and build a snapshot envelope containing status, stdout, stderr,
and file content when relevant.

Stage C adds harness self-tests before relying on matrix outputs. Add tests
that verify:

- every case identifier is unique and contains only lowercase letters, digits,
  hyphens, and underscores;
- every fixture path exists and ends in `.dat`;
- every generated physical case has both a stdout run and an `--in-place` run
  for the same logical combination;
- every selected non-wrap flag signature has both a wrapped and an unwrapped
  logical variant;
- every non-wrap transform flag appears enabled and disabled at least once;
- every pair of non-wrap transform flags appears in all four enabled/disabled
  combinations across the base case catalogue.

Stage D adds the matrix execution test and snapshots. Use
`insta::assert_snapshot!` with stable snapshot names derived from the case
identifier. The snapshot value should be a labelled plaintext envelope such as:

```plaintext
case: wrap-footnotes-stdout
mode: stdout
args: --wrap --footnotes
status: success

[stdout]
...

[stderr]
...

[file]
<not applicable>
```

For an `--in-place` physical run, the same logical case should produce a second
snapshot whose mode is `in-place`, whose stdout and stderr are empty on
success, and whose `[file]` section contains the rewritten temporary file. The
harness should also assert that the stdout run's formatted stdout equals the
`--in-place` run's rewritten file content for the same logical case.

Run the focused matrix test with `INSTA_UPDATE=always` only when creating or
intentionally updating snapshots. Then rerun without `INSTA_UPDATE` to prove no
`.snap.new` files remain.

Stage E documents the harness in `docs/developers-guide.md`. Add a concise
section named "CLI matrix harness" that explains the purpose, case catalogue,
fixture location, `.dat` requirement, snapshot update workflow, and the harness
self-tests. Include the exact focused commands for reviewing and updating
snapshots.

Stage F runs repository validation, reviews the changed code for refactoring
needs, updates this plan with results, and commits the approved implementation
only after gates pass.

## Concrete steps

Work from the repository root:

```bash
pwd
git branch --show
```

Expected:

```plaintext
/data/leynos/Projects/mdtablefix.worktrees/cli-matrix-testing
cli-matrix-testing
```

Add the dependency and fixture skeleton, then create the harness types and
self-tests in `tests/cli_matrix.rs`. Run the focused harness checks first:

```bash
cargo test --test cli_matrix matrix_case_ids_are_unique
cargo test --test cli_matrix matrix_cases_expand_to_stdout_and_in_place
cargo test --test cli_matrix matrix_cases_expand_to_wrapped_and_unwrapped
cargo test --test cli_matrix matrix_cases_cover_all_transform_pairs
```

Expected result: all focused harness tests pass, proving the catalogue can be
trusted before snapshots are reviewed.

Create initial snapshots intentionally:

```bash
INSTA_UPDATE=always cargo test --test cli_matrix cli_matrix_snapshots
```

Expected result: the test passes and `tests/snapshots/` contains new
`cli_matrix__*.snap` files. Review the snapshots before accepting the change.
Then prove the accepted snapshot set is stable:

```bash
cargo test --test cli_matrix
```

Expected result: all harness and matrix tests pass without producing any
`.snap.new` files.

After documentation is updated, run formatting and full gates sequentially with
logs:

```bash
make fmt | tee /tmp/fmt-mdtablefix-cli-matrix-testing.out
make check-fmt | tee /tmp/check-fmt-mdtablefix-cli-matrix-testing.out
make lint | tee /tmp/lint-mdtablefix-cli-matrix-testing.out
make test | tee /tmp/test-mdtablefix-cli-matrix-testing.out
make markdownlint | tee /tmp/markdownlint-mdtablefix-cli-matrix-testing.out
make nixie | tee /tmp/nixie-mdtablefix-cli-matrix-testing.out
git diff --check | tee /tmp/diff-check-mdtablefix-cli-matrix-testing.out
```

Expected result: every command exits successfully. If `make nixie` fails due to
an environmental browser issue rather than a Markdown diagram problem, record
the exact failure in this plan and ask for direction before committing.

## Acceptance criteria

- `Cargo.toml` and `Cargo.lock` include the `insta` dev dependency with a
  Cargo-compatible caret requirement.
- `tests/data/cli-matrix/` contains only `.dat` matrix input fixtures.
- `tests/cli_matrix.rs` uses `assert_cmd` to run the real `mdtablefix` binary.
- `tests/cli_matrix.rs` uses `insta` to snapshot labelled result envelopes.
- Harness self-tests fail if case identifiers are duplicated, fixture paths are
  missing, fixtures do not use `.dat`, stdout or `--in-place` expansion is
  incomplete, wrapped or unwrapped expansion is incomplete, or pairwise
  transform coverage is lost.
- Every logical matrix combination is executed in both file-to-stdout mode and
  `--in-place` mode.
- Every selected non-wrap flag combination is executed both with and without
  `--wrap`.
- The base matrix covers every enabled/disabled pair combination for
  `--renumber`, `--breaks`, `--ellipsis`, `--fences`, `--footnotes`,
  `--code-emphasis`, and `--headings`; generated variants then add the
  required `--wrap` coverage.
- `docs/developers-guide.md` documents the harness and snapshot update
  workflow.
- `make fmt`, `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, `make nixie`, and `git diff --check` pass, with tee logs
  recorded under `/tmp`.

## Rollback plan

If the implementation creates noisy or unstable snapshots, revert the harness
commit and keep this ExecPlan as the record of the failed approach. If only a
specific fixture or case set is noisy, remove that fixture, update
`MATRIX_CASES`, rerun the harness self-tests, and record the narrower case set
in `Decision Log` before proceeding.
