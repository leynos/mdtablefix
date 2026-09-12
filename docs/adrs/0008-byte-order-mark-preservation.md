# Architectural decision record (ADR) 0008: Split the byte-order mark from the document body

## Status

Accepted.

## Date

2026-09-11.

## Context and problem statement

Windows editors may begin a UTF-8 file with a byte-order mark (BOM), the
character U+FEFF, as an encoding hint. Read as text, that character arrives as
the first character of the first line, so a marked table arrives as
`\u{FEFF}|A|B|`, which matches no table pattern and no heading rule. The first
line is therefore invisible to every transform.

The failure is worse than a missed fix. `--check` compares the file's bytes
with what the formatter would write, and a marked file whose content is ragged
would compare equal to itself and be reported as clean: a silent false negative
in the one mode a repository is meant to gate on. A tool that then wrote output
without the mark would delete it, turning a formatting pass into an encoding
change.

The mark is not content. It describes how the file is encoded, it belongs to
one position in the file, and no Markdown transform has an opinion about it.

## Decision drivers

- Fidelity: a document that already carries a mark must still carry exactly one
  after a rewrite, and a document that carries none must not gain one.
- Isolation: no transform module should know the mark exists, for the same
  reason none knows which terminator the source file uses.
- Detection: the mark must not be counted as content or as a line ending when
  the line-ending style is chosen.
- Determinism: a marked document and its unmarked twin must produce the same
  Markdown decisions, differing only in the mark.

## Options considered

### Option A: Split the mark before formatting and restore it after

Parse the document once into "has a mark" plus the body, format the body, and
re-attach the mark when the result is rendered.

Advantages: every transform sees the same lines whether or not the file is
marked; the mark survives a rewrite exactly when one was present; a marked file
compares equal to itself under `--check`. Disadvantages: the parse and the
render must be kept together, or a caller could render one document's lines
with another's mark.

### Option B: Strip the mark permanently

Remove the mark on read, and never write one.

Advantages: no state to carry. Disadvantages: this is the defect, not a fix — a
formatting run would silently change the file's encoding, and a marked file
would never be a fixed point under `--check`.

### Option C: Always emit a mark

Normalize every output document to carry a mark, whether or not the input did.

Advantages: one output shape. Disadvantages: it rewrites every file in a
repository that has never used marks, for no Markdown reason, and it makes the
formatter's output depend on a decision the input did not express.

### Option D: Leave the mark attached and teach each transform to skip it

Advantages: no new type. Disadvantages: it puts an encoding concern into every
pattern that matches at the start of a line, and each new transform becomes a
new chance to forget it.

| Topic                          | Option A | Option B | Option C | Option D |
| ------------------------------ | -------- | -------- | -------- | -------- |
| Marked file is a fixed point   | yes      | no       | yes      | no       |
| Unmarked file stays unmarked   | yes      | yes      | no       | yes      |
| Transforms know about the mark | no       | no       | yes      | yes      |
| One parse, one render          | yes      | yes      | yes      | no       |

_Table 1: Comparison of byte-order-mark options._

## Decision outcome

Option A. The mark is a boundary concern, handled by `SourceDocument` in
`src/io/document.rs`, which binds the mark, the body, and the line-ending
counts of the document they came from:

- `SourceDocument::parse(content)` splits one leading U+FEFF from the body and
  counts the line endings of the body.
- `body()` borrows the text a transform is given, with no mark and with its line
  endings intact.
- `render(&lines)` returns the formatted lines terminated with the document's
  selected line ending, preceded by the mark when the document had one.
- The methods are on one type rather than free functions, so a caller cannot
  render one document's lines with another document's mark or ending.

Two consequences of the parse are deliberate:

- **Counting starts at the body.** The mark is not a line ending, and on a
  document that is nothing but a mark it would otherwise be the only reason a
  majority existed, so the line-ending vote would be decided by a character
  that is not a line ending at all.
- **An empty render keeps the mark.** A marked document that formats to no lines
  renders as the mark alone, not as nothing. This keeps a document that
  produces no output byte-identical to its input, so `--check` stays a fixed
  point and `--in-place` cannot destroy the mark by removing everything around
  it.

The mark is reported, not acted on beyond this: `--check` compares the original
bytes with the rendered bytes, so a marked file that needs no Markdown change
reports no drift, and `--in-place` writes only when those bytes differ.

## Consequences

- A marked file that needs no Markdown change is reported as unchanged, where
  before the mark made a ragged document look clean and a clean document look
  as if it needed no work for the wrong reason.
- A rewrite preserves the mark exactly when one was present; a file that had
  none never acquires one, and a file that had one does not lose it.
- The line-ending majority is computed over the body, so a mark cannot swing the
  vote on a document that is otherwise uniformly terminated.
- Standard input goes through the same boundary, so a marked document piped in
  behaves as it does in a file. An input that produces no lines prints the bare
  terminator rather than the mark, because nothing is written back on that path.

## Known risks and limitations

- Only one leading mark is removed. A second U+FEFF at the start of the body is
  content, as is any mark later in the document.
- The mark is treated as the character U+FEFF in the decoded text. The tool
  reads UTF-8 and does not detect UTF-16 or UTF-32, where a byte-order mark
  takes a different form.
- A marked document that formats to no lines and is printed to standard output
  is rendered by the standard-input path's own rule, which prints one
  terminator; the mark-preserving empty render applies to the file paths.
