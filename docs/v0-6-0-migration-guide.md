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

## Preserved file mode

- **What changed:** The target's permissions are copied to the temporary file
  before the swap on POSIX systems, so the rewritten file keeps its mode. On
  Windows a read-only target has its read-only attribute cleared for the
  duration of the rename and the target's permissions reapplied afterwards, so
  a read-only file is still replaced and is still read-only once the run
  finishes.
- **Who is affected:** Anyone who rewrites files in place.
- **Migration action:** No action is required.

## Read-only targets are replaced

- **What changed:** A read-only target in a writable directory is now rewritten
  successfully, because the swap needs write permission on the containing
  directory rather than on the file itself. On Windows the rename cannot
  replace a read-only destination, so its read-only attribute is cleared for
  the duration of the rename and reapplied afterwards; a read-only file is
  still replaced and is still read-only once the run finishes.
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
