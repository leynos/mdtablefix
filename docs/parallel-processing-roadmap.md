# Roadmap for Parallel File Processing

The command-line tool currently processes input files sequentially. The steps
below outline the work required to allow concurrent processing while preserving
serial output order.

- [ ] **Adopt `rayon` for concurrency**
  - Use `rayon` thread pools to spawn work for each file path.
  - Ensure the approach integrates cleanly with existing modules.
- [ ] **Add chosen crate to `Cargo.toml`**
  - Pin an explicit version and document the decision in `docs/`.
- [ ] **Refactor `main.rs` to launch parallel tasks**
  - Spawn a worker for each file path using the concurrency crate.
  - Maintain a list of handles so outputs can be gathered in order.
- [ ] **Collect results sequentially**
  - Await or join handles in the same order the files were supplied.
  - Print each processed file or error message before moving to the next.
- [ ] **Extend tests for parallel execution**
  - Use `rstest` to verify that processing many files yields correct results.
  - Add tests exercising error handling when some paths are invalid.
- [ ] **Update documentation**
  - Document the new flags or behaviour in `README.md` and module docs.
  - Note any concurrency caveats or performance implications.
