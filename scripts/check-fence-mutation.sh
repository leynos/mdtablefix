#!/usr/bin/env bash
# Confirm that Verus rejects a wrong production fence closing rule.
#
# The mutation drops the marker-character comparison from `closes_fence`, the
# kernel's single closing rule. The mutated predicate still tests depth, run
# length, and trailing whitespace, so it remains plausible: it accepts every
# line a correct run accepts, plus lines of an unrelated marker family. What it
# loses is the property that a line's region is a function of the *opener it
# faces*, and that is precisely what the specification pins.
#
# This is the negative control for LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS.
# The lemma's justification rests on a marker of an unrelated family closing
# nothing; with the check gone a tilde line appears to close a backtick opener,
# so the proof must fail. A mutation gate that still verified would mean the
# lemma was proved from something other than the property it claims.
set -euo pipefail

# Absolute, because `VERUS_RUN` names its repository root as `.`. The runner
# reads that relative to the working directory, so the script moves to the
# repository root and keeps every path it builds independent of the caller's
# directory.
repo_root="$(cd -- "${1:-.}" && pwd)"
cd "${repo_root}"

proof_dir="$(mktemp -d "${repo_root}/verus/.fence-mutation-XXXXXX")"
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
# rewriting.
sed -e 's@../src/classify_kernel.rs@src/classify_kernel.rs@' \
    -e 's@../src/wrap/fence/kernel.rs@src/wrap/fence/kernel.rs@' \
    "${repo_root}/verus/lib.rs" > "${proof_file}"
cp "${repo_root}/verus/classify_spec.rs" "${proof_dir}/classify_spec.rs"
cp "${repo_root}/verus/fence_spec.rs" "${proof_dir}/fence_spec.rs"
cp "${repo_root}/src/classify_kernel.rs" "${proof_dir}/src/classify_kernel.rs"
cp "${repo_root}/src/verified_kernel_macros.rs" "${proof_dir}/src/verified_kernel_macros.rs"
cp "${repo_root}/src/classify_kernel_predicates.rs" "${proof_dir}/src/classify_kernel_predicates.rs"
cp "${repo_root}/src/classify_kernel_consumers.rs" "${proof_dir}/src/classify_kernel_consumers.rs"

# The mutation: `marker == state.marker` becomes `marker == marker`. The
# comparison stays syntactically present and the binding stays used, so the
# mutated kernel compiles cleanly and the only change is that the marker
# character no longer constrains the answer.
sed 's@&& marker == state.marker@\&\& marker == marker@' \
    "${repo_root}/src/wrap/fence/kernel.rs" \
    > "${proof_dir}/src/wrap/fence/kernel.rs"
if cmp -s "${repo_root}/src/wrap/fence/kernel.rs" \
    "${proof_dir}/src/wrap/fence/kernel.rs"; then
    echo "Fence closing-rule mutation did not apply" >&2
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
    echo "Fence closing-rule mutation unexpectedly verified" >&2
    exit 1
fi

# The gate must fail for the intended reason. A verifier that failed somewhere
# else -- a parse error, or an unrelated obligation -- would satisfy a bare
# "did not verify" check while proving nothing about the closing rule, so the
# output must name both the failed postcondition and the specification function
# the mutated body no longer realises.
if ! grep -Fq "verification results::" "${output_file}" || \
    ! grep -Fq "postcondition not satisfied" "${output_file}" || \
    ! grep -Fq "result == crate::spec_closes(state, line)" "${output_file}"; then
    cat "${output_file}"
    echo "Fence mutation did not falsify the closing rule's contract" >&2
    exit 1
fi
