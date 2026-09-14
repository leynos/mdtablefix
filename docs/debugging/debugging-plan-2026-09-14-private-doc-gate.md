# Debugging Plan: fixed-point property failure during private-doc gate

**Generated**: 2026-09-14
**Issue ID**: #440 gate closeout
**Severity**: high
**Falsification sub-agent**: alchemist
**Planning agent boundary**: This document was prepared by the planning agent.
Falsification must be executed by the named sub-agent, not by the planning
agent.

## Problem Statement

The first full `make test` run after enabling the private documentation lint
failed `cli_formatting_reaches_a_fixed_point`. The property expected two passes
of the formatter with `--wrap --renumber --breaks` to agree, but the generated
document moved a bracketed numeric reference differently on the second pass.
The private-doc change should not alter formatter control flow, but the
required gate cannot pass until the failure is classified.

## Context Summary

| Aspect              | Details                                                                        |
| ------------------- | ------------------------------------------------------------------------------ |
| First observed      | 2026-09-14, first full gate on the #440 worktree                               |
| Reproduction rate   | One failing case from 48 generated cases                                       |
| Affected components | `process`, wrapping, renumbering, and break formatting                         |
| Recent changes      | Workspace lint inheritance, source comments, and a proc-macro lint expectation |

### Error Artefacts

```plaintext
minimal failing input: document =
"aa\n-----\n---\naaaaaaa aaaaaaaa aaaaa\n... **bold**`code`\n[1] and text... here\n",
mask = 7
flags: ["--wrap", "--renumber", "--breaks"]
pass 1 ends `code` [\n1] ...; pass 2 ends `code`\n[ 1] ...
```

### Information Gaps

- The first failure was a property-generated case, so its reproducibility from
  the minimized input has not yet been established.
- No baseline run on the pre-#440 commit has yet compared the same input.

______________________________________________________________________

## Hypotheses

### H1: the property exposes a pre-existing formatter fixed-point defect

**Claim**: The minimized input drifts on the parent commit because the current
formatter already handles a wrapped bracketed reference differently on the
second pass.

**Plausibility**: High — the #440 diff changes no formatter statements or
formatting data, and the failure is in an independently generated property.

**Prediction**: Running the minimized input twice against `HEAD` produces the
same first/second-pass mismatch.

#### H1 Falsification Plan

| Step | Action                                                                                                        | Expected Negative Result                                       |
| ---- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| 1    | Build the parent commit in a temporary worktree and format the minimized input twice with the reported flags. | Equal first and second outputs disprove a pre-existing defect. |

**Tooling**: temporary Git worktree, Cargo, and the built `mdtablefix` binary.

**Confidence on falsification**: High. The comparison holds source and
dependencies constant while removing the #440 patch.

______________________________________________________________________

### H2: the #440 policy change changes formatter behaviour indirectly

**Claim**: Workspace membership or the proc-macro expectation changes the
binary used by the property despite no formatter source edits.

**Plausibility**: Low — neither change targets the formatter, but Cargo target
selection could expose an unexpected build-time interaction.

**Prediction**: The minimized input is fixed on the parent commit but drifts in
the current worktree.

#### H2 Falsification Plan

| Step | Action                                                                                                       | Expected Negative Result                           |
| ---- | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------- |
| 1    | Compare the two-pass output from the parent worktree and current worktree with identical command-line flags. | Matching outputs disprove an indirect #440 effect. |

**Tooling**: temporary Git worktree, Cargo, and byte-for-byte output comparison.

**Confidence on falsification**: High. The experiment changes only the #440
patch between runs.

______________________________________________________________________

## Recommended Execution Order

1. **H1** — it is the cheapest decisive baseline comparison.
2. **H2** — it follows directly if the parent and current outputs differ.

## Falsification Results

- **H1 falsified**: the abbreviated fixture initially supplied to the
  experiment did not drift. The full fixture was then used for H2.
- **H2 falsified**: the full fixture drifts identically on the parent commit
  and the #440 worktree, so workspace lint inheritance and the proc-macro
  expectation do not affect formatter behaviour.

The independently validated fix is already on the open stacked branch for
[#504](https://github.com/leynos/mdtablefix/pull/505). The #440 branch must
therefore stack on that branch for the required deterministic gate to exercise
the corrected formatter, without duplicating its behavioural change.

## Termination Criteria

- **Root cause identified**: one hypothesis survives its comparison and the
  other is falsified.
- **Escalation trigger**: both comparisons are inconclusive because the parent
  build or deterministic reproduction cannot be obtained.

## Notes for Executing Agent

Use an exact temporary worktree, preserve the current shared worktree, and do
not modify tracked files. Report whether either hypothesis is falsified and
include the two output pairs as evidence.
