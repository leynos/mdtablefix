# Repository layout

This document explains the major paths in the `mdtablefix` repository and the
responsibilities attached to each one. It is the canonical location for
repository-layout guidance.

## Tree overview

The following tree is a simplified orientation map. It omits generated build
output and most test fixtures.

```plaintext
.
├── AGENTS.md
├── CHANGELOG.md
├── CRUSH.md
├── Cargo.lock
├── Cargo.toml
├── Makefile
├── README.md
├── docs/
│   ├── adrs/
│   ├── execplans/
│   ├── architecture.md
│   ├── contents.md
│   ├── developers-guide.md
│   ├── documentation-style-guide.md
│   ├── repository-layout.md
│   └── users-guide.md
├── src/
│   ├── fences/
│   ├── footnotes/
│   ├── reflow/
│   ├── wrap/
│   └── *.rs
└── tests/
    ├── cli_matrix/
    ├── common/
    ├── data/
    ├── snapshots/
    └── *.rs
```

_Figure 1: Simplified repository tree._

## Top-level files

- `AGENTS.md`: Agent-facing repository instructions. The copy in this
  repository is authoritative for future agent work.
- `CRUSH.md`: Companion agent instructions kept in step with the project
  guidance where applicable.
- `Cargo.toml`: Rust package metadata, dependency declarations, lint policy,
  and binary/library target configuration.
- `Cargo.lock`: Locked dependency graph for reproducible application builds.
- `Makefile`: Canonical command gateway for build, test, lint, formatting,
  Markdown linting, and Mermaid validation.
- `README.md`: Public project overview and quick-start material.
- `CHANGELOG.md`: User-facing release history.

## Documentation paths

- `docs/contents.md`: Index for the documentation set. Update it when
  documents are added, renamed, or removed.
- `docs/users-guide.md`: User-facing command-line behaviour, options,
  examples, and guarantees.
- `docs/developers-guide.md`: Maintainer workflows, quality gates,
  contribution practices, and implementation guidance.
- `docs/architecture.md`: Current system design and architectural rationale for
  the Markdown processing pipeline.
- `docs/documentation-style-guide.md`: Documentation style rules imported from
  the shared Rust agent template.
- `docs/adrs/`: Accepted architecture decision records. Add narrow decision
  records here when design choices need a durable audit trail.
- `docs/execplans/`: Living execution plans for larger implementation work.
  Keep these current while the related work is active.
- `docs/*.md`: Long-lived reference notes, process guides, and project
  roadmaps. Link each stable document from `docs/contents.md`.

## Source paths

- `src/main.rs`: Command-line entry point and application boundary.
- `src/lib.rs`: Library surface used by the binary and integration tests.
- `src/process.rs`: High-level document processing orchestration.
- `src/table.rs`: Markdown table parsing and rendering.
- `src/reflow.rs` and `src/reflow/`: Reflow coordination and focused reflow
  tests.
- `src/wrap.rs` and `src/wrap/`: Paragraph wrapping, inline parsing,
  continuation handling, link references, and related helpers.
- `src/fences.rs` and `src/fences/`: Fenced code block tracking and attachment
  behaviour.
- `src/footnotes.rs` and `src/footnotes/`: Footnote parsing, list handling,
  inline handling, and renumbering.
- `src/*.rs`: Focused Markdown transformations such as headings, breaks,
  frontmatter, HyperText Markup Language (HTML), lists, emphasis, ellipses,
  text processing, and input/output (I/O).

## Test paths

- `tests/*.rs`: Integration and behavioural tests for command-line workflows
  and Markdown transformations.
- `tests/common/`: Shared integration test support. Keep helpers explicit and
  avoid direct environment mutation.
- `tests/cli_matrix/`: Matrix-test support for command-line option combinations
  and invariants.
- `tests/data/`: Input and expected-output fixtures. Treat these as reviewable
  test contracts.
- `tests/snapshots/`: `insta` snapshot outputs for multivariant command-line
  behaviour. Update intentionally and review diffs before committing.

## Generated and local artefacts

The `target/` directory is Cargo build output and is intentionally not part of
the source tree. Do not store hand-authored documentation, fixtures, or release
material there. Use `/tmp` only for logs and scratch output produced while
running validation commands.
