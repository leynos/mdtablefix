#!/usr/bin/env bash
# Confirm that Verus rejects a production ATX marker without its separator.
set -euo pipefail

# Absolute, because `VERUS_RUN` names its repository root as `.`. The runner
# reads that relative to the working directory, so the script moves to the
# repository root and keeps every path it builds independent of the caller's
# directory.
repo_root="$(cd -- "${1:-.}" && pwd)"
cd "${repo_root}"

proof_dir="$(mktemp -d "${repo_root}/verus/.atx-mutation-XXXXXX")"
proof_file="${proof_dir}/lib.rs"
output_file="${proof_dir}/verus.out"

cleanup() {
    rm -rf -- "${proof_dir}"
}
trap cleanup EXIT

mkdir -p "${proof_dir}/src/wrap/fence"

# Both production kernels are included by the proof entry point, so both tree
# layouts have to be reproduced: the classifier at `src/`, the fence kernel at
# `src/wrap/fence/`. Each `include!` and `#[path]` inside those modules is
# relative to its own directory, so these copies resolve without further
# rewriting. Only the classifier is mutated here; the fence kernel and its
# specification travel unmodified, because the proofs in `fence_spec.rs` are
# part of what this gate has to re-verify.
sed -e 's@../src/classify_kernel.rs@src/classify_kernel.rs@' \
    -e 's@../src/wrap/fence/kernel.rs@src/wrap/fence/kernel.rs@' \
    "${repo_root}/verus/lib.rs" > "${proof_file}"
cp "${repo_root}/verus/classify_spec.rs" "${proof_dir}/classify_spec.rs"
cp "${repo_root}/verus/setext_spec.rs" "${proof_dir}/setext_spec.rs"
cp "${repo_root}/verus/fence_spec.rs" "${proof_dir}/fence_spec.rs"
cp "${repo_root}/src/classify_kernel.rs" "${proof_dir}/src/classify_kernel.rs"
cp "${repo_root}/src/verified_kernel_macros.rs" "${proof_dir}/src/verified_kernel_macros.rs"
cp "${repo_root}/src/classify_kernel_predicates.rs" "${proof_dir}/src/classify_kernel_predicates.rs"
# The fence kernel travels unmodified; `fence_spec.rs` re-verifies it here.
cp "${repo_root}/src/wrap/fence/kernel.rs" "${proof_dir}/src/wrap/fence/kernel.rs"
cp "${repo_root}/src/classify_kernel_consumers.rs" "${proof_dir}/src/classify_kernel_consumers.rs"
sed "/^[[:space:]]*result\.push(' '); \/\/ Mandatory ATX separator\.[[:space:]]*$/d" \
    "${repo_root}/src/classify_kernel_setext.rs" \
    > "${proof_dir}/src/classify_kernel_setext.rs"
if cmp -s "${repo_root}/src/classify_kernel_setext.rs" \
    "${proof_dir}/src/classify_kernel_setext.rs"; then
    echo "ATX separator mutation did not apply" >&2
    exit 1
fi

# `VERUS_RUN` carries the runner, its toolchain isolation, and its fixed
# arguments from the Makefile, so overriding that variable reaches this gate
# exactly as it reaches `verus` and `verus-selftest`. Only the proof file is
# added here, because this gate supplies its own mutated copy.
# shellcheck disable=SC2086
if ${VERUS_RUN:?VERUS_RUN must be set} \
    --proof-file "${proof_file}" > "${output_file}" 2>&1; then
    cat "${output_file}"
    echo "ATX separator mutation unexpectedly verified" >&2
    exit 1
fi

if ! grep -Fq "verification results::" "${output_file}" || \
    ! awk '
        /^error:/ { active = /postcondition not satisfied/ }
        active && /spec_classify\(result@,.*LineClass::AtxHeading/ { found = 1 }
        END { exit !found }
    ' "${output_file}"; then
    cat "${output_file}"
    echo "ATX separator mutation did not fail the executable output-classification postcondition" >&2
    exit 1
fi

cat "${output_file}"
echo "ATX separator mutation confirmed the expected executable output-classification rejection"
