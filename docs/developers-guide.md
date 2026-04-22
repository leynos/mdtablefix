# Developers guide

## Frontmatter module visibility

The `frontmatter` module is an internal implementation detail of `mdtablefix`.
Its visibility is restricted to `pub(crate)` in the library so the crate does
not expose YAML frontmatter parsing as part of its supported public API.

This boundary matches the role of the module. The helper exists to shield a
leading YAML frontmatter block from Markdown transforms, including CLI-only
operations such as list renumbering and thematic break normalization. External
callers interact with that behaviour through higher-level formatting entry
points rather than by calling the frontmatter helper directly.

### Rationale

- Keep the public API focused on stable formatting operations rather than
  document pre-processing internals.
- Reduce the risk of accidental API commitments around a narrowly scoped
  helper that may need to change with the processing pipeline.
- Make the intended layering explicit: frontmatter detection supports stream
  processing, but it is not a general-purpose parsing API.

### Implications for internal code organization

The package binary in [src/main.rs](../src/main.rs) is compiled as a separate
crate from the library, so it cannot use library items marked `pub(crate)`. To
preserve CLI access without reopening the library surface, the binary includes
[src/frontmatter.rs](../src/frontmatter.rs) privately via:

```rust
#[path = "frontmatter.rs"]
mod frontmatter;
```

This arrangement keeps the helper available to both the library and the CLI
while maintaining a closed external API boundary.

When working in this area:

- Prefer wiring new behaviour through `process_stream_inner` or other public
  formatting APIs instead of exporting frontmatter helpers.
- Treat changes to frontmatter parsing rules as internal architectural changes
  that should update this guide and any affected behaviour documentation.
- Keep module documentation in sync in both `src/frontmatter.rs` and the
  private module declaration used by the binary.
