# Stop reflowing code-block pipe lines into table rows

This ExecPlan (execution plan) is a living document. Keep these sections
current as work proceeds: `Constraints`, `Tolerances`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, and `Outcomes & retrospective`.

Status: COMPLETE

Issue: <https://github.com/leynos/mdtablefix/issues/373>

## Purpose / big picture

A line inside a code block that begins with `|` — for example a shell pipeline
continuation such as `| tee /tmp/test.log` (indented under the pipeline) — was
being treated as a
Markdown table row and "balanced" with an appended trailing `|`. The result,
`| tee /tmp/test.log |`, is an unterminated shell pipeline that hangs or errors
when copied and run. markdownlint does not catch the corruption, so it ships
silently.

After this change, content inside indented and fenced code blocks is treated as
literal. A leading `|` in a code block is never rewritten with a trailing `|`,
and the reported reproduction round-trips unchanged under the full stream
pipeline.

## Constraints

- Do not add new dependencies; reuse existing table and fence code.
- Do not change the public API exported from `src/lib.rs`.
- Keep existing table reflow and fence normalization semantics stable for all
  currently supported cases.
- Reuse the shared `FenceTracker` close decision; do not duplicate fence logic
  in `src/fences.rs`.
- Keep code files under the repository's 400-line limit.

## Tolerances

- Multi-row or multi-column degenerate tables (without a separator) keep their
  current behaviour; only the lone single-cell candidate is newly guarded.
- Whitespace-only info strings on a closing fence still close the fence, matching
  CommonMark's "spaces and tabs are ignored" allowance.

## Risks

- Over-broadening the minimum-structure guard could suppress legitimate
  single-column tables. Mitigated by gating on the absence of a separator row
  and a single row of a single cell.
- Changing fence-close semantics could regress existing fence tests. Mitigated
  by updating the one log-focused test that assumed an info-bearing closing
  fence closes, and by full-suite validation.

## Progress

- [x] Minimum-structure guard added to `reflow_table` (`src/table.rs`).
- [x] CommonMark closing-fence semantics enforced in `FenceTracker::observe`
  (`src/wrap/fence.rs`).
- [x] Unit, fence-tracker, and integration regression tests added.
- [x] CHANGELOG, ADR 0001, and this ExecPlan recorded.

## Surprises & discoveries

- `tests/table/` is not compiled by any Cargo target (orphaned during an earlier
  test reorg). The running process_stream coverage therefore lives in the
  compiled `tests/table_continuations.rs` target; the orphaned
  `tests/table/process_stream_tests.rs` case is retained for when a stub is
  eventually wired in.

## Decision log

- Guard the degenerate candidate in `parse_and_validate` next to the existing
  `rows_mismatched` bail-out rather than introducing a new abstraction.
- Gate the fence close on an empty (whitespace-only) info string, routing an
  info-bearing same-marker line to a dedicated `unchanged` arm with reason
  `closing_fence_has_info_string` for accurate telemetry.

## Plan of work

1. Harden `reflow_table` so a lone single-cell candidate with no separator row
   is returned unchanged.
2. Tighten `FenceTracker::observe` to CommonMark closing-fence semantics so pipe
   lines inside fenced blocks never leak into table detection.
3. Lock behaviour with tests and record the fix in the changelog, ADR, and this
   plan.

## Concrete steps

- `src/table.rs`: add the minimum-structure guard and a unit test.
- `src/wrap/fence.rs`: bind the info string and gate the close on it; add a
  distinct `unchanged` arm.
- `src/wrap/tests/fence_tracker.rs`: update the log test that assumed an
  info-bearing close, and add positive/negative coverage.
- `tests/table_continuations.rs`: single-pipe passthrough, the ticket's exact
  indented reproduction, and the fenced info-string case.
- `tests/fences.rs`: info-string marker remains literal content.
- `CHANGELOG.md`, `docs/adrs/0001-table-reflow-pipeline.md`, `docs/contents.md`.

## Acceptance criteria

- `| tee /tmp/test.log` passes through `reflow_table` unchanged.
- The ticket's exact input is idempotent under `process_stream`.
- A same-marker line with a non-whitespace info string does not close an open
  fence; a bare or whitespace-only marker still does.
- `make check-fmt`, `make lint`, `make test`, and `make markdownlint` pass.

## Outcomes & retrospective

The two-pronged fix removes the corruption at both the reflow entry point (the
minimum-structure guard) and the fence-detection boundary (CommonMark
closing-fence semantics), so pipe lines in both indented and fenced code blocks
stay literal. The orphaned `tests/table/` directory remains a latent
maintenance hazard; wiring it into Cargo is tracked separately.
