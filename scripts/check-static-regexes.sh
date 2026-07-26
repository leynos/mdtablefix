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
# overrides the ripgrep executable (default: `rg`).
#
# Exit status:
#   0  no prohibited declaration found
#   1  a prohibited declaration was found (diagnostic on stdout)
#   *  ripgrep failed to scan (diagnostic on stderr; rg's status propagated)
set -euo pipefail

RG="${RG:-rg}"
scan_dir="${1:-.}"

# `(?:[[:alnum:]_]+::)*` absorbs any module qualification (for example the
# `once_cell::sync::` in `once_cell::sync::Lazy::new`), so both the direct and
# fully qualified spellings of each supported constructor are rejected.
pattern='\bstatic\b[^;=]*=\s*(?:[[:alnum:]_]+::)*(?:LazyLock|Lazy)::new\s*\(\s*\|\|\s*(\{\s*)?(?:[[:alnum:]_]+::)*Regex::new'

status=0
"$RG" -U --glob '*.rs' "$pattern" "$scan_dir" || status=$?
case $status in
	0) echo "static regular expressions must use lazy_regex!"; exit 1 ;;
	1) exit 0 ;;
	*) echo "failed to scan Rust sources (rg exit $status)" >&2; exit "$status" ;;
esac
