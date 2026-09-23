#!/usr/bin/env bash
# Confirm the ATX postcondition rejects an emitted heading without its space.
set -euo pipefail

repo_root="${1:-.}"
proof_dir="$(mktemp -d "${repo_root}/verus/.atx-mutation-XXXXXX")"
proof_file="${proof_dir}/lib.rs"
output_file="${proof_dir}/verus.out"

cleanup() {
    rm -f "${proof_file}" "${output_file}"
    rmdir "${proof_dir}"
}
trap cleanup EXIT

sed -e "s/\.push('#')\.push(' ')/.push('#')/" \
    -e 's@../src/classify_kernel.rs@../../src/classify_kernel.rs@' \
    "${repo_root}/verus/lib.rs" > "${proof_file}"

# `PROVER_TOOLS` deliberately carries a command and its fixed arguments, as it
# does in the Makefile. Its expansion must therefore remain unquoted here.
# shellcheck disable=SC2086
if env -u RUSTUP_TOOLCHAIN ${PROVER_TOOLS:?PROVER_TOOLS must be set} verus run \
    --repo-root "${repo_root}" --proof-file "${proof_file}" > "${output_file}" 2>&1; then
    cat "${output_file}"
    echo "ATX mutation unexpectedly verified" >&2
    exit 1
fi

grep -Fq "verification results::" "${output_file}"
grep -Fq "assertion failed" "${output_file}"
