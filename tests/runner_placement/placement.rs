//! The runner-placement judgement: where a lane runs, and for how long.
//!
//! Ubicloud's cache proxy is scoped by ref, and a pull request from a fork
//! cannot obtain an Ubicloud runner at all. A lane that names an Ubicloud
//! runner therefore selects it by an expression that falls back to the hosted
//! pool for a fork, and states its own ceiling, because an Ubicloud runner is
//! a self-hosted just-in-time runner that GitHub's six-hour cap for hosted
//! jobs does not bound.

use serde_yaml::{Mapping, Value};

use super::reader;

/// The hosted runner a fork's pull request falls back to.
pub const HOSTED_LABEL: &str = "ubuntu-latest";

/// The context value that is true only for a pull request from a fork.
const FORK_CONDITION: &str = "github.event.pull_request.head.repo.fork";

/// The origin of a run, as far as the runner expression can tell.
#[derive(Clone, Copy, Debug)]
pub enum Origin {
    /// A push or a dispatch: there is no pull request, so the fork value is null.
    NoPullRequest,
    /// A pull request from a branch of this repository.
    SameRepository,
    /// A pull request from a fork.
    Fork,
}

/// Removes one pair of single quotes, or returns `None` for an unquoted term.
fn unquoted(term: &str) -> Option<&str> { term.trim().strip_prefix('\'')?.strip_suffix('\'') }

/// Returns the label a `runs-on` expression selects for a run, or `None` when
/// it is not the estate's `<fork> && '<hosted>' || '<label>'` shape.
///
/// A literal label is not the shape and returns `None`: a lane that never
/// falls back cannot serve a fork, and a lane that never leaves the hosted
/// pool is not placed at all.
pub fn selected_runner(runs_on: &str, origin: Origin) -> Option<String> {
    let body = runs_on
        .trim()
        .strip_prefix("${{")?
        .strip_suffix("}}")?
        .trim();
    let (condition, arms) = body.split_once(" && ")?;
    let (fork_arm, other_arm) = arms.split_once(" || ")?;
    if condition.trim() != FORK_CONDITION {
        return None;
    }
    let chosen = match origin {
        Origin::Fork => fork_arm,
        Origin::NoPullRequest | Origin::SameRepository => other_arm,
    };
    unquoted(chosen).map(str::to_owned)
}

/// Returns why an expression misplaces a lane, one entry per origin it gets
/// wrong; empty when a fork falls back to hosted and every other run is on
/// the Ubicloud runner class `label`.
pub fn placement_faults(runs_on: &str, label: &str) -> Vec<String> {
    [
        (Origin::NoPullRequest, label),
        (Origin::SameRepository, label),
        (Origin::Fork, HOSTED_LABEL),
    ]
    .into_iter()
    .filter_map(|(origin, wanted)| {
        let chosen = selected_runner(runs_on, origin);
        (chosen.as_deref() != Some(wanted))
            .then(|| format!("{origin:?} selects {chosen:?}, wanted {wanted:?}"))
    })
    .collect()
}

/// Returns a job's `runs-on` as text, whatever shape it is written in.
///
/// A string is itself. A sequence or a mapping (`[ubicloud-standard-2]`,
/// `{ group: ... }`) is rendered as YAML, so a label hidden in one still reads
/// as Ubicloud, and the judgement then rejects it: only the runner-selection expression
/// places a lane.
fn runs_on_text(job: &Mapping) -> Option<String> {
    match reader::get(job, "runs-on")? {
        Value::String(text) => Some(text.clone()),
        other => serde_yaml::to_string(other).ok(),
    }
}

/// Returns the key a `runs-on` reads from the matrix, for `${{ matrix.key }}`
/// and `${{ matrix['key'] }}`, or `None` when it reads something else.
fn matrix_key(runs_on: &str) -> Option<String> {
    let body = runs_on
        .trim()
        .strip_prefix("${{")?
        .strip_suffix("}}")?
        .trim();
    let rest = body.strip_prefix("matrix")?;
    let key = match rest.strip_prefix('.') {
        Some(dotted) => dotted,
        None => rest.strip_prefix("['")?.strip_suffix("']")?,
    };
    let is_name = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    is_name.then(|| key.to_owned())
}

/// Renders a matrix value as text: a string is itself, anything else is YAML.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_yaml::to_string(other).unwrap_or_default(),
    }
}

/// Returns every value a job's matrix gives `key`, from its `include` rows and
/// from a top-level list under that key.
fn matrix_values(job: &Mapping, key: &str) -> Vec<String> {
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
        .filter_map(|row| reader::get(row, key));
    let listed = reader::get(matrix, key)
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten();
    rows.chain(listed).map(value_text).collect()
}

/// Returns the runner text of each place a job can land on Ubicloud, or an
/// empty list when it cannot.
///
/// A `runs-on` that names Ubicloud is itself. One that reads a matrix value
/// (`${{ matrix.runner }}`) stands for each of the matrix's values that names
/// Ubicloud, so a placement taken from a matrix row is judged on the row's own
/// runner-selection expression, and the hosted rows are left alone. When the
/// matrix names Ubicloud somewhere the `runs-on` does not read, the `runs-on`
/// text itself is returned, and the judgement rejects it: only a runner-selection
/// expression places a lane.
fn runner_texts(job: &Mapping) -> Vec<String> {
    let Some(text) = runs_on_text(job) else {
        return Vec::new();
    };
    let Some(key) = matrix_key(&text) else {
        return if text.contains("ubicloud") {
            vec![text]
        } else {
            Vec::new()
        };
    };
    let placed: Vec<String> = matrix_values(job, &key)
        .into_iter()
        .filter(|value| value.contains("ubicloud"))
        .collect();
    let strategy_names_ubicloud = reader::get(job, "strategy")
        .map(value_text)
        .is_some_and(|strategy| strategy.contains("ubicloud"));
    match (placed.is_empty(), strategy_names_ubicloud) {
        (false, _) => placed,
        (true, true) => vec![text],
        (true, false) => Vec::new(),
    }
}

/// Returns whether a job can run on an Ubicloud runner, however it says so.
fn names_ubicloud(job: &Mapping) -> bool { !runner_texts(job).is_empty() }

/// One lane whose runner can be an Ubicloud one.
#[derive(Debug, PartialEq, Eq)]
pub struct Placed {
    /// The workflow file name.
    pub workflow: String,
    /// The job id.
    pub job: String,
    /// The `timeout-minutes` the job states, or `None` when it states none
    /// or states something other than a whole number.
    pub ceiling: Option<u64>,
}

/// Returns every job whose `runs-on` names an Ubicloud runner, in file and
/// job order, with the ceiling it states.
pub fn placed_jobs(all: &reader::Workflows) -> Vec<Placed> {
    all.iter()
        .flat_map(|(workflow, document)| {
            reader::jobs(document)
                .into_iter()
                .filter(|(_, job)| names_ubicloud(job))
                .map(move |(job, mapping)| Placed {
                    workflow: workflow.clone(),
                    job: job.to_owned(),
                    ceiling: reader::get(mapping, "timeout-minutes").and_then(Value::as_u64),
                })
        })
        .collect()
}

/// Returns the runner text of every place a job can land on Ubicloud, each
/// with its `workflow: job` name; a job placed through several matrix rows
/// appears once per row.
pub fn placement_expressions(all: &reader::Workflows) -> Vec<(String, String)> {
    all.iter()
        .flat_map(|(workflow, document)| {
            reader::jobs(document)
                .into_iter()
                .flat_map(move |(job, mapping)| {
                    runner_texts(mapping)
                        .into_iter()
                        .map(move |text| (format!("{workflow}: {job}"), text))
                })
        })
        .collect()
}
