# Architectural decision record (ADR) 0007: Preserve the majority input line-ending style

## Status

Accepted.

## Date

2026-09-10.

## Context and problem statement

`mdtablefix` read a document with `str::lines()` and rejoined the formatted
lines with a literal `"\n"`, so every rewritten file gained Unix line feed
endings even when the input used Windows carriage return and line feed (CRLF)
endings. Reformatting a CRLF document therefore produced a whole-file diff
whose only change was the terminator of each line. That buries the Markdown
changes a reviewer needs to see, and it stops a check-only formatting gate from
comparing formatter output with valid CRLF source files.

A formatter must not create a diff that changes nothing but line endings in a
document whose endings are consistent and which already ends with a terminator.

## Decision drivers

- Reformatting must be a no-op for an already-formatted document whose line
  endings are consistent, whatever line-ending style that document uses.
- The selected style must be deterministic: the same input always produces the
  same output, with no dependence on platform, locale, or read order.
- The transform pipeline must stay line-ending agnostic, so no Markdown
  transform needs to know which terminator the source file uses.

## Options considered

### Option A: Preserve the majority style

Count the CRLF pairs and the lone line feed endings in the raw input, and
terminate every output line with whichever style occurs more often.

Advantages: a consistently authored file is preserved exactly, so an
already-formatted document that ends with a terminator is rewritten
byte-for-byte. Disadvantages: a mixed-style document is homogenized, so an
LF-authored snippet inside a mostly-CRLF document is rewritten to CRLF.

### Option B: Preserve the first ending seen

Terminate every output line with the style used by the first line of the
document.

Advantages: cheap, and simple to explain. Disadvantages: a single stray ending
at the top of the file decides the style for the whole document, which is a
worse approximation of author intent than a majority vote.

### Option C: Normalize everything to LF

Keep the existing behaviour and document it as a limitation.

Advantages: no code change. Disadvantages: this is the defect the record
addresses, and it prevents CRLF-oriented repositories from adopting the
formatter without a whole-file rewrite.

| Topic                      | Option A      | Option B          | Option C        |
| -------------------------- | ------------- | ----------------- | --------------- |
| Already-formatted CRLF doc | unchanged     | unchanged         | fully rewritten |
| Mixed-ending document      | majority wins | first ending wins | LF              |
| Deterministic              | yes           | yes               | yes             |
| Cost per document          | one scan      | one scan          | none            |

_Table 1: Comparison of line-ending options._

## Decision outcome

Option A. The `io` module owns the policy through the following public items:

- `LineEnding`, a closed set of the terminators the formatter can emit, with
  `LineEnding::as_str` returning the characters written between lines.
- `detect_line_ending(text) -> LineEnding`, the majority rule. CRLF pairs are
  counted first and subtracted from the total line feed count to obtain the
  lone line feeds; counting line feeds directly would count every CRLF twice
  and leave CRLF unable to win. CRLF is selected only when it strictly
  outnumbers lone line feeds.
- `count_line_endings(text) -> LineEndingCounts`, the same query returned
  together with the `crlf_count` and `lone_lf_count` that decided it, so a
  reporting boundary can state the vote without restating the counting rule.
- `count_line_endings_reported(text, operation, path)`, the same query with the
  reporting boundary attached: it returns identical counts and emits one
  `debug` event with the `crlf_count`, `lone_lf_count`, and `selected_ending`
  fields, plus `operation` and `path` where the boundary has them.
- `serialize_lines(lines, ending) -> String`, which joins the processed lines
  with the selected terminator and appends one further terminator so a
  non-empty result always ends with a line ending.

An exact tie, and a document with no line endings at all, select LF. The
tie-break is arbitrary but total, so the choice never depends on which style
happens to appear first.

Detection happens at the input/output (I/O) boundary only. `str::lines()` still
splits the document, so every line handed to a transform is free of its
terminator, and `serialize_lines` re-applies the detected style on the way out.
No transform module is aware of line endings.

## Consequences

- A file whose endings are mostly CRLF is emitted entirely as CRLF, and a file
  whose endings are mostly LF is emitted entirely as LF.
- An already-formatted document whose endings are consistent, and which already
  ends with a terminator, is rewritten with identical bytes, so a check-only
  gate can compare formatter output with the source file directly. An
  unterminated non-empty file gains one terminator, and a mixed-ending file is
  homogenized to the majority style.
- The library functions `rewrite` and `rewrite_no_wrap` change their observable
  output for CRLF input. That byte change is the point of the decision.
- Endings inside fenced code blocks are homogenized too, because detection is
  per document rather than per region.
- An empty file still produces empty output, while a non-empty file always
  ends with one terminator.
- Standard input differs only when empty: the style detected on non-empty
  standard input selects the terminators written to standard output, while
  empty standard input still prints one terminator, which is LF because there
  is no ending to detect.

## Known risks and limitations

- A mostly-CRLF document that embeds an LF-authored code sample has that
  sample rewritten to CRLF, which is a content change rather than a formatting
  change. Issue #451 specifies majority detection, and `docs/users-guide.md`
  records the consequence.
- A lone carriage return is not treated as a line ending, because `str::lines`
  splits only on line feeds. Such a carriage return stays inside its line as
  literal content, as it did before this decision.
