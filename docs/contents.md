# Documentation contents

- [Documentation contents](contents.md): Index for the repository
  documentation set.

## User and maintainer guides

- [User's guide](users-guide.md): Command-line usage, formatting behaviour,
  options, and examples for people using `mdtablefix`.
- [Migrating to 0.6.0](v0-6-0-migration-guide.md): Behaviour changes in the
  0.6.0 release and the actions they require.
- [Developer's guide](developers-guide.md): Build, test, release,
  contribution, and maintainer workflows for the project.
- [Repository layout](repository-layout.md): Ownership boundaries and
  conventions for the repository tree.
- [Documentation style guide](documentation-style-guide.md): Markdown,
  spelling, document-type, and roadmap conventions used by this repository.
- [Release process](release-process.md): Release checklist and publication
  expectations.

## Design and architecture

- [Architecture](architecture.md): Current design for the Markdown parsing,
  wrapping, table reflow, and processing pipeline.
- [Verification ledger](verification.md): Proof claims, trusted boundaries,
  and result classes for maintainers reviewing verified formatter kernels.

## Decision records

- [Table reflow pipeline](adrs/0001-table-reflow-pipeline.md): Accepted
  decision covering the table parsing and rendering pipeline.
- [Textwrap wrapping engine](adrs/0002-textwrap-wrapping-engine.md): Accepted
  decision covering the adoption of `textwrap` for paragraph wrapping.
- [Treat prose dates as inline fragments](adrs/0003-date-sequences-as-inline-fragments.md):
  Accepted
- [Keep bespoke state machines explicit](adrs/0004-state-machine-abstractions.md):
  Accepted
- [Ellipsis literal-region protection](adrs/0005-ellipsis-literal-regions.md):
  Accepted decision covering links, URLs, and filesystem-like tokens.
- [Single-pass idempotence](adrs/0006-single-pass-idempotence.md): Accepted
  decision covering the fixed-point guarantee for the formatting pipeline.
- [Majority line-ending preservation](adrs/0007-line-ending-detection.md):
  Accepted decision covering how the formatter selects the output line-ending
  style.
- [Verified normalization core](adrs/0008-verified-normalization-core.md):
  Accepted decision covering production-used Verus kernels and their ledger.
- [Byte-order-mark preservation](adrs/0008-byte-order-mark-preservation.md):
  Accepted decision covering how a leading byte-order mark is split from the
  document body and restored on render.
- [Check and diff reporting](adrs/0009-check-and-diff-reporting.md): Accepted
  decision covering the read-only reporting modes, the shared assessment, and
  the exit-status contract.

## Reference material

- [Rust doctest dry guide](rust-doctest-dry-guide.md): Guidance for writing
  Rust documentation tests that avoid brittle or misleading examples.
- [Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md):
  Reference for the fixture and parameterization patterns used in tests.
- [Trailing spaces](trailing-spaces.md): Notes on preserving Markdown hard line
  breaks and other trailing-space-sensitive content.
- [rstest-bdd user's guide](rstest-bdd-users-guide.md): Vendored guide to the
  `rstest-bdd` framework, whose scenarios drive the reporting tests.
- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md):
  Vendored guide to making tests deterministic by injecting their dependencies.

## Execution plans

- [CLI matrix testing](execplans/cli-matrix-testing.md): Plan for command-line
  matrix coverage.
- [Nested code block handling](execplans/issue-262-nested-code-block-handling.md):
  Plan for nested code block handling.
- [Code-block pipe lines](execplans/issue-373-code-block-pipe-line-trailing-pipe.md):
  Plan for stopping code-block pipe lines from gaining a trailing pipe.
- [Wrapping replacement](execplans/replace-bespoke-wrapping-with-textwrap-and-unicode-width.md):
  Plan for replacing bespoke wrapping internals with `textwrap` and
  `unicode-width`.
- [Parallel processing roadmap](execplans/parallel-processing-roadmap.md):
  Roadmap for parallel processing work.
- [YAML frontmatter](execplans/yaml-frontmatter.md): Plan for YAML
  frontmatter handling.
- [Check option](execplans/check-option.md): Plan for `--check`, `--diff`, the
  document boundary, and the exit-status contract.
- [State-machine abstractions roadmap](state-machine-abstractions-roadmap.md):
  Roadmap turning ADR 0004 into implementation work for the parser and wrapping
  state machines.
