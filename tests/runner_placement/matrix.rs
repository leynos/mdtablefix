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

/// Renders a YAML value as text: a string is itself, anything else is YAML.
pub fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_yaml::to_string(other).unwrap_or_default(),
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
        .filter_map(|row| reader::get(row, &key.0));
    let listed = reader::get(matrix, &key.0)
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
    let names_elsewhere = reader::get(job, "strategy")
        .map(value_text)
        .is_some_and(|strategy| strategy.contains("ubicloud"));
    match (placed.is_empty(), names_elsewhere) {
        (false, _) => placed,
        (true, true) => vec![value_text(runs_on)],
        (true, false) => Vec::new(),
    }
}
