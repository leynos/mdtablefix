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

proof_dir="$(mktemp -d "${repo_root}/verus/.compression-mutation-XXXXXX")"
proof_file="${proof_dir}/lib.rs"
output_file="${proof_dir}/verus.out"

cleanup() {
    rm -rf -- "${proof_dir}"
}
trap cleanup EXIT

mirror_verus_proof_tree "${repo_root}" "${proof_dir}"

# The mutation: the `compressible` guard becomes `true`, so every delimiter is
# written compressed. The argument stays in the signature and the shape of the
# branch stays intact, so the mutated kernel compiles cleanly and the only
# change is that the conflict flag no longer reaches the emission decision. That
# the detection is targeted rather than global is part of the gate's value: the
# mutated run reports exactly one failed obligation, the body that no longer
# realises its contract, and every other proof still verifies.
sed 's@    } else if compressible {@    } else if true {@' \
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
    show_mutation_output "${output_file}"
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
    show_mutation_output "${output_file}"
    echo "Compression mutation did not falsify the delimiter agendum's contract" >&2
    exit 1
fi

show_mutation_output "${output_file}"
echo "Compression mutation confirmed the expected delimiter-agendum rejection"
