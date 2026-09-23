#!/usr/bin/env bash
# Confirm that Verus rejects a wrong production Setext consumer decision.
set -euo pipefail

repo_root="${1:-.}"
proof_dir="$(mktemp -d "${repo_root}/verus/.setext-mutation-XXXXXX")"
proof_file="${proof_dir}/lib.rs"
output_file="${proof_dir}/verus.out"

cleanup() {
    rm -rf -- "${proof_dir}"
}
trap cleanup EXIT

mkdir "${proof_dir}/src"
sed 's@../src/classify_kernel.rs@src/classify_kernel.rs@' \
    "${repo_root}/verus/lib.rs" > "${proof_file}"
cp "${repo_root}/verus/classify_spec.rs" "${proof_dir}/classify_spec.rs"
cp "${repo_root}/src/classify_kernel.rs" "${proof_dir}/src/classify_kernel.rs"
cp "${repo_root}/src/classify_kernel_macros.rs" "${proof_dir}/src/classify_kernel_macros.rs"
cp "${repo_root}/src/classify_kernel_predicates.rs" "${proof_dir}/src/classify_kernel_predicates.rs"
sed 's@LineClass::ParagraphText)$@LineClass::AtxHeading)@' \
    "${repo_root}/src/classify_kernel_consumers.rs" \
    > "${proof_dir}/src/classify_kernel_consumers.rs"
if cmp -s "${repo_root}/src/classify_kernel_consumers.rs" \
    "${proof_dir}/src/classify_kernel_consumers.rs"; then
    echo "Setext consumer mutation did not apply" >&2
    exit 1
fi

# `PROVER_TOOLS` carries a command and fixed arguments from the Makefile.
# shellcheck disable=SC2086
if env -u RUSTUP_TOOLCHAIN ${PROVER_TOOLS:?PROVER_TOOLS must be set} verus run \
    --repo-root "${repo_root}" --proof-file "${proof_file}" > "${output_file}" 2>&1; then
    cat "${output_file}"
    echo "Setext consumer mutation unexpectedly verified" >&2
    exit 1
fi

if ! grep -Fq "verification results::" "${output_file}" || \
    ! grep -Fq "postcondition not satisfied" "${output_file}"; then
    cat "${output_file}"
    echo "Setext mutation did not reach a failed proof assertion" >&2
    exit 1
fi
