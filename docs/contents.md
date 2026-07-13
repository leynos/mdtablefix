# Documentation contents

- [Documentation contents](contents.md): Index for the repository
  documentation set.

## User and maintainer guides

- [User's guide](users-guide.md): Command-line usage, formatting behaviour,
  options, and examples for people using `mdtablefix`.
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

## Decision records

- [Table reflow pipeline](adrs/0001-table-reflow-pipeline.md): Accepted
  decision covering the table parsing and rendering pipeline.
- [Textwrap wrapping engine](adrs/0002-textwrap-wrapping-engine.md): Accepted
  decision covering the adoption of `textwrap` for paragraph wrapping.
- [Treat prose dates as inline fragments](adrs/0003-date-sequences-as-inline-fragments.md):
  Accepted
- [Keep bespoke state machines explicit](adrs/0004-state-machine-abstractions.md):
  Accepted
- [Protect literal regions during ellipsis replacement](adrs/0005-ellipsis-literal-regions.md):
  Accepted decision covering links, URLs, and filesystem-like tokens.

## Reference material

- [Rust doctest dry guide](rust-doctest-dry-guide.md): Guidance for writing
  Rust documentation tests that avoid brittle or misleading examples.
- [Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md):
  Reference for the fixture and parameterization patterns used in tests.
- [Trailing spaces](trailing-spaces.md): Notes on preserving Markdown hard line
  breaks and other trailing-space-sensitive content.

## Execution plans

- [CLI matrix testing](execplans/cli-matrix-testing.md): Plan for command-line
  matrix coverage.
- [Nested code block handling](execplans/issue-262-nested-code-block-handling.md):
  Plan for nested code block handling.
- [Wrapping replacement](execplans/replace-bespoke-wrapping-with-textwrap-and-unicode-width.md):
  Plan for replacing bespoke wrapping internals with `textwrap` and
  `unicode-width`.
- [Parallel processing roadmap](execplans/parallel-processing-roadmap.md):
  Roadmap for parallel processing work.
- [YAML frontmatter](execplans/yaml-frontmatter.md): Plan for YAML
  frontmatter handling.
