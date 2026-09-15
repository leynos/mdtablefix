# Environment seam taxonomy

## Status

Accepted.

## Date

2026-09-06.

## Context

`mdtablefix` reads nothing from the process environment. An audit of `src/`,
`tests/`, and `test-macros/` found no call to `std::env::var`, `var_os`,
`vars`, `vars_os`, `set_var`, or `remove_var`; the only environment references
are the compile-time `env!("CARGO_PKG_VERSION")` and
`env!("CARGO_MANIFEST_DIR")` macros, which read Cargo's build-time values
rather than the running process.

That is a property worth keeping rather than a coincidence worth ignoring.
Ambient environment access is a shared-mutable-state problem: a test that sets
or removes a variable changes it for every other test in the same process, so
the suite has to be serialized around it. Serialization costs wall-clock time
and leaves cores idle, and the cost grows with the suite. `AGENTS.md` already
forbids direct environment mutation in tests and points contributors at
dependency injection and the `mockable` crate, but nothing in the build
enforced that instruction, and no document said which injection shape to reach
for.

Enforcing the zero-state now is far cheaper than migrating call sites later,
which is what `leynos/netsuke` had to do across three issues before it could
state the taxonomy this record mirrors.

## Decision

### Prohibit ambient access

`clippy.toml` lists the six environment methods under `disallowed-methods`,
each with a reason string that names the remedy, and both package manifests
raise `clippy::disallowed_methods` to `deny`. The Makefile's `lint` target runs
Clippy over every target and every feature with warnings denied, so the
prohibition covers test and build targets, not only the library.

Both manifests also deny `clippy::allow_attributes` and
`clippy::allow_attributes_without_reason`. Without them the ban has an obvious
escape: a bare `#[allow(clippy::disallowed_methods)]` silences it wherever it is
written, records no justification, and never warns once it stops applying. With
them, the only available suppression is an `#[expect]` carrying a reason, which
is exactly what the composition-root exception below requires. The rule is
enforced rather than merely stated.

`clippy::allow_attributes` does not fire on an *inner* attribute, so
`#![allow(clippy::disallowed_methods, reason = "...")]` at the top of a module
switches the ban off for everything in it and passes `make lint`. Naming the
lint is not even necessary: Clippy places `disallowed_methods` in the `style`
group, so `clippy::style` and the wider `clippy::all` each switch it off without
mentioning it, and `#![cfg_attr(all(), allow(clippy::disallowed_methods))]` is
honoured too. All three were measured against this repository.

Clippy cannot close that itself, so `tests/env_access_suppressions.rs` parses
every `.rs` file below the repository root with `syn` and fails if any
attribute allows a protected lint. The walk starts at the root rather than at a
list of source directories, skipping only `target` and dotted directories, so
it covers build scripts, benches, examples, second binaries and anything added
outside the usual places, compiled or not. It follows
`cfg_attr`, reaches attributes on nested and function-local items, walks macro
token streams, and compares lint paths rather than substrings. Parsing
rather than searching is deliberate: a text scan cannot follow `cfg_attr`, and
cannot tell an attribute from attribute-shaped text in a string literal or a
doc comment, which is how its first draft reported this repository's own
mutation records as violations.

Two shapes are refused structurally rather than by the meta they carry,
because neither is a complete attribute where it is written. A `macro_rules!`
arm may forward the attribute's *path*, writing `#[$attr]` and letting its
caller supply `allow`: the arm's attribute does not parse and the invocation
carries no `#`, so neither half is a suppression alone. And `rustc` parses an
`include!` target as Rust whatever its extension, so an `allow` inside a
`.rs.txt` fixture silences the calls around the inclusion while an enclosing
`expect` stays fulfilled and warns about nothing.

Both rules are narrow on purpose, since a contract that reports a false
positive gets switched off and then reports nothing. A forwarded path is
refused at inner scope, which applies to everything around it, and at outer
scope only where the arm writes an `env` access itself or forwards a fragment
the caller fills with code; `$(#[$meta:meta])*` carrying doc comments onto a
generated setter is left alone, as are `#[doc = $text]` and
`#[derive($traits)]`, whose paths are written out. An `include!` is a finding
unless its target is a literal `.rs` path, which the scan reads in its own
right; `include_str!` and `include_bytes!` embed bytes rather than compiling
source. The walk descends only into `macro_rules!` transcribers, an attribute
handed to an invocation being the macro's to discard.

The lint level is declared once, in the workspace's `[workspace.lints.clippy]`
table, and each member inherits it with `[lints] workspace = true`. One
`cargo clippy --workspace` run then lints every member in its own right, so
`test-macros` is governed as a package rather than as a capped dependency.
Issue #439 made the two packages one workspace and retired the earlier
arrangement, a per-package `[lints.clippy]` table in each manifest and a second
`--manifest-path test-macros/Cargo.toml` invocation in the `lint` recipe.
Issue #438 aligns the wider lint baseline with `netsuke`.

The test suite has two composition roots of its own, each carrying the
item-scoped `expect`. `write_failure_child` in `tests/rewrite_atomic.rs` is the
child half of a test that re-execs this binary under `ulimit -f`, so the
environment its parent composed is the only channel into that process.
`ambient_variable` in `tests/support/idempotence_harness.rs` reads
`PROPTEST_CASES`, which proptest's own configuration honours and whoever runs
the suite sets; `case_count` takes the reader as an argument, so the parsing and
the fallback are exercised without any read at all.

### Choose a seam by call-site count

When a future change does need a value that the environment supplies, inject
it. Pick the shape by how many call sites the boundary has and whether it is
expected to grow:

- **An explicit value**, for one-off configuration. The caller resolves the
  value once and passes it as an argument. This is the default and covers most
  cases; prefer it until a second caller appears.
- **A narrow reader closure**, for a small reusable boundary. The module owns a
  private function taking an `FnOnce(&str) -> Result<String, env::VarError>` (or
  the `OsString`-typed equivalent) instead of reading the process itself. Tests
  supply a closure returning fixed values.
- **A shared environment trait**, only when several values and several tests
  justify it. `mockable`'s `Env` trait is the intended vehicle: production
  supplies `DefaultEnv`, tests supply `MockEnv`. Do not introduce a trait for a
  single variable read by a single caller.

None of these is a general-purpose environment service. Each is owned by the
module that needs its value, stays private to that module, and covers one
variable or one precedence ladder.

### Permit one exception, scoped to an item

A direct read is permitted only at a genuine executable composition root, which
in this repository means `main` or a function it calls directly to assemble the
command-line application. Such a site carries
`#[expect(clippy::disallowed_methods, reason = "...")]` on the item, never
`allow` and never a module- or crate-wide suppression. `expect` is deliberate:
once the site is migrated to a seam, the expectation goes unfulfilled and warns,
so the exception removes itself instead of rotting.

### Build child environments explicitly

Integration tests spawn the `mdtablefix` binary through `assert_cmd`. Where such
a test needs a controlled variable in the child, it sets it on the command with
`Command::env` or clears it with `Command::env_remove`, as
`tests/static_regex_lint.rs` does for `RG`. Mutating the test process's own
environment so a child inherits the change is not an accepted alternative; it
reintroduces exactly the shared state the policy exists to prevent.

## Consequences

- Ambient environment access cannot enter the repository unnoticed: a new call
  fails `make lint` with a diagnostic naming the remedy.
- The test suite has no environment-induced reason to serialize, so no
  serialization group is needed and none should be added for that reason.
- Contributors adding environment-dependent behaviour have a stated rule for
  choosing a seam, rather than three defensible shapes and no yardstick.
- `tests/env_access_policy.rs` fails if any of the six entries leaves
  `clippy.toml`, if either package stops denying one of the three policy lints,
  or if the `lint` recipe stops running Clippy over both packages, every target,
  and every feature with warnings denied.
- `tests/env_access_enforcement.rs` fails if the lint stops firing. It runs
  Clippy over a fixture package that calls all six methods, under this
  repository's own configuration, and asserts one diagnostic per method carrying
  that method's reason string. A configuration can keep its shape and still lint
  nothing, so shape alone was not enough to rely on.
- The only `#[allow]` in the repository, an unexplained
  `clippy::unnecessary_map_or` suppression in `src/lists.rs`, went away with the
  lint it silenced: the code now uses `is_none_or`, which is what the lint asked
  for.
- The compile-time `env!` macros are unaffected. They resolve during the build
  and read no process state.

## References

- [Netsuke seam taxonomy](https://github.com/leynos/netsuke/blob/main/docs/adr-008-environment-seam-taxonomy.md),
  the record this one mirrors.
- [Issue #441](https://github.com/leynos/mdtablefix/issues/441):
  requested this policy.
- [Issue #438](https://github.com/leynos/mdtablefix/issues/438):
  aligns the wider lint baseline with netsuke's.
- [Issue #439](https://github.com/leynos/mdtablefix/issues/439):
  makes the two packages one workspace, moving the lint declaration there.
- [Clippy's `allow_attributes`](https://rust-lang.github.io/rust-clippy/master/index.html#allow_attributes):
  the check that does not fire on inner attributes.
