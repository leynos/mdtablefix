#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${FAKE_PROVER_TOOLS_LOG:?}"
printf 'VERUS_Z3_PATH=%s\n' "${VERUS_Z3_PATH:-}" >> "${FAKE_PROVER_TOOLS_LOG:?}"
if [[ "$*" == *".atx-mutation-"* ]]; then
    case "${FAKE_PROVER_TOOLS_SMOKE_MODE:?}" in
        mutation_accepted) exit 0 ;;
        mutation_runner_failure)
            echo "runner unavailable"
            exit 1
            ;;
        mutation_class_rejected)
            echo "error: postcondition not satisfied"
            echo "crate::spec_classify(result@, crate::canonical_context()) == LineClass::AtxHeading"
            ;;
        mutation_equality_only)
            echo "error: postcondition not satisfied"
            echo "result@ == crate::setext_spec::assembled_heading(prefix@, level as int, text@)"
            ;;
        mutation_invariant_only)
            echo "note: spec_classify(result@, ctx) == LineClass::AtxHeading"
            echo "error: invariant not satisfied at loop entry"
            ;;
    esac
    echo "verification results:: 70 verified, 1 errors"
    exit 1
fi
if [[ "$*" == *".emission-mutation-"* ]]; then
    case "${FAKE_PROVER_TOOLS_SMOKE_MODE:?}" in
        emission_rejected | emission_trigger_note)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.emission-mutation-fake/plan_spec_emission.rs:71:13
   |
71 |       ensures spec_delimiter_reads_back(source, emitted),
   |               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: postcondition not satisfied
   --> verus/.emission-mutation-fake/plan_spec_witnesses.rs:260:9
    |
258 |   proof fn lemma_witness_normalized_contract_refuses_a_wrong_respelling()
    |         ----------------------------------------------------------------- at the end of the function body
OUT
            if [[ "${FAKE_PROVER_TOOLS_SMOKE_MODE}" == emission_trigger_note ]]; then
                cat <<'OUT'

note: automatically chose triggers for this expression:
  --> verus/.emission-mutation-fake/plan_spec_fold_invariant.rs:82:9
OUT
            fi
            echo
            echo "verification results:: 229 verified, 2 errors"
            exit 1
            ;;
        emission_accepted) exit 0 ;;
        emission_runner_failure)
            echo "runner unavailable"
            exit 1
            ;;
        emission_unrelated_failure)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.emission-mutation-fake/plan_spec_agreement.rs:12:5

verification results:: 230 verified, 1 errors
OUT
            exit 1
            ;;
        emission_parse_error)
            cat <<'OUT'
error: expected one of `,` or `>`, found `(`
  --> verus/.emission-mutation-fake/plan_spec_agreement.rs:79:5
error: aborting due to 1 previous error
OUT
            exit 1
            ;;
        emission_unlocated_failure)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.emission-mutation-fake/plan_spec_emission.rs:71:13

error: postcondition not satisfied

verification results:: 229 verified, 2 errors
OUT
            exit 1
            ;;
        emission_double_location)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.emission-mutation-fake/plan_spec_emission.rs:71:13
  --> verus/.emission-mutation-fake/plan_spec_agreement.rs:12:5

verification results:: 229 verified, 1 errors
OUT
            exit 1
            ;;
        emission_coded_error)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.emission-mutation-fake/plan_spec_emission.rs:71:13

error[E0425]: cannot find value `spec_delimiter_reads_back` in this scope
  --> verus/.emission-mutation-fake/plan_spec_emission.rs:74:9
error: aborting due to 2 previous errors

verification results:: 228 verified, 2 errors
OUT
            exit 1
            ;;
    esac
fi
if [[ "$*" == *".blank-tag-mutation-"* ]]; then
    case "${FAKE_PROVER_TOOLS_SMOKE_MODE:?}" in
        blank_tag_rejected | blank_tag_trigger_note)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.blank-tag-mutation-fake/plan_spec_witnesses.rs:293:9
   |
290 | proof fn lemma_witness_null_specifier_reads_back()
    |          --------------------------------------- at the end of the function body
293 |         spec_delimiter_reads_back(spec_witness_null_opener(), spec_witness_null_emitted()),
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition
OUT
            if [[ "${FAKE_PROVER_TOOLS_SMOKE_MODE}" == blank_tag_trigger_note ]]; then
                cat <<'OUT'

note: automatically chose triggers for this expression:
  --> verus/.blank-tag-mutation-fake/plan_spec_witnesses.rs:305:9
OUT
            fi
            echo
            echo "verification results:: 230 verified, 1 errors"
            exit 1
            ;;
        blank_tag_accepted) exit 0 ;;
        blank_tag_runner_failure)
            echo "runner unavailable"
            exit 1
            ;;
        blank_tag_unrelated_failure)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.blank-tag-mutation-fake/plan_spec_emission.rs:71:13

verification results:: 230 verified, 1 errors
OUT
            exit 1
            ;;
        blank_tag_precondition_failure)
            cat <<'OUT'
error: precondition not satisfied
  --> verus/.blank-tag-mutation-fake/plan_spec_witnesses.rs:293:9

verification results:: 230 verified, 1 errors
OUT
            exit 1
            ;;
        blank_tag_unlocated_failure)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.blank-tag-mutation-fake/plan_spec_witnesses.rs:293:9

error: postcondition not satisfied

verification results:: 229 verified, 2 errors
OUT
            exit 1
            ;;
        blank_tag_double_location)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.blank-tag-mutation-fake/plan_spec_witnesses.rs:293:9
  --> verus/.blank-tag-mutation-fake/plan_spec_emission.rs:71:13

verification results:: 230 verified, 1 errors
OUT
            exit 1
            ;;
        blank_tag_coded_error)
            cat <<'OUT'
error: postcondition not satisfied
  --> verus/.blank-tag-mutation-fake/plan_spec_witnesses.rs:293:9

error[E0425]: cannot find value `spec_delimiter_reads_back` in this scope
  --> verus/.blank-tag-mutation-fake/plan_spec_witnesses.rs:296:9
error: aborting due to 2 previous errors

verification results:: 228 verified, 2 errors
OUT
            exit 1
            ;;
    esac
fi
if [[ "$*" == "verus run --repo-root . --no-install-missing --proof-file verus/smoke.rs" ]]; then
    case "${FAKE_PROVER_TOOLS_SMOKE_MODE:?}" in
        rejected)
            echo "Verus proofs failed"
            exit 1
            ;;
        accepted) exit 0 ;;
        unrelated_failure)
            echo "runner unavailable"
            exit 1
            ;;
    esac
fi
