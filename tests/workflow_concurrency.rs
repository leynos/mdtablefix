//! Contracts cancelling superseded pull-request runs.
//!
//! Every push to a pull request starts a fresh run of each gate, and the run
//! already in flight is answering a question about a commit nobody will
//! merge. Left alone it holds a runner until it finishes, so the branch pays
//! twice for one answer. A concurrency group keyed on the pull request makes
//! the newer run cancel the older one.
//!
//! Cancellation has to stay conditioned on the event. A literal
//! `cancel-in-progress: true` would also cancel a push to `main`, a schedule,
//! and a dispatch, none of which has a successor waiting: the run that
//! publishes a release, or the one that records coverage, would be killed by
//! whatever landed next. The condition is part of the contract rather than an
//! implementation detail, and `cancellation_is_conditioned_on_the_event`
//! fails on the literal.
//!
//! The group also has to distinguish one pull request from another. A group
//! derived from `github.run_id` is unique per run and so cancels nothing,
//! while a constant group would let one branch cancel another's gates.
//!
//! Only `pull_request` is in scope. A `pull_request_target` workflow runs
//! against the base repository to carry a token, and the one that uses it
//! here automates pull-request housekeeping rather than building; cancelling
//! an auto-merge mid-flight is a hazard with no minutes to win.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_yaml::{Mapping, Value};

/// The exact `cancel-in-progress` value every pull-request workflow carries.
///
/// The comparison is against this one string rather than a substring search,
/// which is what makes a literal `true` fail: it parses as a boolean, not as
/// this.
const CANCEL_EXPRESSION: &str = "${{ github.event_name == 'pull_request' }}";

/// The trigger that puts a workflow in scope.
const PULL_REQUEST: &str = "pull_request";

/// Expressions unique to a single run.
///
/// A group built from one of these can never match another run, so it queues
/// nothing and cancels nothing while reading exactly like a concurrency
/// control.
const RUN_UNIQUE_EXPRESSIONS: [&str; 4] = [
    "github.run_id",
    "github.run_number",
    "github.run_attempt",
    "github.sha",
];

/// Expressions that differ between two pull requests.
///
/// A group naming none of them is shared by every branch, so one pull
/// request's push would cancel another's gates.
const PER_PULL_REQUEST_EXPRESSIONS: [&str; 3] = [
    "github.event.pull_request.number",
    "github.head_ref",
    "github.ref",
];

/// Workflows known to start on `pull_request`.
///
/// Discovery reads the workflow directory, so a new workflow is covered the
/// day it lands. A dynamic list that silently empties would turn every
/// contract here into a vacuous pass, and this names the floor discovery must
/// still reach.
const KNOWN_PULL_REQUEST_WORKFLOWS: [&str; 2] = ["ci.yml", "verus.yml"];

/// Extensions GitHub accepts for a workflow file.
///
/// Scanning only `.yml` would silently exempt a `.yaml` workflow from every
/// contract here.
const WORKFLOW_SUFFIXES: [&str; 2] = ["yml", "yaml"];

/// One workflow file, reduced to what these contracts ask of it.
struct Workflow {
    /// File name within the workflow directory.
    file: String,
    /// Event names declared under `on`.
    triggers: Vec<String>,
    /// The top-level `concurrency` mapping, absent when none is declared.
    concurrency: Option<Mapping>,
}

impl Workflow {
    /// Returns a `concurrency` value rendered as written, or an empty string.
    ///
    /// Rendering rather than interpreting is deliberate: a literal
    /// `cancel-in-progress: true` and the expression that conditions
    /// cancellation on the event are both valid YAML for the same key, and a
    /// reader that coerced the literal to a boolean could not tell them
    /// apart.
    fn setting(&self, key: &str) -> String {
        let Some(mapping) = self.concurrency.as_ref() else {
            return String::new();
        };
        match mapping.get(Value::String(key.to_owned())) {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Bool(flag)) => flag.to_string(),
            Some(Value::Number(number)) => number.to_string(),
            _ => String::new(),
        }
    }
}

/// Returns the repository's workflow directory.
fn workflow_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".github")
        .join("workflows")
}

/// Returns the event names a parsed workflow declares under `on`.
///
/// YAML 1.1 resolves an unquoted `on:` key to the boolean `true`, so a
/// workflow that drops the quotes would otherwise read as declaring no
/// triggers at all and pass every contract vacuously.
fn triggers(document: &Mapping) -> Vec<String> {
    let declared = document
        .get(Value::String("on".to_owned()))
        .or_else(|| document.get(Value::Bool(true)));
    match declared {
        Some(Value::Mapping(mapping)) => mapping
            .keys()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
        Some(Value::Sequence(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
        Some(Value::String(event)) => vec![event.clone()],
        _ => Vec::new(),
    }
}

/// Reads and parses every workflow a pull request can start.
fn pull_request_workflows() -> Result<Vec<Workflow>> {
    let directory = workflow_dir();
    let mut found = Vec::new();
    for entry in fs::read_dir(&directory)
        .with_context(|| format!("read workflow directory {}", directory.display()))?
    {
        let path = entry.context("read a workflow directory entry")?.path();
        let is_workflow = path
            .extension()
            .and_then(|suffix| suffix.to_str())
            .is_some_and(|suffix| WORKFLOW_SUFFIXES.contains(&suffix));
        if !is_workflow {
            continue;
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read workflow {}", path.display()))?;
        let document: Mapping = serde_yaml::from_str(&text)
            .with_context(|| format!("parse workflow {}", path.display()))?;
        let declared = triggers(&document);
        if !declared.iter().any(|event| event == PULL_REQUEST) {
            continue;
        }
        found.push(Workflow {
            file: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned(),
            triggers: declared,
            concurrency: document
                .get(Value::String("concurrency".to_owned()))
                .and_then(Value::as_mapping)
                .cloned(),
        });
    }
    found.sort_by(|left, right| left.file.cmp(&right.file));
    Ok(found)
}

#[test]
fn discovery_still_finds_the_known_pull_request_workflows() -> Result<()> {
    // Every contract below iterates a list built by reading the workflow
    // directory. If that read broke, or the `on` key changed shape, the list
    // would empty and each contract would pass having asserted nothing.
    let discovered = pull_request_workflows()?;
    for expected in KNOWN_PULL_REQUEST_WORKFLOWS {
        assert!(
            discovered.iter().any(|workflow| workflow.file == expected),
            "`{expected}` starts on `{PULL_REQUEST}` but discovery missed it; the contracts below \
             would pass without asserting anything about it"
        );
    }
    Ok(())
}

#[test]
fn every_pull_request_workflow_declares_a_concurrency_group() -> Result<()> {
    // Without a group, every push to the branch leaves its predecessor
    // running to completion on a runner.
    for workflow in pull_request_workflows()? {
        let group = workflow.setting("group");
        assert!(
            !group.trim().is_empty(),
            "`{}` declares `{:?}` and must declare `concurrency.group`; without it a superseded \
             run holds a runner until it finishes",
            workflow.file,
            workflow.triggers
        );
    }
    Ok(())
}

#[test]
fn the_concurrency_group_is_not_unique_to_one_run() -> Result<()> {
    // A group built from the run identifier or the commit SHA matches no
    // other run, so it cancels nothing while reading as a concurrency
    // control.
    for workflow in pull_request_workflows()? {
        let group = workflow.setting("group");
        for expression in RUN_UNIQUE_EXPRESSIONS {
            assert!(
                !group.contains(expression),
                "`{}` builds its concurrency group from `{expression}`, which is unique to one \
                 run; the group would never match a superseded run and would cancel nothing",
                workflow.file
            );
        }
    }
    Ok(())
}

#[test]
fn the_concurrency_group_distinguishes_one_pull_request_from_another() -> Result<()> {
    // A constant group would put every open pull request in one queue, and
    // the first push anywhere would cancel the gates running everywhere else.
    for workflow in pull_request_workflows()? {
        let group = workflow.setting("group");
        assert!(
            PER_PULL_REQUEST_EXPRESSIONS
                .iter()
                .any(|expression| group.contains(expression)),
            "`{}` must key its concurrency group on the pull request, by naming one of \
             {PER_PULL_REQUEST_EXPRESSIONS:?}; a group shared by every branch would cancel \
             unrelated pull requests",
            workflow.file
        );
    }
    Ok(())
}

#[test]
fn cancellation_is_conditioned_on_the_event() -> Result<()> {
    // A literal `true` reads as a stricter setting and is a regression: it
    // would cancel runs that have no successor waiting, such as the release
    // publication on a tag.
    for workflow in pull_request_workflows()? {
        assert_eq!(
            workflow.setting("cancel-in-progress"),
            CANCEL_EXPRESSION,
            "`{}` must set `cancel-in-progress` to `{CANCEL_EXPRESSION}`; a missing value leaves \
             superseded runs in flight, and a literal `true` also cancels pushes, schedules, and \
             dispatches",
            workflow.file
        );
    }
    Ok(())
}
