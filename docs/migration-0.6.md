# Migrating to 0.6.0

This note is for users of the `mdtablefix` command-line interface (CLI) and
consumers of the library application programming interface (API). It covers
the behaviour changes in the 0.6.0 release and the migration action required
for each one ([#465](https://github.com/leynos/mdtablefix/issues/465)).

## Atomic in-place replacement

- **What changed:** `--in-place`, `mdtablefix::io::rewrite`, and
  `mdtablefix::io::rewrite_no_wrap` now write the new content to a temporary
  file beside the target, flush and sync it, copy the target's permissions
  across, and rename it over the target. A failure before the rename leaves the
  original file byte-identical rather than truncated.
- **Who is affected:** Anyone who rewrites files in place, whether through the
  CLI or the library.
- **Migration action:** No action is required for normal use. The target's
  inode changes on every rewrite, so tooling that pins the inode (file
  watchers, hard links, and open handles) does not follow the replacement.

## Preserved file mode

- **What changed:** The target's permissions are copied to the temporary file
  before the swap, so the rewritten file keeps its mode.
- **Who is affected:** Anyone who rewrites files in place.
- **Migration action:** No action is required.

## Read-only targets are replaced

- **What changed:** A read-only target in a writable directory is now rewritten
  successfully, because the swap needs write permission on the containing
  directory rather than on the file itself.
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
