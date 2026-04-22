# Changelog

## [Unreleased]

### Added

- `--code-emphasis` flag to fix emphasis markers that adjoin inline code.
  Runs before wrapping and footnote conversion.

### Fixed

- Preserve trailing spaces on the final line when wrapping Markdown, retaining
  hard break semantics. See [trailing spaces](docs/trailing-spaces.md) for
  details. ([#65](https://github.com/leynos/mdtablefix/issues/65))
- Preserve fenced and indented code blocks verbatim when `--wrap` is used, so
  commands inside code examples are not joined or re-wrapped.
  ([#261](https://github.com/leynos/mdtablefix/issues/261))
- Keep trailing punctuation attached to inline code spans during wrapping to
  maintain readability.
- Avoid converting numeric references in ATX heading text (including headings in
  blockquotes and list items) when the `--footnotes` option is enabled.
