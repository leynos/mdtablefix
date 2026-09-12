# Migrating to 0.6.0

This note is for users of the `mdtablefix` command-line interface (CLI) and
consumers of the library application programming interface (API). It covers
the behaviour changes in the 0.6.0 release and the migration action required
for each one ([#465](https://github.com/leynos/mdtablefix/issues/465)).

## Atomic in-place replacement

- **What changed:** `--in-place`, `mdtablefix::io::rewrite`, and
  `mdtablefix::io::rewrite_no_wrap` now write the new content to a temporary
  file beside the target, flush and sync it, and rename it over the target with
  the target's permissions preserved. A failure before the rename leaves the
  original file byte-identical rather than truncated.
- **Who is affected:** Anyone who rewrites files in place, whether through the
  CLI or the library.
- **Migration action:** No action is required for normal use. A successful
  rewrite of a regular file creates a new file that takes over the target's
  name, so handles, hard links, and watches tied to the previous file keep the
  old contents and do not follow the replacement. A failed rewrite, or a
  target declined for being a symbolic link, leaves the original file
  untouched.

## Line-ending preservation

- **What changed:** The formatter now terminates every line of its output with
  the line-ending style holding the strict majority of the input document's
  line endings, rather than always emitting line feeds (LFs). CRLF pairs and
  lone line feeds are counted, and CRLF is selected only when it strictly
  outnumbers the lone line feeds. An exact tie, and an input with no line
  endings at all, select LF, so the result is deterministic. Only CRLF and lone
  LF are recognized; a lone carriage return is content. The change covers file
  arguments printed to standard output, `--in-place`, standard input, and the
  library entry points `mdtablefix::io::rewrite` and
  `mdtablefix::io::rewrite_no_wrap`; standard input keeps its existing contract
  of printing one terminator even when the output has no lines, while an empty
  file still produces empty output. Five items are new public API: `LineEnding`,
  whose `as_str` method returns the characters written between lines;
  `LineEndingCounts`, the counts behind a selection;
  `detect_line_ending(text) -> LineEnding`;
  `count_line_endings(text) -> LineEndingCounts`; and
  `serialize_lines(lines, ending) -> String`. The boundary that acts on the
  selection reports it at `debug` level, under the message
  `selected the majority line ending`.
- **Who is affected:** Anyone who formats a document with CRLF or mixed
  endings, through the CLI or the library.
- **Migration action:** No action is required for normal use. A file authored
  with one style keeps it, so a CRLF file is no longer rewritten as LF, and the
  whole-file diff that changed nothing but terminators disappears. An
  already-formatted document whose endings are consistent, and which already
  ends with a terminator, is rewritten with identical bytes, so it can be
  compared with formatter output byte-for-byte. Because the choice is made per
  document, a mostly-CRLF document is emitted entirely as CRLF, so an
  LF-authored snippet inside it is rewritten to CRLF, and a mixed-ending
  document is normalized to its majority style on the first run; that diff can
  be larger than the table changes alone.

## Preserved file mode

- **What changed:** The target's permissions are copied to the temporary file
  before the swap, so the rewritten file keeps its mode and a read-only target
  is replaced by a read-only file rather than by a writable one. On Windows the
  destination's `FILE_ATTRIBUTE_READONLY` is cleared immediately before the
  rename, because it otherwise blocks the swap; a swap that does not complete
  puts the original attribute back.
- **Who is affected:** Anyone who rewrites files in place.
- **Migration action:** No action is required.

## Read-only targets are replaced

- **What changed:** A read-only target in a writable directory is now rewritten
  successfully, because the swap needs write permission on the containing
  directory rather than on the file itself, and because the replacement takes
  over the read-only state instead of losing it. Windows records read-only as
  `FILE_ATTRIBUTE_READONLY`, which blocks the rename, so `mdtablefix` clears the
  destination's attribute immediately before the swap and puts the original
  attribute back if the swap does not complete. A run interrupted between those
  two steps, or a restoration that itself fails, can leave the target's
  read-only attribute cleared; the contents are unchanged either way, because a
  swap that does not complete leaves the original file byte-identical.
- **Who is affected:** Workflows that relied on `--in-place` failing for
  read-only files as a guard.
- **Migration action:** Check the file mode before invoking `mdtablefix` where
  the previous failure acted as a guard.

## Symbolic links are declined

- **What changed:** A symlinked target now fails with `InvalidInput` naming the
  link, instead of replacing the link entry with a regular file and leaving the
  real file untouched.
- **Who is affected:** Anyone who passes a symbolic link as the target.
- **Migration action:** Pass the link's target path, or resolve the link before
  invoking `mdtablefix`.

## Stale temporary files

- **What changed:** Temporary files are named
  `<target>.mdtablefix-<pid>-<n>.tmp` and are removed on a best-effort basis
  when a run fails. A leftover file from a killed run only costs one retry,
  because up to 16 candidate names are tried.
- **Who is affected:** Anyone who inspects the target directory, and any run
  that is killed before it completes.
- **Migration action:** Delete stale files once no run is in progress. They are
  always safe to delete.

## Full error chain

- **What changed:** A failed file reports the whole error chain, so a declined
  rewrite states its reason as well as the file being written.
- **Who is affected:** Scripts that match exact standard-error text.
- **Migration action:** Update scripts that match exact standard-error text
  where the added detail breaks an assertion.

## Exit status for operational errors

- **What changed:** A run that cannot read or rewrite a file now exits `2`,
  where earlier versions exited `1`. Exit `1` is reserved for drift: `--check`
  and `--diff` return it when at least one file would be reformatted.
- **Who is affected:** Scripts and continuous-integration jobs that branch on
  the status. A guard written as
  `mdtablefix ...; [ $? -eq 1 ] && handle_failure` stops firing, and a test
  that asserts only "non-zero" cannot tell the two failures apart.
- **Migration action:** Test for the status you mean. Use `--check` and read
  its `1` when the question is whether the tree drifts, and treat `2` as an
  operational failure in every mode. An error outranks drift, so `2` is the
  status to alert on.

## Unchanged files are not rewritten

- **What changed:** `--in-place` writes only the files whose bytes would
  change. A file that is already formatted keeps its inode and its
  modification time, and a symbolic link to such a file now succeeds because
  no write is attempted.
- **Who is affected:** Build systems that use modification time for staleness
  checks, and anyone who watches inodes to detect rewrites.
- **Migration action:** None. A clean tree no longer looks modified.

## Read-only reporting modes

- **What changed:** `--check` reports each file that would be reformatted, as
  its path followed by the line delta, and `--diff` prints a unified diff for
  each of them. Both are read-only: a clean file prints nothing. A mode flag
  requires at least one file path, so `mdtablefix --check` with no files is a
  usage error, and at most one of `--in-place`, `--check`, and `--diff` may be
  given.
- **Who is affected:** Anyone adding a formatting gate to a pipeline.
- **Migration action:** None. The modes are additive; see the
  [user's guide](users-guide.md#command-line-usage) for the report line format
  and the exit-status contract.

## New library entry point

- **What changed:** `mdtablefix::io::replace_file` atomically replaces a target
  inside an already-open directory capability. The entry point is additive, so
  `rewrite` and `rewrite_no_wrap` keep their signatures.
- **Who is affected:** Library consumers that already hold a
  `cap_std::fs_utf8::Dir` capability.
- **Migration action:** No action is required.

```rust,no_run
pub fn replace_file(
    directory: &cap_std::fs_utf8::Dir,
    path: &camino::Utf8Path,
    contents: &str,
) -> std::io::Result<()>
```

## Replacement metrics

- **What changed:** `mdtablefix::io::replace_file`, and therefore `rewrite`,
  `rewrite_no_wrap`, and `--in-place`, now emit three bounded counters and one
  bounded histogram through the `metrics` façade. The crate installs no
  recorder.
- **Who is affected:** Library consumers and host applications that install a
  metrics recorder.
- **Migration action:** No action is required for normal use. A host that wants
  the counters installs a recorder once at startup with
  `metrics::set_global_recorder(...)`. The
  [Metrics](developers-guide.md#metrics) section of the developer's guide
  lists the metric names and labels.
