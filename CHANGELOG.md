# Changelog

## [Unreleased]

### Added

- Release assets for `x86_64-apple-darwin`, `aarch64-apple-darwin` and
  `x86_64-pc-windows-msvc`, so `cargo binstall` can install `mdtablefix` on
  macOS and Windows without compiling it.
  ([#459](https://github.com/leynos/mdtablefix/issues/459))
- A `cargo-binstall` archive for `x86_64-unknown-freebsd`, which previously
  published only a bare binary. Every target the ungated `pkg-url` template
  resolves to now has an asset behind it.
- `scripts/package_release_artifacts.py`, a standard-library packager that
  stages the bare binary, the `cargo-binstall` archive and their `.sha256`
  sidecars identically on Linux, macOS and Windows runners.
- `scripts/verify_binstall_layout.py` and `tests/release_packaging.rs`, which
  render `pkg-url` and `bin-dir` from `Cargo.toml` and fail unless the staged
  archive matches, for every published target.
- A `binstall-packaging` continuous-integration job that builds, stages,
  extracts and runs the release binary on Ubuntu, macOS and Windows.
- A `windows-atomic-contract` continuous-integration job that runs the atomic
  replacement suites, and then the whole test suite, on a Windows runner, so a
  change that is correct only on Unix is caught by the pull request rather than
  by the release that follows it.
  ([#465](https://github.com/leynos/mdtablefix/issues/465))
- `--code-emphasis` flag to fix emphasis markers that adjoin inline code.
  Runs before wrapping and footnote conversion.
- Treat common English date sequences as atomic inline fragments during
  wrapping, including ordinal day, numeric day, and month-name-first forms.
  ([#346](https://github.com/leynos/mdtablefix/issues/346))
- The in-place replacement path emits bounded `metrics` counters and a
  `mdtablefix_io_replace_duration_seconds` histogram, so a host application
  that installs a recorder can watch replacements, their outcomes, and how long
  they take. The crate installs no recorder itself.
  ([#465](https://github.com/leynos/mdtablefix/issues/465))
- Add the line-ending helpers `LineEnding`, `LineEndingCounts`,
  `count_line_endings`, `detect_line_ending`, and `serialize_lines`, so a
  caller can select and apply the majority line-ending style of an input
  document through pure queries.
  ([#451](https://github.com/leynos/mdtablefix/issues/451))
- `--check` reports each file that would be reformatted, with the line delta
  the rewrite would make, and `--diff` prints a unified diff of the changes
  instead. Both are read-only, both exit `1` when a file would change, and
  neither writes anything.
  ([#452](https://github.com/leynos/mdtablefix/issues/452))

### Changed

- `--in-place`, `rewrite` and `rewrite_no_wrap` replace a read-only file in a
  writable directory instead of failing, because the atomic swap needs write
  permission on the containing directory rather than on the file itself, and the
  replacement inherits the target's permissions, read-only included. On Windows
  the destination's `FILE_ATTRIBUTE_READONLY` blocks the rename, so it is
  cleared immediately before the swap and put back if the swap does not
  complete.
  ([#465](https://github.com/leynos/mdtablefix/issues/465))
- `--in-place`, `rewrite` and `rewrite_no_wrap` decline a symbolic link instead
  of replacing the link entry with a regular file, which previously left the
  real file untouched while destroying the link. Rewrite the link's target
  directly.
  ([#465](https://github.com/leynos/mdtablefix/issues/465))
- A failed file reports the full error chain, so a declined rewrite states its
  reason rather than only the file being written. See
  [Migrating to 0.6.0](docs/v0-6-0-migration-guide.md) for the actions these
  changes require.
- Ungate the `[package.metadata.binstall]` configuration, which no longer
  applies only to 64-bit Linux GNU targets. One `pkg-url` template now covers
  Linux, macOS and Windows.
  ([#459](https://github.com/leynos/mdtablefix/issues/459))
- Checksum sidecars now name the asset alone rather than its staging path, so
  `sha256sum --check` works beside the downloaded file.
- Require callers of `FenceTracker::observe` and `FenceTracker::in_fence` to
  provide the current blockquote depth. This is a breaking API change for
  existing one-argument callers.
- Declare LF line endings for every tracked file in `.gitattributes`, so a
  Windows checkout hands the suites the fixture and snapshot bytes a Unix
  checkout sees rather than the CRLF that Git for Windows writes by default.
- Record the CLI matrix exit status as the numeric exit code rather than through
  `ExitStatus`'s `Display`, which spells an ordinary exit `exit status: 0` on
  Unix and `exit code: 0` on Windows. Envelopes now read `status: code: 0`,
  `status: code: <n>` for a non-zero exit, or `status: no exit code` when a
  signal ended the process, so the snapshots are platform-independent.
- Gate the `check-static-regexes` regression tests to Unix, because the guard
  they drive is a `bash` script that stands in for ripgrep with stub scripts
  carrying the executable bit. The Linux lint job still runs that guard over
  the same sources.
  ([#347](https://github.com/leynos/mdtablefix/issues/347))
- `format_breaks` now returns `Vec<Cow<'_, str>>` rather than `Vec<String>`,
  so unchanged lines stay borrowed instead of forcing heap allocations.
- Reserve exit `1` for drift and exit `2` for operational failure. A run that
  could not read or rewrite a file now exits `2` where it previously exited
  `1`, so a caller written as `mdtablefix ...; [ $? -eq 1 ]` needs updating.
  Drift fails a run only under `--check` and `--diff`, and an incomplete
  analysis is never reported as a clean tree. This is a breaking change for
  scripts, and the crate version becomes `0.6.0`.
  ([#451](https://github.com/leynos/mdtablefix/issues/451))
- Write only the files whose bytes would change under `--in-place`. A file that
  is already formatted keeps its inode and its modification time, and a
  symbolic link to such a file succeeds because no write is attempted.

### Fixed

- Emit formatted output with the line-ending style that holds the majority of
  the input's line endings, so a carriage return and line feed (CRLF) document
  is no longer rewritten as LF. A consistently ended document keeps its ending,
  while a mixed-ending document is normalized to the majority style. An exact
  tie, and a non-empty input with no line endings at all, select LF. An
  `--in-place` run over a CRLF file now writes CRLF rather than line feeds, and
  the change is not reversible for a file an earlier version already rewrote:
  its original endings are no longer recoverable from the file.
  ([#451](https://github.com/leynos/mdtablefix/issues/451))
- Format text adjoining an inline code span when it ends with a non-ASCII
  character, such as the ellipsis `--ellipsis` produces, instead of aborting on
  a character boundary. The emphasis split stepped one byte past the last
  character rather than one character past it.
- Write in-place output through a temporary file in the same directory and
  rename it over the target, so an interrupted run or a full disk can no longer
  leave a Markdown file truncated with no way to recover it. The original file
  mode is preserved, and a stale temporary file left by an abruptly killed run
  is retried past rather than reused.
  ([#465](https://github.com/leynos/mdtablefix/issues/465))
- Replace a read-only destination on Windows too, by clearing
  `FILE_ATTRIBUTE_READONLY` on the target immediately before the rename and
  putting the original attribute back if the swap does not complete, so a
  read-only file in a writable directory is replaced instead of failing. The
  temporary file carries the target's permissions into the rename, so the
  replacement is read-only as well.
  ([#465](https://github.com/leynos/mdtablefix/issues/465))
- Set the `cargo-binstall` `bin-dir` to `{ bin }{ binary-ext }`. The previous
  `.` rendered an empty source path, so `cargo binstall mdtablefix` failed
  before downloading anything.
  ([#458](https://github.com/leynos/mdtablefix/issues/458))
- Stop appending a trailing `|` to lone pipe-prefixed lines in code blocks, so
  shell pipeline continuations such as `| tee /tmp/test.log` are no longer
  corrupted into unterminated pipelines. A same-marker line carrying an info
  string is now treated as literal content rather than a closing fence, per
  CommonMark. ([#373](https://github.com/leynos/mdtablefix/issues/373))
- Reflow the prose following a joined cross-line inline-code span in the same
  `--wrap` pass, so formatter output is immediately idempotent.
  ([#375](https://github.com/leynos/mdtablefix/issues/375))
- Preserve authored line boundaries inside a cross-line inline-code span when
  joining it would exceed the wrap width and every authored source line already
  fits within that width, preventing new MD013 violations.
  ([#370](https://github.com/leynos/mdtablefix/issues/370))
- Document `--wrap` as a parameterless 80-column flag.
  ([#388](https://github.com/leynos/mdtablefix/issues/388))
- Keep reference-style links atomic while wrapping, so an opening bracket cannot
  be stranded before its label.
  ([#374](https://github.com/leynos/mdtablefix/issues/374))
- Keep colon-suffixed footnote references attached to preceding prose so they
  cannot become column-one footnote definitions.
  ([#372](https://github.com/leynos/mdtablefix/issues/372))
- Represent table row boundaries structurally so literal `ROW_END` cell content
  cannot corrupt reflow.
  ([#364](https://github.com/leynos/mdtablefix/issues/364))
- Preserve four-space and tab-indented code blocks when `--ellipsis` is used,
  so literal `...` in command output and source examples remains unchanged.
  ([#369](https://github.com/leynos/mdtablefix/issues/369))
- Preserve semantic `...` sequences in links, URLs, filesystem paths, and split
  link reference continuations when `--ellipsis` is used, preventing
  typographic normalization from changing resource destinations.
  ([#386](https://github.com/leynos/mdtablefix/issues/386))
- Keep hyphenated compounds containing an inline code span atomic during
  wrapping, including the leading-hyphen forms such as `` pre-`LLMPort` `` and
  `` (API-`Foo`) ``. The hyphen-prefix token is coupled forward to the
  following code span, mirroring the existing opening-punctuation behaviour.
  ([#307](https://github.com/leynos/mdtablefix/issues/307))
- Keep GFM footnote references coupled to sentence-ending punctuation on inline
  code spans and Markdown links during wrapping, including when the span is
  preceded by opening punctuation.
  ([#299](https://github.com/leynos/mdtablefix/issues/299))
- Keep opening brackets attached to following inline code spans and links during
  wrapping instead of stranding punctuation at line ends. (
  [#293](https://github.com/leynos/mdtablefix/issues/293))
- Keep inflectional affixes (`s`, `'s`, `ed`, `ing`) and hyphenated compounds
  attached to the preceding inline code span during paragraph reflow. A suffix
  that directly follows a closing backtick fence is absorbed into the code
  token, so the two never end up on separate lines after wrapping.
  ([`#300`](https://github.com/leynos/mdtablefix/issues/300))
- Preserve inline GFM footnote references as unbreakable tokens when wrapping
  Markdown paragraphs. ([#277](https://github.com/leynos/mdtablefix/issues/277))
- Preserve link reference definitions verbatim when `--wrap` is used, so
  labels, URLs, and optional titles are never collapsed into prose or split
  across lines. ([`#292`](https://github.com/leynos/mdtablefix/issues/292))
- Normalize whitespace-only artefacts during wrapping by rebalancing atomic
  tails.
- Preserve trailing spaces on the final line when wrapping Markdown, retaining
  hard break semantics. See [trailing spaces](docs/trailing-spaces.md) for
  details. ([#65](https://github.com/leynos/mdtablefix/issues/65))
- Preserve fenced and indented code blocks verbatim when `--wrap` is used, so
  commands inside code examples are not joined or re-wrapped. (
  [#261](https://github.com/leynos/mdtablefix/issues/261))
- Keep trailing punctuation attached to Markdown links and inline code spans
  during wrapping to maintain readability.
- Allow wrapping between space-separated inline code spans instead of treating
  the full sequence as a single unbreakable unit. (
  [#252](https://github.com/leynos/mdtablefix/issues/252))
- Avoid converting numeric references in ATX heading text (including headings in
  blockquotes and list items) when the `--footnotes` option is enabled.
- Compute continuation-line indentation from Unicode display width (via
  `UnicodeWidthStr::width`) rather than byte or character count, so prefixes
  containing full-width characters no longer misalign wrapped output.
- Convert `<table>...</table>` blocks that span multiple lines and carry
  leading indentation, leaving surrounding non-table lines at the same
  indentation level untouched.
- Preserve a document's line-ending style and byte-order mark when rewriting
  files. A CRLF file keeps CRLF instead of being converted to line feeds, and a
  byte-order-marked table is now detected and reflowed rather than leaving its
  header verbatim with the data row before the separator. The bytes written
  change, so a file already processed by an earlier version is rewritten once
  more, and that rewrite cannot be undone by reverting this change.
  ([#451](https://github.com/leynos/mdtablefix/issues/451))
