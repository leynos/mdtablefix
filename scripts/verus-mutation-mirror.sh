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
# it. The third covers the question the greps cannot answer on their own:
# whether the run failed where the gate meant it to. All three concerns belong
# to the gates alone.
#
# Re-use policy: source this file from a mutation gate, call
# `mirror_verus_proof_tree` once after creating an empty proof directory and
# before writing the mutated copy over it, route every display of verifier
# output through `show_mutation_output`, and confirm the failure's shape with
# `assert_failures_attributed` before grepping for its message. It is not a
# general-purpose helper and has no callers outside
# `scripts/check-*-mutation.sh`.
#
# Usage: `mirror_verus_proof_tree REPO_ROOT PROOF_DIR`
#        `show_mutation_output OUTPUT_FILE`
#        `assert_failures_attributed OUTPUT_FILE EXPECTED_FILE...`

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

# Confirm a failing verifier run failed where the gate expected it to.
#
# Grepping for a gate's own contract answers "did the run report this?" but not
# "did it report this because of the mutation?". A run that also, or instead,
# hit an unrelated obligation, a parse error, a precondition, or a runner
# failure satisfies a bare grep: the run failed, the expected string appeared,
# and the gate went green while the injected mutation had nothing to do with
# the failure. A negative control has to fail for its stated reason.
#
# So every error in the transcript must be a failed postcondition, and every
# such error must be located in one of the files the caller names. An error's
# location is the first `--> file:line:column` after its `error:` header;
# comparing basenames keeps the caller independent of the gate's `mktemp`
# directory, whose name changes on every run.
#
# Each error must have exactly one such location, and the located errors must
# account for the whole count. A parser that only prints the locations it finds
# would let an extra, location-free postcondition failure through: it would be
# counted as explained, contribute no location, and leave the module check
# looking complete.
#
# Usage: `assert_failures_attributed OUTPUT_FILE EXPECTED_FILE...`
assert_failures_attributed() {
    local output_file="$1"
    shift
    local total
    local explained
    # `error: aborting due to N previous errors` is the compiler's summary of
    # the Verus errors above it, not one of them, so it must not be counted as
    # an unexplained failure. An `error[E0425]:` header counts, though: a coded
    # error is a compile failure, not a verification one, and counting only the
    # bare form let a transcript pass that carried both the intended
    # postcondition failure and an unrelated compiler error beside it.
    read -r total explained < <(awk '
        /^error(\[[^]]*\])?:/ && !/^error: aborting due to / {
            total++
            if (index($0, "error: postcondition not satisfied") == 1) {
                explained++
            }
        }
        END { print total + 0, explained + 0 }
    ' "${output_file}")
    if [ "${total}" -eq 0 ]; then
        echo "Verifier reported no error at all" >&2
        return 1
    fi
    if [ "${total}" -ne "${explained}" ]; then
        echo "Verifier failed with an error that was not a failed postcondition" >&2
        return 1
    fi

    local expected=" $* "
    local location
    local base
    local located=0
    # One line per counted error: its location when the error block named
    # exactly one, `!` when it named none, `**` when it named several. An
    # error that named an expected module first and an unrelated one second
    # would otherwise have its second location ignored, so "exactly one" is
    # the rule rather than "at least one". This pass counts the same headers
    # the first one does, coded errors included, so the two counts can be
    # compared.
    while IFS= read -r location; do
        located=$((located + 1))
        case "${location}" in
            "!")
                echo "Verifier reported an error with no source location" >&2
                return 1
                ;;
            "**")
                echo "Verifier reported an error with more than one source location" >&2
                return 1
                ;;
        esac
        base="${location##*/}"
        case "${expected}" in
            *" ${base} "*) ;;
            *)
                echo "Verifier failed outside the expected modules: ${base}" >&2
                return 1
                ;;
        esac
    done < <(awk '
        # One verdict per error block, emitted at the next diagnostic header
        # or at the end of the transcript. `note:` and `warning:` end a block
        # as surely as another error does: their own `-->` lines point at
        # source too, and counting one as a second location would reject every
        # transcript that reports a trigger suggestion after a failure.
        function emit() {
            if (!open) return
            if (count == 0) print "!"
            else if (count > 1) print "**"
            else print first
            open = 0
        }
        /^(error|warning|note)(\[[^]]*\])?:/ {
            emit()
            open = $0 ~ /^error(\[[^]]*\])?:/ && $0 !~ /^error: aborting due to /
            count = 0
            next
        }
        open && /-->/ {
            sub(/^[[:space:]]*-->[[:space:]]*/, "")
            sub(/:[0-9]+:[0-9]+$/, "")
            if (count == 0) first = $0
            count++
        }
        END { emit() }
    ' "${output_file}")
    if [ "${located}" -ne "${total}" ]; then
        echo "Verifier reported ${total} errors but ${located} source locations" >&2
        return 1
    fi
}
