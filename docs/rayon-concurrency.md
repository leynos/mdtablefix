# Concurrency with `rayon`

`mdtablefix` uses the `rayon` crate to process multiple files concurrently.
`rayon` provides a work-stealing thread pool and simple parallel iterators. The
tool relies on Rayon’s global thread pool so that no manual setup is required.
The dependency is specified as `^1.0` in `Cargo.toml` to track stable API
changes within the same major release.

Parallelism is enabled automatically whenever more than one file path is
provided on the command line. Each worker gathers its output before printing so
results appear in the original order. This buffering increases memory usage and
may reduce performance if many tiny files are processed.

```mermaid
sequenceDiagram
    participant User as actor User
    participant CLI as CLI Main
    participant FileHandler as handle_file
    participant Stdout as Stdout
    participant Stderr as Stderr

    User->>CLI: Run CLI with multiple files (not in-place)
    CLI->>FileHandler: handle_file(file1)
    CLI->>FileHandler: handle_file(file2)
    CLI->>FileHandler: handle_file(file3)
    Note over CLI,FileHandler: Files processed in parallel
    FileHandler-->>CLI: Result (Ok(Some(output)) or Err(error))
    loop For each file in input order
        CLI->>Stdout: Print output (if Ok)
        CLI->>Stderr: Print error (if Err)
    end
    CLI-->>User: Exit (with error if any file errored)
```
