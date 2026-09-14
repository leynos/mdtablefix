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

# Only Rust code may satisfy a claim; comments and literals cannot provide
# declarations.
function mask_rust_non_code() {
    awk '
    function raw_string_open(line, start,    next_char, hashes) {
        if (substr(line, start, 1) == "r") {
            next_char = start + 1
        } else if (substr(line, start, 2) == "br") {
            next_char = start + 2
        } else {
            return 0
        }

        hashes = ""
        while (substr(line, next_char, 1) == "#") {
            hashes = hashes "#"
            next_char++
        }
        if (substr(line, next_char, 1) != "\"") {
            return 0
        }

        raw_terminator = "\"" hashes
        return next_char + 1
    }

    {
        code = ""
        position = 1
        line_length = length($0)
        while (position <= line_length) {
            if (block_depth > 0) {
                if (substr($0, position, 2) == "/*") {
                    block_depth++
                    position += 2
                } else if (substr($0, position, 2) == "*/") {
                    block_depth--
                    position += 2
                } else {
                    position++
                }
                continue
            }

            if (raw_terminator != "") {
                if (substr($0, position, length(raw_terminator)) == raw_terminator) {
                    terminator_length = length(raw_terminator)
                    raw_terminator = ""
                    position += terminator_length
                } else {
                    position++
                }
                continue
            }

            if (in_string || in_character) {
                if (substr($0, position, 1) == "\\") {
                    position += 2
                } else if ((in_string && substr($0, position, 1) == "\"") || \
                           (in_character && substr($0, position, 1) == "\047")) {
                    in_string = 0
                    in_character = 0
                    position++
                } else {
                    position++
                }
                continue
            }

            if (substr($0, position, 2) == "//") {
                break
            }
            if (substr($0, position, 2) == "/*") {
                block_depth = 1
                position += 2
                continue
            }

            raw_content_start = raw_string_open($0, position)
            if (raw_content_start > 0) {
                position = raw_content_start
                continue
            }

            if (substr($0, position, 2) == "b\"") {
                in_string = 1
                position += 2
                continue
            }
            if (substr($0, position, 1) == "\"") {
                in_string = 1
                position++
                continue
            }
            if (substr($0, position, 1) == "\047") {
                in_character = 1
                position++
                continue
            }

            code = code substr($0, position, 1)
            position++
        }
        print code
    }
    '
}

while IFS= read -r symbol; do
    [[ -z "${symbol}" ]] && continue
    if [[ ! "${symbol}" =~ ^[[:alpha:]_][[:alnum:]_]*$ ]]; then
        echo "verification ledger names missing symbol: ${symbol}"
        exit 1
    fi

    visibility="(pub(\\((crate|self|super|in[[:space:]]+[[:alnum:]_:]+)\\))?[[:space:]]+)?"
    qualifiers="(const[[:space:]]+)?(async[[:space:]]+)?(unsafe[[:space:]]+)?(extern([[:space:]]+\"[^\"]+\")?[[:space:]]+)?"
    declaration="^[[:space:]]*${visibility}${qualifiers}fn[[:space:]]+${symbol}([[:space:]<(])"
    source_files="$("${rg_cmd[@]}" --files --glob '*.rs' "${source_dir}")" || exit $?
    masked_source="$(
        while IFS= read -r source_file; do
            mask_rust_non_code < "${source_file}"
        done <<<"${source_files}"
    )"
    status=0
    printf '%s\n' "${masked_source}" | "${rg_cmd[@]}" --regexp "${declaration}" >/dev/null || status=$?
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
