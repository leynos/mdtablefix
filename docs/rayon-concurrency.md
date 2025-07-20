# Concurrency with `rayon`

`mdtablefix` uses the `rayon` crate to process multiple files concurrently.
`rayon` provides a work-stealing thread pool and simple parallel iterators. The
tool relies on Rayon’s global thread pool so that no manual setup is required.
The version is pinned to `1.10` in `Cargo.toml` to avoid breaking changes from
a future major release.
