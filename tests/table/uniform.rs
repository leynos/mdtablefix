//! Uniform column width regression tests.

use super::*;

fn assert_uniform_column_widths(output: &[String]) {
    assert!(!output.is_empty());
    let widths: Vec<usize> = output[0]
        .trim_matches('|')
        .split('|')
        .map(str::len)
        .collect();
    for row in output {
        let cols: Vec<&str> = row.trim_matches('|').split('|').collect();
        for (i, col) in cols.iter().enumerate() {
            assert_eq!(col.len(), widths[i]);
        }
    }
}

#[test]
fn test_uniform_example_one() {
    let input = lines_vec![
        "| Logical type | PostgreSQL | SQLite notes |",
        "|--------------|-------------------------|-------------------------------------------|",
        "| strings | `TEXT` (or `VARCHAR`) | `TEXT` - SQLite ignores the length specifier anyway |",
        "| booleans | `BOOLEAN DEFAULT FALSE` | declare as `BOOLEAN`; Diesel serialises to 0 / 1 so this is fine |",
        "| integers | `INTEGER` / `BIGINT` | ditto |",
        "| decimals | `NUMERIC` | stored as FLOAT in SQLite; Diesel `Numeric` round-trips, but beware precision |",
        "| blobs / raw | `BYTEA` | `BLOB` |",
    ];
    let output = reflow_table(&input);
    assert_uniform_column_widths(&output);
}

#[test]
fn test_uniform_example_two() {
    let input = lines_vec![
        "| Option | How it works | When to choose it |",
        "|--------------------------------------|----------------------------------------------------------------|---------------------------------------------------------------|",
        "| **B. Pure-Rust migrations** | Implement `diesel::migration::Migration<DB>` in a Rust file (`up.rs` / `down.rs`) and compile with both `features = [\"postgres\", \"sqlite\"]`. The query builder emits backend-specific SQL at runtime. | You prefer the type-checked DSL and can live with slightly slower compile times. |",
        "| **C. Lowest-common-denominator SQL** | Write one `up.sql`/`down.sql` that *already* works on both engines. This demands avoiding SERIAL/IDENTITY, JSONB, `TIMESTAMPTZ`, etc. | Simple schemas, embedded use-case only, you are happy to supply integer primary keys manually. |",
        "| **D. Two separate migration trees** | Maintain `migrations/sqlite` and `migrations/postgres` directories with identical version numbers. Use `embed_migrations!(\"migrations/<backend>\")` to compile the right set. | You ship a single binary with migrations baked in. |",
    ];
    let output = reflow_table(&input);
    assert_uniform_column_widths(&output);
}
