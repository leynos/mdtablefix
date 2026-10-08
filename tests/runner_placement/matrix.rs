//! Reads a placement taken from a job's matrix.
//!
//! A `runs-on` may read a matrix value instead of naming a runner, as a string
//! (`${{ matrix.runner }}`, `${{ matrix['runner'] }}`) or as an entry of a list
//! (`["${{ matrix.runner }}"]`). The placement is then decided by the matrix
//! rows, so the rows are what the contract has to read.

use serde_yaml::{Mapping, Value};

use super::reader;

/// A key read from the matrix context.
#[derive(Debug, PartialEq, Eq)]
struct MatrixKey(String);

impl MatrixKey {
    /// Returns every key `text` reads from the matrix, in either access form.
    fn all_in(text: &str) -> Vec<Self> {
        let is_name = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
        text.match_indices("matrix")
            .filter_map(|(at, word)| {
                let rest = &text[at + word.len()..];
                let body = rest.strip_prefix('.').or_else(|| rest.strip_prefix("['"))?;
                let name: String = body.chars().take_while(|c| is_name(*c)).collect();
                (!name.is_empty()).then_some(Self(name))
            })
            .collect()
    }
}

/// Renders a YAML value as text: a string is itself, anything else is its
/// debug form.
///
/// The text is only searched for a runner label and judged against the
/// runner-selection shape, which a non-string value can never match, so the
/// debug form carries everything needed and, unlike a YAML rendering, cannot
/// fail and have its failure read as an empty value.
pub fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => format!("{other:?}"),
    }
}

/// Returns the value under `key`, matching the name without regard to case, as
/// GitHub does for matrix properties.
fn get_ci<'a>(mapping: &'a Mapping, key: &MatrixKey) -> Option<&'a Value> {
    mapping
        .iter()
        .find(|(name, _)| {
            name.as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(&key.0))
        })
        .map(|(_, value)| value)
}

/// Returns whether any value under `value`, at any depth, is a string naming
/// Ubicloud. Property names are not values: a matrix property called
/// `ubicloud_runner` over hosted labels does not place a job on Ubicloud.
fn values_name_ubicloud(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains("ubicloud"),
        Value::Sequence(items) => items.iter().any(values_name_ubicloud),
        Value::Mapping(entries) => entries.values().any(values_name_ubicloud),
        _ => false,
    }
}

/// Returns the keys a `runs-on` value reads from the matrix, whether it is one
/// string or a list whose entries are strings.
fn referenced_keys(runs_on: &Value) -> Vec<MatrixKey> {
    match runs_on {
        Value::String(text) => MatrixKey::all_in(text),
        Value::Sequence(entries) => entries
            .iter()
            .filter_map(Value::as_str)
            .flat_map(MatrixKey::all_in)
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns whether a `runs-on` value reads any key from the matrix.
pub fn reads_matrix(runs_on: &Value) -> bool { !referenced_keys(runs_on).is_empty() }

/// Returns every value a job's matrix gives `key`, from its `include` rows and
/// from a top-level list under that key.
fn values_for(job: &Mapping, key: &MatrixKey) -> Vec<String> {
    let Some(matrix) = reader::get(job, "strategy")
        .and_then(Value::as_mapping)
        .and_then(|strategy| reader::get(strategy, "matrix"))
        .and_then(Value::as_mapping)
    else {
        return Vec::new();
    };
    let rows = reader::get(matrix, "include")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_mapping)
        .filter_map(|row| get_ci(row, key));
    let listed = get_ci(matrix, key)
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten();
    rows.chain(listed).map(value_text).collect()
}

/// Returns the runner text of each place a job can land on Ubicloud through
/// its matrix, given the job and its `runs-on` value.
///
/// Each matrix value that names Ubicloud under a key the `runs-on` reads stands
/// for one place, so a placement taken from a row is judged on the row's own
/// runner-selection expression and hosted rows are left alone. When the matrix
/// names Ubicloud somewhere the `runs-on` does not read, the `runs-on` text
/// itself is returned, and the judgement rejects it: only a runner-selection
/// expression places a lane. A `runs-on` that reads no matrix key yields
/// nothing here.
pub fn matrix_placements(job: &Mapping, runs_on: &Value) -> Vec<String> {
    let keys = referenced_keys(runs_on);
    if keys.is_empty() {
        return Vec::new();
    }
    let placed: Vec<String> = keys
        .iter()
        .flat_map(|key| values_for(job, key))
        .filter(|value| value.contains("ubicloud"))
        .collect();
    let names_elsewhere = reader::get(job, "strategy").is_some_and(values_name_ubicloud);
    match (placed.is_empty(), names_elsewhere) {
        (false, _) => placed,
        (true, true) => vec![value_text(runs_on)],
        (true, false) => Vec::new(),
    }
}
