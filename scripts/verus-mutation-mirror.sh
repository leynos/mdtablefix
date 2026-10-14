#!/usr/bin/env bash
# Shared supporting helpers for the Verus mutation gates.
#
# Every mutation gate has to hand Verus a self-contained proof tree, because it
# supplies a mutated copy of one production module and must not touch the
# working tree. `verus/lib.rs` names its production modules through `#[path]`
# attributes, so the mirror has to reproduce each of those paths and everything
# they include in turn.
#
# The mirror copies the whole `src/` tree rather than a list of kernels. A
# hand-maintained list is what broke the classifier and fence gates when the
# planner was added to `verus/lib.rs`: the proof entry point gained a module
# whose source no gate was copying, so the run died with "couldn't read
# .../src/fences/plan.rs" and the gate rejected an infrastructure failure --
# correctly -- instead of reporting a proof result. Mirroring the tree removes
# that failure mode. Every `#[path]` inside a production module is relative to
# its own directory and points within `src/`, so the copies resolve unchanged;
# only the proof entry point's own `../src/` prefix needs rewriting.
#
# The second helper covers the gates' other shared need: displaying a
# deliberately failing run's output without GitHub reading instructions out of
# it. Both concerns belong to the gates alone.
#
# Re-use policy: source this file from a mutation gate, call
# `mirror_verus_proof_tree` once after creating an empty proof directory and
# before writing the mutated copy over it, and route every display of verifier
# output through `show_mutation_output`. It is not a general-purpose helper and
# has no callers outside `scripts/check-*-mutation.sh`.
#
# Usage: `mirror_verus_proof_tree REPO_ROOT PROOF_DIR`
#        `show_mutation_output OUTPUT_FILE`

mirror_verus_proof_tree() {
    local repo_root="$1"
    local proof_dir="$2"

    # The whole tree, so a new module in `verus/lib.rs` needs no gate change.
    cp -r "${repo_root}/src" "${proof_dir}/src"

    # The proof entry point moves to the mirror's root, so its production
    # modules are one level down rather than two. Its specification modules
    # (`#[path = "classify_spec.rs"]` and friends) are already root-relative and
    # stay correct because the copies land beside it.
    sed 's@"\.\./src/@"src/@' "${repo_root}/verus/lib.rs" \
        > "${proof_dir}/lib.rs"

    # The specifications themselves, minus two files that must not travel: the
    # entry point just written, and `smoke.rs`, whose deliberately false
    # assertion exists only to prove the harness is live and would turn every
    # mirror into a failing run.
    local spec base
    for spec in "${repo_root}"/verus/*.rs; do
        base="$(basename "${spec}")"
        case "${base}" in lib.rs | smoke.rs) continue ;; esac
        cp "${spec}" "${proof_dir}/${base}"
    done
}

# Print a failing verifier's output without letting GitHub act on it.
#
# Every gate runs Verus against a mutated kernel, so every gate has output that
# is *supposed* to contain errors. The runner renders each Verus diagnostic as a
# GitHub workflow command (`::error file=...,line=...::postcondition not
# satisfied`) so that a failing proof annotates the offending source line. Echoing
# that transcript raw makes the runner annotate the mirrored paths under the
# gate's `mktemp` scratch directory, which the Checks UI then presents as
# failures of the unmutated production contracts -- `src/fences/plan.rs` and
# `src/wrap/fence/kernel.rs` -- that this repository proves sound. The gate is
# green and its own evidence says the opposite of what the annotation implies.
#
# `::stop-commands::` suspends workflow-command parsing until a matching
# `::<token>::` line, so the transcript prints verbatim while the annotation is
# suppressed. The token must be unguessable, because any line of the output that
# happened to equal it would end the suppression early; a fresh one per call
# costs nothing and cannot be predicted by the output being printed.
show_mutation_output() {
    local output_file="$1"
    local token
    token="$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')"
    echo "::stop-commands::${token}"
    cat "${output_file}"
    echo "::${token}::"
}
