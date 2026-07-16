# Architecture Decision Record (ADR) 0001: Preserve table structure during reflow

- Status: Accepted
- Date: 2026-04-22

## Context

`mdtablefix` reflows Markdown tables by parsing buffered lines into logical
rows, calculating column widths, and then formatting aligned output. Recent
continuation-row fixes exposed three coupled failure modes:

- Rows with empty leading cells, such as `|   |   | more text |`, lost their
  original column positions during the global row split.
- Rows that contained escaped pipes, such as `\|`, could be reconstructed with
  literal `|` characters and then split into too many cells on the next parse.
- Table widths drifted when ellipsis replacement ran after reflow, because `...`
  and `…` occupy different display widths in the rendered output.

These regressions produced malformed tables and markdownlint failures,
including inconsistent column counts and separator widths.

## Decision

The table reflow pipeline now follows these rules:

- Protect leading empty continuation cells with a private marker before
  structural row parsing.
- Preserve physical source-line boundaries, and use the inferred table width
  to recover only complete legacy rows concatenated on one line instead of
  encoding row boundaries as sentinel cell content.
- Restore the protected cells only after parsing has completed.
- Re-escape literal pipe characters in non-leading cells when rebuilding a
  protected row, so reparsing preserves the original cell boundaries.
- Measure column widths with `UnicodeWidthStr::width` and keep separator
  columns at a minimum width of three dashes while preserving alignment markers.
- Apply ellipsis replacement to buffered table lines before calling
  `reflow_table`, so the formatter sees the final cell contents.

## Consequences

- Continuation rows keep their original column positions, even when the first
  cells are empty.
- Escaped pipes remain literal content instead of becoming accidental
  delimiters during reparsing.
- Tables that contain wide Unicode characters or ellipsis substitutions align
  by rendered width rather than byte length.
- Literal cell content cannot be mistaken for an in-band row-boundary marker.
- The parser carries a private marker for leading empty continuation cells and
  re-escapes literal pipes in non-leading cells during row rebuilding, which
  keeps the behaviour deterministic and testable.
