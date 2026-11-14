#!/usr/bin/env bash
# Confirm that Verus rejects weakening the normalized-emission clause.
#
# `spec_emission_reads_back`'s `Normalized` arm is the contract the conflict
# fallback relies on: a delimiter the planner chose not to compress is written
# in its *source* marker family and run, with a null language specifier
# dropped and nothing else changed. The mutation replaces the arm's body with
# `true`, which is the plausible mistake rather than a strawman: the arm still
# type-checks, still sits in the same match, and still mentions every variable
# its siblings mention. What it loses is that the written bytes are read back
# at all, so the emitter could write anything for a block the planner refused
# to compress and the proof would not notice.
#
# This is the negative control for the emission contract's normalization half.
# `lemma_normalized_emission_reads_back` consumes the arm and must hand the
# block lemmas `spec_delimiter_reads_back`; with the arm weakened its
# postcondition cannot close. The witness that pins what the contract
# *rejects* fails with it, because a contract that is `true` rejects nothing.
# A gate that still verified would mean the document theorem never read the
# clause it claims to consume, and that the witness was decoration.
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

proof_dir="$(mktemp -d "${repo_root}/verus/.emission-mutation-XXXXXX")"
proof_file="${proof_dir}/lib.rs"
output_file="${proof_dir}/verus.out"

cleanup() {
    rm -rf -- "${proof_dir}"
}
trap cleanup EXIT

mirror_verus_proof_tree "${repo_root}" "${proof_dir}"

# The mutation: the `Normalized` arm's implication becomes `true`. The arm keeps
# its shape and the surrounding match keeps its three branches, so the mutated
# specification compiles cleanly and the only change is that a normalized
# emission no longer has to read back as the line it replaced.
sed 's@line.normalizable ==> spec_delimiter_reads_back(line.source, line.emitted)@true@' \
    "${repo_root}/verus/plan_spec_agreement.rs" \
    > "${proof_dir}/plan_spec_agreement.rs"
if cmp -s "${repo_root}/verus/plan_spec_agreement.rs" \
    "${proof_dir}/plan_spec_agreement.rs"; then
    echo "Normalized-emission mutation did not apply" >&2
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
    echo "Normalized-emission mutation unexpectedly verified" >&2
    exit 1
fi

# The gate must fail for the intended reason. The run has to reach the
# verifier, every error it reports has to be a failed postcondition, and every
# one of those has to be located in the consumer lemma or the witness -- a
# mutation that broke some unrelated obligation would otherwise look like a
# detection. Only then do the two attributions mean what they claim.
if ! assert_failures_attributed "${output_file}" \
        plan_spec_emission.rs plan_spec_witnesses.rs || \
    ! grep -Fq "verification results::" "${output_file}" || \
    ! grep -Fq "ensures spec_delimiter_reads_back(source, emitted)" "${output_file}" || \
    ! grep -Fq "lemma_witness_normalized_contract_refuses_a_wrong_respelling" "${output_file}"; then
    show_mutation_output "${output_file}"
    echo "Normalized-emission mutation did not falsify the clause's contract" >&2
    exit 1
fi

show_mutation_output "${output_file}"
echo "Normalized-emission mutation confirmed the expected read-back rejection"