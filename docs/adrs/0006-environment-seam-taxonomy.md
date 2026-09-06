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

The lint level is declared in each package's own `[lints.clippy]` table, and
`lint` runs Clippy twice: once for the root package and once with
`--manifest-path test-macros/Cargo.toml`. Both are interim measures forced by
the repository not yet being a Cargo workspace. `test-macros` is a path
dev-dependency rather than a member, so the root invocation does not lint it and
Cargo caps its `deny` while it compiles as a dependency; the second invocation
lints it in its own right. Issue #439 makes the two packages one workspace and
moves lint policy to `[workspace.lints.*]`, and issue #438 aligns the wider lint
baseline with `netsuke`. Once #439 lands, each package inherits the level with
`[lints] workspace = true` and a single `--workspace` run replaces both
invocations.

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
  `clippy.toml`, if either package stops denying the lint, or if the Makefile's
  Clippy gate stops covering every target and feature with warnings denied.
- The compile-time `env!` macros are unaffected. They resolve during the build
  and read no process state.

## References

- [Netsuke seam taxonomy](https://github.com/leynos/netsuke/blob/main/docs/adr-008-environment-seam-taxonomy.md),
  the record this one mirrors.
- Issue #441, which requested this policy; issues #438 and #439, which move the
  lint declaration to the workspace.
