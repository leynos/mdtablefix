//! Reads workflows for the runner-placement contract.
//!
//! `serde_yaml` refuses a mapping that repeats a key, so a `runs-on` or a
//! `timeout-minutes` cannot carry one value in the file and another in the
//! parse. Both file extensions GitHub accepts are read, in either case.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, camino::Utf8Path},
};
use serde_yaml::{Mapping, Value};

/// The directory GitHub reads workflows from, relative to the repository.
const WORKFLOW_PREFIX: &str = ".github/workflows/";

/// Extensions GitHub accepts for a workflow file, compared case-insensitively.
const WORKFLOW_EXTENSIONS: [&str; 2] = ["yml", "yaml"];

/// Every workflow in the repository, keyed by file name.
pub type Workflows = BTreeMap<String, Value>;

/// Parses one workflow, refusing a mapping that declares a key twice.
///
/// # Errors
///
/// Returns an error naming the file when it is not valid YAML or repeats a key
/// within one mapping.
pub fn parse(name: &str, text: &str) -> Result<Value> {
    serde_yaml::from_str(text).with_context(|| format!("parse {name} as YAML"))
}

/// Returns whether `name` is a workflow file, in either extension or case.
fn is_workflow(name: &str) -> bool {
    Utf8Path::new(name).extension().is_some_and(|extension| {
        WORKFLOW_EXTENSIONS
            .iter()
            .any(|accepted| extension.eq_ignore_ascii_case(accepted))
    })
}

/// Reads every workflow in `.github/workflows`.
///
/// # Errors
///
/// Returns an error when the directory cannot be read, when any workflow
/// fails [`parse`], or when no workflow is found at all, since a contract
/// ranging over an empty set would pass.
pub fn workflows() -> Result<Workflows> {
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the repository root")?;
    let directory = root
        .open_dir(WORKFLOW_PREFIX)
        .with_context(|| format!("open {WORKFLOW_PREFIX}"))?;
    let mut found = Workflows::new();
    for entry in directory
        .entries()
        .with_context(|| format!("read {WORKFLOW_PREFIX}"))?
    {
        let name = entry
            .context("read a workflow directory entry")?
            .file_name()
            .context("workflow file name should be UTF-8")?;
        if !is_workflow(&name) {
            continue;
        }
        let text = directory
            .read_to_string(&name)
            .with_context(|| format!("read {name}"))?;
        let parsed = parse(&name, &text)?;
        found.insert(name, parsed);
    }
    ensure!(
        !found.is_empty(),
        "no workflows found under {WORKFLOW_PREFIX}"
    );
    Ok(found)
}

/// Returns the value under `key` in a mapping, if the value is present.
pub fn get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_owned()))
}

/// Returns every job of a workflow as `(job id, job mapping)`.
pub fn jobs(workflow: &Value) -> Vec<(&str, &Mapping)> {
    workflow
        .as_mapping()
        .and_then(|root| get(root, "jobs"))
        .and_then(Value::as_mapping)
        .map(|jobs| {
            jobs.iter()
                .filter_map(|(id, job)| Some((id.as_str()?, job.as_mapping()?)))
                .collect()
        })
        .unwrap_or_default()
}
