# Architectural decision record (ADR) 0009: Report drift from one shared assessment

## Status

Accepted.

## Date

2026-09-11.

## Context and problem statement

A formatter used as a repository gate must be able to answer "would this file
change?" without changing it, and must make that answer usable by both a human
reading a log and a script deciding whether to fail a build. Issue #452 asked
for exactly that: a check-only mode that lists the files needing reformatting,
a readable diff for the files that need it, and an exit status a pipeline can
branch on.

The risk in adding such a mode is divergence. If the reporting path formats
files by its own route, then `--check` and `--in-place` can disagree: a file
reported as needing changes that `--in-place` then leaves alone, or worse, a
file reported as clean that `--in-place` rewrites. A gate that disagrees with
the writer it is meant to gate is not a gate, and the disagreement would be
invisible until someone compared two runs.

Two smaller hazards follow from the same place. A source file is a whole
document — byte-order mark, line endings, and all — so a comparison made on
body text alone would call a file unchanged while a rewrite would still alter
its bytes. And a reporting mode must not be able to write even by mistake,
because the mode's whole value is the guarantee that it did not.

## Decision drivers

- One formatting decision per file, shared by every mode, so `--check`,
  `--diff`, `--in-place`, and printing cannot disagree.
- Read-only by construction for the reporting modes, not by convention.
- A stable, machine-parseable standard output; human summary text elsewhere.
- Deterministic output: no timestamps, no colour, no dependence on completion
  order, no dependence on which diff algorithm happened to be affordable.
- An exit-status contract that distinguishes "clean", "drift", and "could not
  be assessed", with the last outranking the middle.

## Options considered

### Option A: One assessment, rendered per mode

Read the file once, format it once with the shared closure, decide changed or
unchanged by comparing bytes, then render the finding in the shape the selected
mode asks for.

Advantages: divergence is impossible, because there is only one formatting
decision and only one change decision. Disadvantages: the assessment's two
strings are live at the same time, so peak memory per file is roughly twice its
size while it is being analysed.

### Option B: Separate check and write paths

Give the check-only mode its own formatting and comparison route.

Advantages: each path can be tuned alone. Disadvantages: this is the divergence
risk the record exists to prevent; the two routes would have to be kept in step
by review rather than by construction.

### Option C: `--check` prints the diff, with `--concise` for the file list

Implement issue #452 literally: `--check` emits a diff per changed file, and a
further flag narrows the output to filenames.

Advantages: matches the issue's wording. Disadvantages: the terse form is the
one a CI log wants and the diff is the one a human wants; making the diff the
default forces every pipeline to pass a second flag to get the quiet form, and
the two cannot be requested together.

### Option D: `--check` lists files, and no diff mode

Advantages: smallest surface. Disadvantages: a user who wants to see what would
change must run `--diff`-equivalent tooling externally, which is where
disagreement with the formatter starts.

| Topic                        | Option A | Option B | Option C | Option D |
| ---------------------------- | -------- | -------- | -------- | -------- |
| Modes can disagree           | no       | yes      | no       | no       |
| Quiet output is the default  | yes      | yes      | no       | yes      |
| Diff available from the tool | yes      | yes      | yes      | no       |
| Read-only enforced by type   | yes      | no       | yes      | yes      |

_Table 1: Comparison of reporting-mode designs._

## Decision outcome

Option A. The command line exposes four modes, and the driver in
`src/driver.rs` decides everything they share:

- `Mode` is a closed set: `Print` (no flag), `InPlace`, `Check`, and `Diff`.
  `Mode::reports` names the two that describe files instead of printing them,
  and those two are exactly the modes that fail on drift; `Mode::verb` names
  the operation for error contexts, so a failure says "writing" only where a
  write was attempted.
- `Assessment` holds the file's bytes as read and the text the formatter would
  write. `Assessment::is_changed` compares those two strings directly, so the
  decision is made on the whole document rather than on body text, and a
  document whose mark or terminators would change is not mistaken for a clean
  one.
- `ReadOnlyDir` is a newtype over the directory capability with no write method.
  The reporting modes receive it instead of the writable `Dir`, so "a reporting
  mode cannot write" is enforced by the type rather than by a test double.
- The formatter is one closure of type `Formatter`, built once per run and
  passed by reference to every mode. There is no second formatting route to
  keep in step.
- `exit_status(mode, any_drift, any_error)` is a pure function. An error yields
  exit `2` in every mode; drift yields exit `1` only where `Mode::reports`, and
  `--in-place` over drifting files therefore exits `0`, as does a bare
  invocation whose contract is to print the formatted text.
- `in_argument_order(results)` restores argument order from explicit indices,
  because the parallel collection's order is not a documented guarantee; a
  report list must not depend on which worker finished first.
- Findings are rendered per mode: `--check` prints one line per drifting file
  as `<path> +<insertions> -<deletions>`, and `--diff` prints a unified diff
  per drifting file. A clean file prints nothing in either mode.

The two rendering shapes are the whole of `--concise`'s intent, so issue #452's
`--concise` flag is superseded rather than implemented: `--check` is already
the quiet form — one line per changed file, no diff content — and `--diff` is
the verbose one. A third flag would have had to name one of two renderings of
the same assessment, and the two modes already name them.

Streams are part of the contract. Standard output carries only report lines or
diffs, so it can be piped; the human summary
(`2 files would be reformatted, 1 file left unchanged.`) and every error go to
standard error. Diff headers name the path on both sides with `/` separators,
no timestamps, and no colour, and the renderer switches from Myers to Patience
above 1,000 lines on either side: a line threshold rather than a time budget,
because a timeout would make the output depend on how fast the machine was.

## Consequences

- `mdtablefix --check FILE...` is a gate: exit `0` when nothing would change,
  `1` when something would, `2` when a file could not be read. An error outranks
  drift, so a partly unreadable run cannot report "clean".
- `mdtablefix --diff FILE...` shows what would change without changing it, and
  exits like `--check`.
- A file that cannot be read is reported on standard error, counted, and the run
  continues, so one unreadable file does not abandon the rest. Exit `2` still
  follows, because a run that could not assess every file cannot claim the tree
  is clean.
- The mode flags require an input source in the command line's own grammar, so a
  mode flag with no files is a usage error rather than a silent fall-through to
  standard input.
- `--in-place` writes only files whose bytes would change, so a clean file keeps
  its inode and modification time. See the user's guide for the one visible
  consequence, a symlink to an already-clean file.
- Issue #452's acceptance criteria are met by `--check` and `--diff` together,
  with the diff rendered by `--diff` rather than by `--check`; the issue is
  closed as superseded on that basis rather than left open for `--concise`.

## Known risks and limitations

- The counts in a report line come from a line diff between the input and the
  formatted text, so they describe lines replaced rather than edits a reviewer
  would count.
- `--diff`'s output can differ between the two diff algorithms for the same
  conceptual change above the 1,000-line threshold, but it is deterministic for
  a given input.
- A shell glob that matches nothing passes its literal pattern to the tool, so
  `mdtablefix --check *.md` in a directory with no Markdown files is an
  unreadable-path error (exit `2`) rather than an empty success. The user's
  guide records the workaround.
- `--in-place` declines a symlinked target rather than replacing the link, so a
  symlink to a file that needs reformatting fails that file's analysis. This is
  inherited from the atomic replacement policy, not introduced here.
- The gate is only as good as the formatter's idempotence. A transform that is
  not a fixed point makes `--check` report drift that a further `--in-place`
  pass does not clear; issue #474 records one such class under `--headings`.
