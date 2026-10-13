#!/usr/bin/env bash
# Confirm that Verus rejects ignoring the conflict flag when a block is emitted.
#
# The planner decides one compression answer per block, and the whole reason the
# conflict flag exists is that this answer is unsafe for a block holding an
# interior line the rewritten opener would newly close. The mutation makes
# `delimiter_agendum` compress every delimiter it is handed, discarding the
# `compressible` argument that carries the block's answer. The mutated body is
# plausible and still compiles: it writes a three-backtick delimiter exactly
# where the correct body writes one, and differs only for the blocks the flag
# was protecting.
#
# This is the negative control for the document theorem's conflict half. With
# the flag ignored, a rewritten opener can be closed by an interior line that
# did not close the source opener, so every later line leaves the literal
# region. LEM-NO-CONFLICT-ALL-CLEAR and the compressed-block induction both
# depend on that not happening, so the proof must fail. A gate that still
# verified would mean those lemmas were proved from something other than the
# property they claim.
set -euo pipefail

# Absolute, because `VERUS_RUN` names its repository root as `.`. The runner
# reads that relative to the working directory, so the script moves to the
# repository root and keeps every path it builds independent of the caller's
# directory.
repo_root="$(cd -- "${1:-.}" && pwd)"
cd "${repo_root}"

proof_dir="$(mktemp -d "${repo_root}/verus/.compression-mutation-XXXXXX")"
proof_file="${proof_dir}/lib.rs"
output_file="${proof_dir}/verus.out"

cleanup() {
    rm -rf -- "${proof_dir}"
}
trap cleanup EXIT

mkdir -p "${proof_dir}/src/wrap/fence"

# Every production kernel the proof entry point includes has to be reproduced at
# the path its `#[path]` attribute names: the classifier and the planner at
# `src/`, the fence kernel at `src/wrap/fence/`. Each `include!` and `#[path]`
# inside those modules is relative to its own directory, so these copies resolve
# without further rewriting.
sed -e 's@../src/classify_kernel.rs@src/classify_kernel.rs@' \
    -e 's@../src/wrap/fence/kernel.rs@src/wrap/fence/kernel.rs@' \
    -e 's@../src/fences/plan.rs@src/fences/plan.rs@' \
    "${repo_root}/verus/lib.rs" > "${proof_file}"
mkdir -p "${proof_dir}/src/fences"
cp "${repo_root}/verus/classify_spec.rs" "${proof_dir}/classify_spec.rs"
cp "${repo_root}/verus/fence_spec.rs" "${proof_dir}/fence_spec.rs"
cp "${repo_root}/verus/plan_spec.rs" "${proof_dir}/plan_spec.rs"
cp "${repo_root}/src/classify_kernel.rs" "${proof_dir}/src/classify_kernel.rs"
cp "${repo_root}/src/verified_kernel_macros.rs" "${proof_dir}/src/verified_kernel_macros.rs"
cp "${repo_root}/src/classify_kernel_predicates.rs" "${proof_dir}/src/classify_kernel_predicates.rs"
cp "${repo_root}/src/classify_kernel_consumers.rs" "${proof_dir}/src/classify_kernel_consumers.rs"
cp "${repo_root}/src/wrap/fence/kernel.rs" "${proof_dir}/src/wrap/fence/kernel.rs"

# The mutation: the `compressible` argument stops constraining the answer, so
# every delimiter is written compressed. The binding stays used and the shape of
# the branch stays intact, so the mutated kernel compiles cleanly and the only
# change is that the conflict flag no longer reaches the emission decision.
sed -e 's@^    } else if compressible {@    } else if true {@' \
    -e 's@^        Agendum::Normalized@        Agendum::Compressed@' \
    "${repo_root}/src/fences/plan.rs" \
    > "${proof_dir}/src/fences/plan.rs"
if cmp -s "${repo_root}/src/fences/plan.rs" \
    "${proof_dir}/src/fences/plan.rs"; then
    echo "Compression-flag mutation did not apply" >&2
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
    echo "Compression-flag mutation unexpectedly verified" >&2
    exit 1
fi

# The gate must fail for the intended reason. A verifier that failed somewhere
# else -- a parse error, or an unrelated obligation -- would satisfy a bare "did
# not verify" check while proving nothing about the conflict flag, so the output
# must name the failed obligation and the specification function the mutated
# body no longer realises.
if ! grep -Fq "verification results::" "${output_file}" || \
    ! grep -Fq "postcondition not satisfied" "${output_file}" || \
    ! grep -Fq "result == crate::spec_delimiter_agendum(is_delimiter, compressible)" \
        "${output_file}"; then
    cat "${output_file}"
    echo "Compression mutation did not falsify the delimiter agendum's contract" >&2
    exit 1
fi
