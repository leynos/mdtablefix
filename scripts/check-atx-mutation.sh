#!/usr/bin/env bash
# Confirm the ATX postcondition rejects an emitted heading without its space.
set -euo pipefail

repo_root="${1:-.}"
proof_file="$(mktemp /tmp/mdtablefix-atx-mutation-XXXXXX.rs)"
output_file="$(mktemp /tmp/mdtablefix-atx-mutation-XXXXXX.out)"

cleanup() {
    unlink "${proof_file}"
    unlink "${output_file}"
}
trap cleanup EXIT

sed "s/\.push('#')\.push(' ')/.push('#')/" "${repo_root}/verus/lib.rs" > "${proof_file}"

# `PROVER_TOOLS` deliberately carries a command and its fixed arguments, as it
# does in the Makefile. Its expansion must therefore remain unquoted here.
# shellcheck disable=SC2086
if env -u RUSTUP_TOOLCHAIN ${PROVER_TOOLS:?PROVER_TOOLS must be set} verus run \
    --repo-root "${repo_root}" --proof-file "${proof_file}" > "${output_file}" 2>&1; then
    cat "${output_file}"
    echo "ATX mutation unexpectedly verified" >&2
    exit 1
fi

grep -Fq "Verus proofs failed" "${output_file}"
