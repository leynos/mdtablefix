#!/usr/bin/env bash
# Shared mirror builder for the Verus mutation gates.
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
# Re-use policy: source this file from a mutation gate and call
# `mirror_verus_proof_tree` once, after creating an empty proof directory and
# before writing the mutated copy over it. It is not a general-purpose helper
# and has no callers outside `scripts/check-*-mutation.sh`.
#
# Usage: `mirror_verus_proof_tree REPO_ROOT PROOF_DIR`

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
