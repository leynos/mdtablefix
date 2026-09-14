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

if env -u RUSTUP_TOOLCHAIN \
    uvx --from "git+https://github.com/leynos/rust-prover-tools@$(cat "${repo_root}/tools/rust-prover-tools/REF")" \
    prover-tools verus run --repo-root "${repo_root}" --proof-file "${proof_file}" > "${output_file}" 2>&1; then
    cat "${output_file}"
    echo "ATX mutation unexpectedly verified" >&2
    exit 1
fi

grep -Fq "Verus proofs failed" "${output_file}"
