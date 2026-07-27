#!/usr/bin/env bash
# Reject hand-rolled static regular expressions that bypass the `lazy_regex!`
# convention.
#
# The guard scans Rust sources for `static` declarations that wrap `Regex::new`
# directly in a supported lazy-wrapper constructor. Two wrapper families are
# supported, each matched whether spelled directly or fully qualified:
#
#   * `std::sync::LazyLock::new`
#   * `once_cell::sync::Lazy::new`
#
# Usage: check-static-regexes.sh [SCAN_DIR]
#
# SCAN_DIR defaults to the current directory. The RG environment variable
# overrides the ripgrep command (default: `rg`). It is split on whitespace, so
# it may carry arguments — for example `RG='rg --pcre2'` — matching the way the
# Makefile's `$(RG)` expansion behaved before the scan was extracted here.
#
# Exit status:
#   0  no prohibited declaration found
#   1  a prohibited declaration was found (diagnostic on stdout)
#   *  ripgrep failed to scan (diagnostic on stderr; rg's status propagated)
set -euo pipefail

read -r -a rg_cmd <<<"${RG:-rg}"
scan_dir="${1:-.}"

# `(?:[[:alnum:]_]+::)*` absorbs any module qualification (for example the
# `once_cell::sync::` in `once_cell::sync::Lazy::new`), so both the direct and
# fully qualified spellings of each supported constructor are rejected.
# `(?:move\s+)?` covers `move` closures such as `LazyLock::new(move || ...)`.
pattern='\bstatic\b[^;=]*=\s*(?:[[:alnum:]_]+::)*(?:LazyLock|Lazy)::new\s*\(\s*(?:move\s+)?\|\|\s*(\{\s*)?(?:[[:alnum:]_]+::)*Regex::new'

status=0
"${rg_cmd[@]}" -U --glob '*.rs' "$pattern" "$scan_dir" || status=$?
case $status in
	0) echo "static regular expressions must use lazy_regex!"; exit 1 ;;
	1) exit 0 ;;
	*) echo "failed to scan Rust sources (rg exit $status)" >&2; exit "$status" ;;
esac
