# State-machine abstractions roadmap

This roadmap turns [ADR 0004](adrs/0004-state-machine-abstractions.md) into
implementation work for the parser and wrapping state machines. The ADR keeps
the current explicit Rust state machines, rejects external state-machine crates
for the present codebase, and defines a local pattern for future helpers.

The roadmap follows the GIST model: phases carry ideas, steps validate those
ideas, and tasks are review-sized execution units. It does not promise dates.

## 1. Align existing stateful helpers with ADR 0004

Idea: if the current helpers already expose explicit state carriers,
event-shaped transitions, typed modes, instrumentation, and focused transition
tests, maintainers can keep the bespoke approach without relying on convention
alone.

This phase audits and tightens the existing state machines where ADR 0004's
pattern is already visible but unevenly documented or tested.

### 1.1. Verify state ownership and transition names

This step answers whether existing helpers make their state carriers and
transition boundaries clear enough for review. The outcome informs whether
future parser work can reuse the same naming pattern directly.

- [ ] 1.1.1. Audit `ProcessBuffer`, `PendingPrefix`,
  `DefinitionScanState`, `HtmlTableState`, and `ListState` for state-carrier
  names that describe the domain state they own.
  - See ADR 0004.
  - Success: every audited helper either follows the ADR naming pattern or has
    a documented reason to keep its current name.
- [ ] 1.1.2. Rename or extract transition helpers where a method hides more
  than one input event or output decision.
  - Requires 1.1.1.
  - See ADR 0004.
  - Success: transition methods read as event handlers, and callers can tell
    whether each helper consumed input, emitted output, or deferred normal
    processing.

### 1.2. Make transition modes and diagnostics explicit

This step answers whether the current machines expose the decisions reviewers
need when behaviour changes. The outcome determines the minimum diagnostic
standard for new stateful helpers.

- [ ] 1.2.1. Replace ambiguous boolean transition parameters or return values
  with small enums where a helper has more than two meaningful outcomes.
  - Requires 1.1.2.
  - See ADR 0004.
  - Success: transition modes are named after domain behaviour rather than
    implementation mechanics.
- [ ] 1.2.2. Add `tracing` events to non-obvious transitions that preserve
  input verbatim, reject a candidate transition, or change continuation mode.
  - Requires 1.2.1.
  - See ADR 0004 and docs/developers-guide.md §Stateful pipeline helpers.
  - Success: maintainers can inspect branch decisions without parsing emitted
    Markdown output by hand.

### 1.3. Lock transition boundaries with tests

This step answers whether the explicit helper pattern is backed by enough
regression coverage to compensate for not using generated transition graphs.
The outcome defines the expected test shape for future state-machine changes.

- [ ] 1.3.1. Add focused unit tests for state transition boundaries that are
  not already covered by table-driven cases.
  - Requires steps 1.1-1.2.
  - See ADR 0004.
  - Success: each transition helper has happy-path and edge-case coverage for
    the state it owns.
- [ ] 1.3.2. Add behavioural coverage for stream-output changes caused by
  parser or wrapping state transitions.
  - Requires 1.3.1.
  - See ADR 0004 and docs/architecture.md §Markdown stream processor.
  - Success: externally visible output remains stable for table buffering,
    fence passthrough, continuation joining, and verbatim fallback cases.

## 2. Establish the future adoption checkpoint

Idea: if future state-machine work has a concrete checkpoint before it adds a
crate, dependency decisions can be made from local evidence rather than from
general framework appeal.

This phase converts ADR 0004's dependency threshold into a repeatable workflow
for future parser or wrapping changes.

### 2.1. Document the checkpoint in maintainer guidance

This step answers where maintainers should look before adopting `statig`,
`smlang`, or another state-machine crate. The outcome keeps dependency review
close to the code that would use the abstraction.

- [ ] 2.1.1. Extend the developer guide with a state-machine adoption checklist
  that mirrors ADR 0004's recommendation threshold.
  - See ADR 0004 and docs/developers-guide.md §Stateful pipeline helpers.
  - Success: a maintainer can decide when to spike an external crate without
    rereading the whole ADR.
- [ ] 2.1.2. Add an ADR update template note for any future state-machine crate
  spike.
  - Requires 2.1.1.
  - See ADR 0004.
  - Success: future dependency proposals record graph size, event model,
    lifecycle hooks, error handling, generated-code legibility, and adoption
    risk.

### 2.2. Keep dependency adoption evidence local

This step answers how the project will compare explicit Rust against a crate if
a future subsystem crosses the ADR threshold. The outcome gives reviewers a
small, reproducible spike shape instead of a speculative rewrite.

- [ ] 2.2.1. Define a spike fixture for comparing an explicit helper against a
  crate-backed version when a future subsystem meets the ADR threshold.
  - Requires 2.1.2.
  - See ADR 0004.
  - Success: the spike demonstrates transition readability, diagnostic hooks,
    error handling, and test coverage before dependency adoption is proposed.
- [ ] 2.2.2. Record any accepted crate-backed state-machine experiment in a new
  or amended ADR before production adoption.
  - Requires 2.2.1.
  - See ADR 0004.
  - Success: production code does not gain a state-machine crate without a
    committed design record and passed quality gates.

## 3. Defer generated transition graphs until they justify themselves

Idea: if explicit state machines continue to serve the current stream rewriting
work, generated graphs and macro DSLs can stay outside the core implementation
until they solve a demonstrated maintenance problem.

This phase keeps deferred work visible without turning it into current scope.

### 3.1. Revisit visualisation only for larger machines

This step answers whether diagram generation or transition tables would reduce
review risk for a future, larger machine. The outcome prevents small helpers
from carrying framework overhead for documentation value alone.

- [ ] 3.1.1. Re-evaluate transition diagrams when a future helper has enough
  states or events that review cannot reliably track the graph from tests and
  code.
  - Requires phase 2.
  - See ADR 0004.
  - Success: diagram generation is adopted only when it shortens review and
    maintenance work for a concrete machine.
