# Changelog

## [Unreleased]

### Added

- `--code-emphasis` flag to fix emphasis markers that adjoin inline code.
  Runs before wrapping and footnote conversion.

### Fixed

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
  attached to the preceding inline code span during paragraph reflow.
  A suffix that directly follows a closing backtick fence is absorbed into the
  code token, so the two never end up on separate lines after wrapping.
  ([`#300`](https://github.com/leynos/mdtablefix/issues/300))
- Preserve inline GFM footnote references as unbreakable tokens when wrapping
  Markdown paragraphs. ([#277](https://github.com/leynos/mdtablefix/issues/277))
- Preserve link reference definitions verbatim when `--wrap` is used, so
  labels, URLs, and optional titles are never collapsed into prose or split
  across lines.
  ([`#292`](https://github.com/leynos/mdtablefix/issues/292))
- Normalize whitespace-only artefacts during wrapping by rebalancing atomic
  tails.
- Preserve trailing spaces on the final line when wrapping Markdown, retaining
  hard break semantics. See [trailing spaces](docs/trailing-spaces.md) for
  details. ([#65](https://github.com/leynos/mdtablefix/issues/65))
- Preserve fenced and indented code blocks verbatim when `--wrap` is used, so
  commands inside code examples are not joined or re-wrapped. (
  [#261](https://github.com/leynos/mdtablefix/issues/261))
- Keep trailing punctuation attached to inline code spans during wrapping to
  maintain readability.
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
- Correct the `cargo-binstall` Linux GNU `bin-dir` override so binaries are
  installed from the archive's current directory (`.`) rather than a derived
  `{ bin }{ binary-ext }` path, restoring `cargo binstall` on Linux.
