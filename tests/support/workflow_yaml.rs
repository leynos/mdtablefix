//! Accessors over the parsed release workflow, shared by its contract tests.
//!
//! `tests/release_workflow.rs` states what the workflow must contain; this
//! module only reads it. Each accessor names the key it could not find, so a
//! renamed job or step fails with the name the contract was looking for.

use anyhow::{Context, Result};
use serde_yaml::{Mapping, Value};

const RELEASE_WORKFLOW: &str = include_str!("../../.github/workflows/release.yml");

/// Parses the release workflow bundled into the test binary.
pub fn parse_workflow() -> Result<Value> {
    serde_yaml::from_str(RELEASE_WORKFLOW).context("parse release workflow YAML")
}

/// Views `value` as a mapping, naming `description` on failure.
pub fn as_mapping<'a>(value: &'a Value, description: &str) -> Result<&'a Mapping> {
    value
        .as_mapping()
        .with_context(|| format!("{description} should be a mapping"))
}

/// Looks up `key` in `mapping`, failing when it is absent.
pub fn get<'a>(mapping: &'a Mapping, key: &str) -> Result<&'a Value> {
    mapping
        .get(Value::String(key.to_owned()))
        .with_context(|| format!("mapping should define {key}"))
}

/// Returns the job called `name` from `workflow`.
pub fn job<'a>(workflow: &'a Value, name: &str) -> Result<&'a Mapping> {
    let root = as_mapping(workflow, "workflow")?;
    let jobs = as_mapping(get(root, "jobs")?, "jobs")?;
    as_mapping(get(jobs, name)?, name)
}

/// Returns the steps of `job`.
pub fn steps(job: &Mapping) -> Result<&[Value]> {
    get(job, "steps")?
        .as_sequence()
        .map(Vec::as_slice)
        .context("job steps should be a sequence")
}

/// Views `step` as a mapping, naming `description` on failure.
pub fn step_mapping<'a>(step: &'a Value, description: &str) -> Result<&'a Mapping> {
    as_mapping(step, description)
}

/// Returns the step of `steps` named `name`.
pub fn named_step<'a>(steps: &'a [Value], name: &str) -> Result<&'a Mapping> {
    steps
        .iter()
        .find_map(|step| {
            let mapping = step.as_mapping()?;
            (get_string(mapping, "name") == Some(name)).then_some(mapping)
        })
        .with_context(|| format!("job should define the {name} step"))
}

/// Returns the position of the step of `steps` named `name`.
pub fn named_step_index(steps: &[Value], name: &str) -> Result<usize> {
    steps
        .iter()
        .position(|step| {
            step.as_mapping()
                .and_then(|mapping| get_string(mapping, "name"))
                == Some(name)
        })
        .with_context(|| format!("job should define the {name} step"))
}

/// Returns the string under `key`, if the key exists and holds one.
pub fn get_string<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a str> {
    mapping
        .get(Value::String(key.to_owned()))
        .and_then(Value::as_str)
}

/// Returns the string under `group.key`, if both exist.
pub fn nested_string<'a>(mapping: &'a Mapping, group: &str, key: &str) -> Option<&'a str> {
    mapping
        .get(Value::String(group.to_owned()))
        .and_then(Value::as_mapping)
        .and_then(|nested| get_string(nested, key))
}

/// Returns the `run` command of `step`.
pub fn command(step: &Mapping) -> Result<&str> {
    get_string(step, "run").context("step should define a run command")
}

/// Checks `fragments` occur in `command` in the given order.
pub fn assert_fragments_in_order(command: &str, fragments: &[&str]) -> Result<()> {
    let mut search_start = 0;
    for fragment in fragments {
        let relative_index = command[search_start..]
            .find(fragment)
            .with_context(|| format!("command should contain {fragment:?} in order"))?;
        search_start += relative_index + fragment.len();
    }
    Ok(())
}
