#!/usr/bin/env bash
# Verify that every claim in the verification ledger names a source symbol.
#
# Usage: check-verification-ledger.sh [SCAN_DIR]
#
# Exit status:
#   0  every named ledger symbol exists below SCAN_DIR/src
#   1  a ledger claim names a missing symbol
#   *  a file or ripgrep failure is propagated
set -euo pipefail

read -r -a rg_cmd <<<"${RG:-rg}"
scan_dir="${1:-.}"
ledger_path="${scan_dir}/docs/verification.md"
source_dir="${scan_dir}/src"

claim_symbols="$(awk -F '|' '
function trim(value) {
    sub(/^[[:space:]]+/, "", value)
    sub(/[[:space:]]+$/, "", value)
    return value
}

/^\|/ {
    symbol = trim($3)
    gsub(/`/, "", symbol)
    if (symbol != "" && symbol != "Executable function" && symbol !~ /^-+$/ && symbol != "Pending") {
        print symbol
    }
}
' "${ledger_path}")"

while IFS= read -r symbol; do
    [[ -z "${symbol}" ]] && continue
    status=0
    "${rg_cmd[@]}" -F --glob '*.rs' -- "${symbol}" "${source_dir}" >/dev/null || status=$?
    case "${status}" in
        0) ;;
        1)
            echo "verification ledger names missing symbol: ${symbol}"
            exit 1
            ;;
        *)
            echo "failed to scan verification symbols (rg exit ${status})" >&2
            exit "${status}"
            ;;
    esac
done <<<"${claim_symbols}"
