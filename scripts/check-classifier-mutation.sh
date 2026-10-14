#!/usr/bin/env bash
# Confirm that Verus rejects a production ATX marker without its separator.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# The mirror builder is the one place that knows how a proof tree is laid out,
# so a change to `verus/lib.rs`'s module list cannot leave this gate copying a
# stale subset of it.
# shellcheck source=scripts/verus-mutation-mirror.sh
. "${script_dir}/verus-mutation-mirror.sh"

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

# The mirror carries every production kernel unmodified except the classifier
# consumer written below, because the proofs in `fence_spec.rs` and
# `plan_spec.rs` are part of what this gate has to re-verify.
mirror_verus_proof_tree "${repo_root}" "${proof_dir}"

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
    show_mutation_output "${output_file}"
    echo "ATX separator mutation unexpectedly verified" >&2
    exit 1
fi

if ! grep -Fq "verification results::" "${output_file}" || \
    ! awk '
        /^error:/ { active = /postcondition not satisfied/ }
        active && /spec_classify\(result@,.*LineClass::AtxHeading/ { found = 1 }
        END { exit !found }
    ' "${output_file}"; then
    show_mutation_output "${output_file}"
    echo "ATX separator mutation did not fail the executable output-classification postcondition" >&2
    exit 1
fi

show_mutation_output "${output_file}"
echo "ATX separator mutation confirmed the expected executable output-classification rejection"
