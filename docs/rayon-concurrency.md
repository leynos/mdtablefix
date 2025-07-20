# Concurrency with `rayon`

`mdtablefix` uses the `rayon` crate to process multiple files concurrently.
`rayon` provides a work-stealing thread pool and simple parallel iterators. The
version is pinned to `1.10` in `Cargo.toml` to avoid breaking changes from a
future major release.
