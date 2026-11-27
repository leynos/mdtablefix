#!/usr/bin/env bash
# Confirm that Verus rejects dropping the blank-tag half of delimiter read-back.
#
# `spec_delimiter_reads_back` is a disjunction because normalizing a delimiter
# can raise its blank tag: dropping a `null` language specifier turns a
# non-blank tag blank, and the blank tag is what `spec_closes` consults. The
# mutation deletes the `spec_blank_tag_raised` disjunct, leaving plain reading
# agreement. That is the tempting simplification -- production only ever
# removes a specifier, so the two readings "look" equal -- and it is wrong for
# exactly the lines the conflict fallback rewrites.
#
# This is the negative control for the read-back contract's weaker half. Every
# theorem takes `spec_delimiter_reads_back` as a hypothesis, so dropping a
# disjunct cannot break a theorem's body: it narrows what the hypothesis
# admits. What it breaks is the evidence that the hypothesis is satisfiable by
# a real normalization, which is what
# `lemma_witness_null_specifier_reads_back` records. The gate therefore fails
# on that witness, and it is the witness -- not the theorems -- that keeps the
# disjunction honest. Deleting the witness would make this gate fail as a
# mutation that verified, which is the second thing it guards.
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

proof_dir="$(mktemp -d "${repo_root}/verus/.blank-tag-mutation-XXXXXX")"
proof_file="${proof_dir}/lib.rs"
output_file="${proof_dir}/verus.out"

cleanup() {
    rm -rf -- "${proof_dir}"
}
trap cleanup EXIT

mirror_verus_proof_tree "${repo_root}" "${proof_dir}"

# The mutation: the disjunction collapses to its first operand. The predicate
# keeps its name, signature, and doc comment, so the mutated specification
# compiles cleanly; `spec_blank_tag_raised` stays defined because the lemmas
# that reveal it are untouched. The only change is that a raised blank tag no
# longer counts as reading back.
sed 's@ || spec_blank_tag_raised(source, emitted)$@@' \
    "${repo_root}/verus/plan_spec_agreement.rs" \
    > "${proof_dir}/plan_spec_agreement.rs"
if cmp -s "${repo_root}/verus/plan_spec_agreement.rs" \
    "${proof_dir}/plan_spec_agreement.rs"; then
    echo "Blank-tag mutation did not apply" >&2
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
    echo "Blank-tag mutation unexpectedly verified" >&2
    exit 1
fi

# The gate must fail for the intended reason. The run has to reach the
# verifier, every error it reports has to be a failed postcondition, and every
# one of those has to be located in the witness file -- a mutation that broke
# some unrelated obligation would otherwise look like a detection. The two
# attributions then pin *which* witness and *which* of its stated conclusions
# the mutation falsified.
if ! assert_failures_attributed "${output_file}" plan_spec_witnesses.rs || \
    ! grep -Fq "verification results::" "${output_file}" || \
    ! grep -Fq "lemma_witness_null_specifier_reads_back" "${output_file}" || \
    ! grep -Fq "spec_delimiter_reads_back(spec_witness_null_opener(), spec_witness_null_emitted())" \
        "${output_file}"; then
    show_mutation_output "${output_file}"
    echo "Blank-tag mutation did not falsify the read-back witness" >&2
    exit 1
fi

show_mutation_output "${output_file}"
echo "Blank-tag mutation confirmed the expected read-back rejection"