//! Reads workflows for the CV-005 contract.
//!
//! Every reader here errs towards seeing more. A workflow the contract cannot
//! see is a workflow it passes, so each place GitHub accepts more than one
//! spelling is read in all of them: both file extensions in either case, the
//! `on` key as a string or as the boolean YAML 1.1 makes of it, a trigger
//! written as a scalar, a sequence or a mapping, and a reusable-workflow call
//! however its local path is prefixed.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde_yaml::{Mapping, Value};

/// The directory GitHub reads workflows from, relative to the repository.
const WORKFLOW_PREFIX: &str = ".github/workflows/";

/// Extensions GitHub accepts for a workflow file, compared case-insensitively.
const WORKFLOW_EXTENSIONS: [&str; 2] = ["yml", "yaml"];

/// Every workflow in the repository, keyed by file name.
pub type Workflows = BTreeMap<String, Value>;

/// Returns the repository's workflow directory.
fn workflow_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".github")
        .join("workflows")
}

/// Parses one workflow, refusing a mapping that declares a key twice.
///
/// `serde_yaml` refuses duplicate keys itself, and this is the one place the
/// contract parses, so the refusal cannot be bypassed by a second reader. A
/// parser that kept the last duplicate would let a `runs-on` or an `if:`
/// carry one value in the file and another in the parse.
///
/// # Errors
///
/// Returns an error naming the file when it is not valid YAML or repeats a
/// key within one mapping.
pub fn parse(name: &str, text: &str) -> Result<Value> {
    serde_yaml::from_str(text).with_context(|| format!("parse {name} as YAML"))
}

/// Returns whether `path` names a workflow file, in either extension or case.
fn is_workflow(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
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
/// fails [`parse`], or when no workflow is found at all, since every contract
/// ranging over an empty set would pass.
pub fn workflows() -> Result<Workflows> {
    let directory = workflow_dir();
    let mut found = Workflows::new();
    for entry in
        fs::read_dir(&directory).with_context(|| format!("read {}", directory.display()))?
    {
        let path = entry.context("read a workflow directory entry")?.path();
        if !is_workflow(&path) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("workflow file name should be UTF-8")?
            .to_owned();
        let text = fs::read_to_string(&path).with_context(|| format!("read {name}"))?;
        let parsed = parse(&name, &text)?;
        found.insert(name, parsed);
    }
    ensure!(
        !found.is_empty(),
        "no workflows found under {}",
        directory.display()
    );
    Ok(found)
}

/// Returns the value under `key` in a mapping, if the value is present.
pub fn get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_owned()))
}

/// Returns the event names a workflow declares, in any of the three forms.
///
/// GitHub accepts `on: push`, `on: [push, pull_request]` and the mapping
/// form. A mapping-only reader stringifies the sequence into one key named
/// after the whole list, and the workflow then escapes every pull-request
/// clause. The key itself is read as the string `on` and as the boolean YAML
/// 1.1 resolves a bare `on` to.
pub fn trigger_names(workflow: &Value) -> Vec<String> {
    let Some(root) = workflow.as_mapping() else {
        return Vec::new();
    };
    let Some(on) = get(root, "on").or_else(|| root.get(Value::Bool(true))) else {
        return Vec::new();
    };
    match on {
        Value::String(name) => vec![name.clone()],
        Value::Sequence(names) => names
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Value::Mapping(events) => events
            .keys()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns the configuration of one trigger, when it is written as a mapping.
pub fn trigger<'a>(workflow: &'a Value, event: &str) -> Option<&'a Value> {
    let root = workflow.as_mapping()?;
    get(root, "on")
        .or_else(|| root.get(Value::Bool(true)))?
        .as_mapping()
        .and_then(|events| get(events, event))
}

/// Returns whether a workflow starts on a pull request of either kind.
pub fn starts_on_pull_request(workflow: &Value) -> bool {
    trigger_names(workflow)
        .iter()
        .any(|name| matches!(name.as_str(), "pull_request" | "pull_request_target"))
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

/// Returns every step of every job in a workflow.
pub fn steps(workflow: &Value) -> Vec<&Mapping> {
    jobs(workflow)
        .into_iter()
        .filter_map(|(_, job)| get(job, "steps").and_then(Value::as_sequence))
        .flatten()
        .filter_map(Value::as_mapping)
        .collect()
}

/// Returns a step's `uses` reference, when it has one.
pub fn uses(step: &Mapping) -> Option<&str> { get(step, "uses").and_then(Value::as_str) }

/// What a job-level `uses:` reference names.
#[derive(Debug, PartialEq, Eq)]
pub enum Call<'a> {
    /// A workflow file directly under this repository's workflow directory.
    Local(&'a str),
    /// A local-shaped reference the contract cannot resolve to one file.
    Refused,
    /// A reusable workflow in another repository, or not a workflow call.
    Remote,
}

/// Classifies a job-level `uses` reference.
///
/// Matched by shape rather than by an enumerated list of spellings: a leading
/// `./` or GitHub's documented `$/` is stripped, and what remains is local
/// when it is a path under `.github/workflows/`. A call into another
/// repository carries an owner first and so never matches. A local-shaped
/// reference carrying an `@ref`, or naming a subdirectory, is refused rather
/// than read as remote: it resolves to no file here, so reading it as "not
/// local" would let whatever it runs escape the closure in silence.
pub fn classify_call(reference: &str) -> Call<'_> {
    let path = reference
        .strip_prefix("./")
        .or_else(|| reference.strip_prefix("$/"))
        .unwrap_or(reference);
    let Some(file) = path.strip_prefix(WORKFLOW_PREFIX) else {
        return Call::Remote;
    };
    if file.is_empty() || file.contains('/') || file.contains('@') {
        return Call::Refused;
    }
    Call::Local(file)
}

/// Returns each job's `uses` reference in a workflow, with the job's id.
pub fn job_calls(workflow: &Value) -> Vec<(&str, &str)> {
    jobs(workflow)
        .into_iter()
        .filter_map(|(id, job)| Some((id, get(job, "uses")?.as_str()?)))
        .collect()
}

/// Returns the workflows a pull request can reach, by file name.
///
/// This starts from every workflow triggered by a pull request and follows
/// job-level `uses:` calls into this repository's own workflows until nothing
/// new is reached. A workflow declaring only `workflow_call` never names a
/// pull request, yet it runs with whatever the caller hands it, including the
/// caller's secrets under `secrets: inherit`. Enumerating triggers alone
/// leaves exactly that workflow outside every pull-request clause.
pub fn pull_request_closure(all: &Workflows) -> BTreeSet<String> {
    let mut reached: BTreeSet<String> = all
        .iter()
        .filter(|(_, workflow)| starts_on_pull_request(workflow))
        .map(|(name, _)| name.clone())
        .collect();
    let mut pending: Vec<String> = reached.iter().cloned().collect();
    while let Some(name) = pending.pop() {
        let Some(workflow) = all.get(&name) else {
            continue;
        };
        for (_, reference) in job_calls(workflow) {
            let Call::Local(callee) = classify_call(reference) else {
                continue;
            };
            if all.contains_key(callee) && reached.insert(callee.to_owned()) {
                pending.push(callee.to_owned());
            }
        }
    }
    reached
}

/// Returns each local call, from the named workflows, to a file that is not there.
///
/// The closure can only follow a call to a workflow it has read, so a call
/// to a missing file would otherwise drop out of it in silence, and with it
/// whatever that file would run once it exists. Each entry names the caller
/// and the reference as written.
pub fn missing_callees(all: &Workflows, names: &BTreeSet<String>) -> Vec<String> {
    names
        .iter()
        .filter_map(|name| Some((name, all.get(name)?)))
        .flat_map(|(name, workflow)| {
            job_calls(workflow)
                .into_iter()
                .filter(|(_, reference)| {
                    matches!(classify_call(reference), Call::Local(file) if !all.contains_key(file))
                })
                .map(move |(_, reference)| format!("{name} calls `{reference}`, which is not there"))
        })
        .collect()
}
