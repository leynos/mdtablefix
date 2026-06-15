# Architectural decision record (ADR) 0004: Keep bespoke state machines explicit

## Status

Accepted. Keep the current parser and wrapping state machines explicit rather
than adopting an external Rust state-machine crate.

## Date

2026-06-13.

## Context and problem statement

`mdtablefix` contains several small state machines that protect Markdown syntax
while transforming a stream of lines. The clearest examples are:

- `ProcessBuffer` in `src/process.rs`, which tracks table buffering, table
  flushing, ellipsis ordering, and fence-aware passthrough;
- continuation helpers in `src/wrap/continuation.rs`, which join deferred
  prefixed lines, track inline-code span closure, split close-then-reopen
  continuations, and choose between normalized and verbatim emission;
- link-reference and paragraph state helpers in `src/wrap/link_reference.rs`
  and `src/wrap/paragraph.rs`, which keep short-lived parsing windows and
  pending-prefix state separate from the main wrapping loop.

These machines are not independent business workflows. They are parsing and
rewriting mechanisms whose transitions depend on source-line content, local
Markdown classification, and immediate output side effects. The desired
abstraction would need to reduce boilerplate, keep transition decisions
observable, propagate errors consistently where transitions can fail, and make
the code easier for maintainers to scan.

External research found several Rust finite-state-machine crates:

- `rust-fsm` provides a trait and declarative syntax for classical, Mealy, and
  Moore machines. It can generate state, input, and output enums, but its own
  documentation notes that the DSL cannot attach data to states, pushing
  complex cases back to manual implementations.[^1]
- `smlang` provides a `no_std` procedural macro with transition lists, guards,
  actions, wildcard states, internal transitions, state data, event data, async
  guards/actions, and logging hooks for events, guards, actions, and state
  transitions.[^2]
- `statig` provides hierarchical event-driven state machines with state-local
  storage, shared storage, context, async support, and introspection callbacks
  around dispatch and transitions.[^3]
- `sfsm` generates static embedded-oriented state machines and includes a
  fallible-machine variant plus feature-gated tracing callbacks, but requires
  users to implement state and transition traits around the generated graph.[^4]
- `finny` provides a procedural builder API with compile-time transition graph
  validation, guards, actions, state regions, event queueing, submachines, and
  timers. Its current public documentation is sparse, and the latest version
  listed as of 2026-06-13 is `0.2.0`.[^5]
- `typed-fsm` is a recent zero-dependency-by-default event-driven framework
  with lifecycle hooks, optional logging, guards expressed in process handlers,
  and concurrency/interrupt support. It currently has low adoption compared
  with the established crates.[^6]
- `state-machines` offers a Ruby-inspired typestate-oriented DSL with guards,
  callbacks, async support, hierarchical states, and optional dynamic dispatch.
  Its project description says it is also a learning platform for Rubyists
  moving to Rust.[^7]

As of 2026-06-13, the crates.io `state-machine` keyword page shows `statig` as
the most prominent general-purpose crate by recent downloads, while other
candidates either target testing, embedded generation, async actors, or
lower-adoption niches.[^8]

## Decision outcome

Do not adopt an external Rust state-machine crate for the current `mdtablefix`
parser and wrapping state machines.

Instead, keep these machines as explicit Rust enums, structs, and helper
methods, and standardize the local pattern for future state-machine work:

- Name the state carrier directly after the domain state it owns, for example
  `ProcessBuffer`, `PendingPrefix`, or `LinkTitleWindow`.
- Keep transition methods small and event-shaped. A method such as
  `handle_table_line` should describe the input event and return whether it
  consumed the line, emitted output, or left the caller to continue normal
  processing.
- Model transition modes with small enums when a boolean would hide intent, as
  `ContinuationMode` already does.
- Emit `tracing` events at non-obvious branch points, especially when the
  branch preserves source verbatim, rejects a candidate transition, or changes
  a pending continuation mode.
- Return typed domain errors from transition helpers only when a transition can
  fail in a way the caller can inspect or recover from. Do not introduce opaque
  application errors in library APIs.
- Add focused unit tests around each transition boundary. Use behavioural tests
  when a state machine affects externally visible stream output.

The implementation work needed to apply this pattern is tracked in the
[state-machine abstractions roadmap](../state-machine-abstractions-roadmap.md).

## Consequences

Positive:

- The implementation stays close to the Markdown parsing problem. Maintainers
  can read branch predicates and side effects in ordinary Rust without learning
  a macro-specific transition DSL.
- The project avoids adding procedural macro dependencies for code that is
  already compact and tightly coupled to stream rewriting side effects.
- Existing `tracing` instrumentation can continue to describe domain-specific
  decisions such as width-triggered verbatim continuations, rather than being
  forced through generic event names.
- Error propagation remains aligned with the repository policy: semantic error
  enums are introduced where callers can act on them, while infallible parser
  transitions stay infallible.

Negative:

- Compile-time validation of transition graphs remains manual. Tests and review
  discipline must catch accidental missing transitions.
- Diagram generation and generated transition tables are not available unless
  added separately.
- Different modules can still drift in style unless future changes follow the
  local pattern above.

## Options considered

### Adopt `statig`

`statig` is the strongest general-purpose candidate. It supports dynamic
event-driven systems, state-local storage, hierarchical superstates, async
handlers, and explicit introspection callbacks.[^3] That makes it a good fit
for larger workflows with externally submitted events.

It is not a good fit for the current `mdtablefix` machines. `ProcessBuffer` and
the continuation helpers are not long-lived event queues; they are small
parsing contexts whose transitions are interleaved with string inspection,
buffer mutation, and immediate output emission. Adopting `statig` would turn
line-classification branches into an event model without removing much of the
underlying Markdown-specific logic.

### Adopt `smlang`

`smlang` has useful table-driven transition syntax, guard/action support, async
support, and hook methods for logging state-machine activity.[^2] It is
attractive where a transition graph is naturally expressed as a compact list of
state/event/guard/action rows.

The cost is that `mdtablefix` would need to invent events for source-line
classification, table continuation detection, code-span closure, and
verbatim-flush decisions. Most of the complexity would move into guards and
actions, so the resulting code would be less direct than the current helper
functions.

### Adopt `rust-fsm`

`rust-fsm` is mature and simple. It can generate input, state, and output enums
and can produce Mermaid diagrams through a feature flag.[^1]

Its DSL limitation around state data is a poor match for the continuation
state, which carries pending text, original source lines, width measurements,
synthetic join offsets, hard-break status, and the open fence length. Manual
implementation would be required for the complicated cases, weakening the
boilerplate-reduction argument.

### Adopt `sfsm` or `finny`

`sfsm` and `finny` both emphasize generated static machines with explicit state
and transition definitions.[^4][^5] They are better suited to embedded or
workflow-style systems where the transition graph is the main artefact.

For `mdtablefix`, these crates would introduce generator and trait boilerplate
around parsing logic that is already more important than the graph shape.
`sfsm`'s tracing and fallible variants are useful signals, but they do not
justify adopting the full state-machine framework for the current code.

### Adopt `typed-fsm` or `state-machines`

`typed-fsm` and `state-machines` both provide type-safe macro DSLs with useful
hooks. `typed-fsm` is lightweight and explicit about logging, lifecycle hooks,
and zero-dependency defaults, but it is new and has low current adoption.[^6]
`state-machines` is feature-rich, but its typestate-first model is aimed at API
sequences and educational parity with a Ruby library.[^7]

Both options would still require wrapping the current parser branches in
synthetic events. Neither provides a clear maintainability win over local enums
and helper methods.

## Recommendation

Keep the status quo for dependency adoption. Do not add a state-machine crate
until a future subsystem has all of these properties:

- the transition graph is larger or more important than the per-transition
  parsing code;
- transitions are driven by explicit events rather than source-line predicates;
- the machine has reusable lifecycle hooks, recoverable transition errors, or
  introspection requirements that would otherwise be repeated across modules;
- a spike shows that the generated or framework-driven code is shorter and
  more legible than the equivalent explicit Rust.

If that threshold is met, evaluate `statig` first for dynamic event-driven
machines and `smlang` second for compact transition-table machines. Revisit
`typed-fsm` only if its adoption and documentation mature enough to offset the
risk of adding a newer macro dependency.

[^1]: [`rust-fsm` crate documentation](https://crates.io/crates/rust-fsm).
[^2]: [`smlang` crate documentation](https://crates.io/crates/smlang).
[^3]: [`statig` crate documentation](https://crates.io/crates/statig).
[^4]: [`sfsm` crate documentation](https://docs.rs/sfsm).
[^5]: [`finny` crate documentation](https://docs.rs/finny).
[^6]: [`typed-fsm` crate documentation](https://crates.io/crates/typed-fsm).
[^7]: [`state-machines` crate documentation](https://crates.io/crates/state-machines).
[^8]: [crates.io `state-machine` keyword results](https://crates.io/keywords/state-machine).
