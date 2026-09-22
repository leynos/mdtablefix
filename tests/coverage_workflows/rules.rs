//! The CV-005 judgements, each returning the reasons a workflow fails.
//!
//! An empty vector means the workflow complies. Returning reasons rather than
//! a boolean lets the fixture cases assert *which* clause fired, so a rule
//! that fails for the wrong reason cannot pass as one that works.

use serde_yaml::{Mapping, Value};

use super::{
    reader::{self, get, uses},
    text::{computes_a_secret, rendered, rendered_mapping},
};

/// The action the estate uses to publish coverage to `CodeScene`.
pub const UPLOAD_ACTION: &str = "leynos/shared-actions/.github/actions/upload-codescene-coverage";
/// The action that measures coverage and maintains the ratchet baseline.
pub const COVERAGE_ACTION: &str = "leynos/shared-actions/.github/actions/generate-coverage";
/// The secret a pull-request lane must not receive.
const ACCESS_TOKEN: &str = "CS_ACCESS_TOKEN";
/// The expression that hands a step the secret itself.
///
/// The publisher's placement clauses look for this rather than the bare name,
/// because the upload's own guard names `env.CS_ACCESS_TOKEN` without holding
/// anything: a step whose `env` lost the secret would otherwise still read as
/// receiving it through its `if:`.
const SECRET_REFERENCE: &str = "secrets.CS_ACCESS_TOKEN";
/// The CLI a lane must not reach for directly either.
const COVERAGE_CLI: &str = "cs-coverage";
/// The service host, however it is reached.
const CODESCENE_HOST: &str = "codescene.io";
/// The conjunct that restricts the publisher's upload to the trunk.
const MAIN_REF_GUARD: &str = "github.ref == 'refs/heads/main'";

/// Reads a workflow input as a boolean, accepting the string and native forms.
fn input_is(step: &Mapping, key: &str, expected: bool) -> bool {
    get(step, "with")
        .and_then(Value::as_mapping)
        .and_then(|with| get(with, key))
        .is_some_and(|value| {
            value.as_bool() == Some(expected)
                || value.as_str() == Some(expected.to_string().as_str())
        })
}

/// Returns whether a step runs the shared coverage action.
fn is_coverage(step: &Mapping) -> bool {
    uses(step).is_some_and(|r| r.starts_with(COVERAGE_ACTION))
}

/// Returns whether a step uploads to `CodeScene`, through the action or the CLI.
///
/// `upload` is the action's default mode, so an absent mode is an upload,
/// and `check` is not one: it gates changed lines and publishes nothing.
fn is_upload(step: &Mapping) -> bool {
    let mode = get(step, "with")
        .and_then(Value::as_mapping)
        .and_then(|with| get(with, "mode"))
        .and_then(Value::as_str);
    let action = uses(step).is_some_and(|r| r.starts_with(UPLOAD_ACTION))
        && matches!(mode, None | Some("upload"));
    let cli = get(step, "run")
        .and_then(Value::as_str)
        .is_some_and(runs_cli_upload);
    action || cli
}

/// Returns whether a `run` body invokes `cs-coverage upload`.
///
/// Read as the shell reads it rather than as text: a backslash-newline
/// continuation joins `cs-coverage \` and `upload` into one command, and any
/// run of whitespace separates the words, so a contiguous-text search would
/// miss an upload the shell still performs. A path to the binary counts too.
fn runs_cli_upload(run: &str) -> bool {
    let joined = run.replace("\\\n", " ").replace("\\\r\n", " ");
    let words: Vec<&str> = joined.split_whitespace().collect();
    words.windows(2).any(|pair| {
        (pair[0] == COVERAGE_CLI || pair[0].ends_with(&format!("/{COVERAGE_CLI}")))
            && pair[1] == "upload"
    })
}

/// Returns the reasons a workflow a pull request can reach breaches CV-005.
pub fn pull_request_findings(workflow: &Value) -> Vec<String> {
    let mut findings = Vec::new();
    let text = rendered(workflow);
    if text.contains(ACCESS_TOKEN) {
        findings.push(format!("a pull-request lane receives {ACCESS_TOKEN}"));
    }
    if computes_a_secret(&text) {
        findings.push("a pull-request lane reaches a secret by a computed name".to_owned());
    }
    if text.to_ascii_lowercase().contains(CODESCENE_HOST) {
        findings.push(format!("a pull-request lane contacts {CODESCENE_HOST}"));
    }
    for (id, job) in reader::jobs(workflow) {
        if get(job, "secrets").and_then(Value::as_str) == Some("inherit") {
            findings.push(format!(
                "job {id} forwards every secret with `secrets: inherit`"
            ));
        }
    }
    for (id, reference) in reader::job_calls(workflow) {
        if reader::classify_call(reference) == reader::Call::Refused {
            findings.push(format!(
                "job {id} calls `{reference}`, which resolves to no workflow here"
            ));
        }
    }
    for step in reader::steps(workflow) {
        if uses(step).is_some_and(|r| r.starts_with(UPLOAD_ACTION)) {
            findings.push(format!("a pull-request lane invokes {UPLOAD_ACTION}"));
        }
        if get(step, "run")
            .and_then(Value::as_str)
            .is_some_and(|run| run.contains(COVERAGE_CLI))
        {
            findings.push(format!("a pull-request lane runs {COVERAGE_CLI} directly"));
        }
        if is_coverage(step) && !input_is(step, "with-ratchet", true) {
            findings.push("a pull-request coverage step does not set with-ratchet".to_owned());
        }
        if is_coverage(step) && !input_is(step, "publish-artefact", false) {
            findings.push("a pull-request coverage step publishes its report".to_owned());
        }
    }
    findings
}

/// Returns whether a workflow is triggered by a push restricted to `main`.
///
/// A push with no branch filter is not a main publisher: it fires on every
/// branch, so the baseline it writes would be whichever branch pushed last.
/// A tag filter fails too, since it names no branch at all.
pub fn publishes_from_main(workflow: &Value) -> bool {
    reader::trigger(workflow, "push")
        .and_then(Value::as_mapping)
        .and_then(|push| get(push, "branches"))
        .and_then(Value::as_sequence)
        .is_some_and(|branches| {
            !branches.is_empty()
                && branches
                    .iter()
                    .all(|branch| branch.as_str() == Some("main"))
        })
}

/// Returns the conjuncts of an `if:` condition, or `None` if it has a `||`.
///
/// Quoted literals are blanked before the operators are looked for, so a
/// `||` inside a string does not count and an `&&` inside one does not
/// split. A disjunction anywhere makes every conjunct optional, which is why
/// it is refused rather than parsed: `... && ref == main || dispatch` passes
/// any substring search for the ref and uploads a dispatch from any branch.
pub fn conjuncts(condition: &str) -> Option<Vec<String>> {
    let body = condition.trim();
    let body = body
        .strip_prefix("${{")
        .and_then(|inner| inner.strip_suffix("}}"))
        .unwrap_or(body);
    // Blanked byte for byte, so an offset found here is an offset in `body`
    // even when a literal holds a multi-byte character.
    let mut in_quote = false;
    let mut unquoted = String::with_capacity(body.len());
    for character in body.chars() {
        if character == '\'' {
            in_quote = !in_quote;
            unquoted.push(character);
        } else if in_quote {
            unquoted.extend(std::iter::repeat_n(' ', character.len_utf8()));
        } else {
            unquoted.push(character);
        }
    }
    if unquoted.contains("||") {
        return None;
    }
    let mut parts = Vec::new();
    let mut start = 0;
    for (offset, _) in unquoted.match_indices("&&") {
        parts.push(body[start..offset].to_owned());
        start = offset + 2;
    }
    parts.push(body[start..].to_owned());
    Some(
        parts
            .iter()
            .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect(),
    )
}

/// Returns whether an upload step's condition confines it to `main`.
fn guarded_to_main(step: &Mapping) -> bool {
    get(step, "if")
        .and_then(Value::as_str)
        .and_then(conjuncts)
        .is_some_and(|parts| parts.iter().any(|part| part == MAIN_REF_GUARD))
}

/// Returns the reasons the token reaches somewhere other than the upload.
///
/// "Some step has the token" proves nothing: moving it to the coverage step
/// satisfies that while the upload's own guard goes false and publishing
/// silently stops. So the uploading step must hold it, no other step may, and
/// no wider scope may declare it.
fn token_findings(workflow: &Value) -> Vec<String> {
    let mut findings = Vec::new();
    let root = workflow.as_mapping();
    if root
        .and_then(|root| get(root, "env"))
        .is_some_and(|env| rendered(env).contains(SECRET_REFERENCE))
    {
        findings.push(format!(
            "the publisher declares {ACCESS_TOKEN} for every job"
        ));
    }
    for (id, job) in reader::jobs(workflow) {
        if get(job, "env").is_some_and(|env| rendered(env).contains(SECRET_REFERENCE)) {
            findings.push(format!("job {id} declares {ACCESS_TOKEN} for every step"));
        }
        if forwards_the_token(job) {
            findings.push(format!(
                "job {id} forwards {ACCESS_TOKEN} to a reusable workflow"
            ));
        }
    }
    if computes_a_secret(&rendered(workflow)) {
        findings.push("the publisher reaches a secret by a computed name".to_owned());
    }
    for step in reader::steps(workflow) {
        let holds = rendered_mapping(step).contains(SECRET_REFERENCE);
        if is_upload(step) && !holds {
            findings.push(format!("the upload step does not receive {ACCESS_TOKEN}"));
        }
        if !is_upload(step) && holds {
            findings.push(format!(
                "a step other than the upload receives {ACCESS_TOKEN}"
            ));
        }
    }
    findings
}

/// Returns whether a job calling a reusable workflow hands it the token.
///
/// Such a job has no steps, so the step clauses never see it: the token can
/// travel through its `with:` inputs, a named `secrets:` entry, or
/// `secrets: inherit`, and the called workflow then holds it outside the one
/// upload step the publisher is allowed.
fn forwards_the_token(job: &Mapping) -> bool {
    if get(job, "uses").is_none() {
        return false;
    }
    let inherits = get(job, "secrets").and_then(Value::as_str) == Some("inherit");
    let names_it = ["with", "secrets"]
        .iter()
        .filter_map(|key| get(job, key))
        .any(|value| rendered(value).contains(SECRET_REFERENCE));
    inherits || names_it
}

/// Returns the reasons the publisher's runs could cancel one another.
///
/// A cancelled publisher abandons both its upload and its baseline write, so
/// runs share a group that never cancels the run in progress: a newer push
/// replaces a pending run rather than queueing behind it, and the newest
/// baseline wins. Any `cancel-in-progress` other than an
/// absent key or a literal `false` is refused, an expression included: the
/// question is whether a push to `main` can ever be cancelled, and only the
/// literal answers it without evaluation.
fn concurrency_findings(workflow: &Value) -> Vec<String> {
    let mut findings = Vec::new();
    let group = workflow
        .as_mapping()
        .and_then(|root| get(root, "concurrency"));
    if group.is_none() {
        findings.push("the publisher declares no concurrency group".to_owned());
    }
    let job_groups = reader::jobs(workflow)
        .into_iter()
        .filter_map(|(_, job)| get(job, "concurrency"));
    for concurrency in group.into_iter().chain(job_groups) {
        let cancel = concurrency
            .as_mapping()
            .and_then(|mapping| get(mapping, "cancel-in-progress"));
        if cancel.is_some_and(|value| value.as_bool() != Some(false)) {
            findings.push("the publisher may cancel a run in progress".to_owned());
        }
    }
    findings
}

/// Returns the reasons a main publisher fails to publish what CV-005 requires.
pub fn publisher_findings(workflow: &Value) -> Vec<String> {
    let mut findings = Vec::new();
    let steps = reader::steps(workflow);
    if !steps
        .iter()
        .any(|step| is_coverage(step) && input_is(step, "with-ratchet", true))
    {
        findings.push("the main publisher generates no ratcheted coverage".to_owned());
    }
    let uploads: Vec<_> = steps.iter().filter(|step| is_upload(step)).collect();
    if uploads.is_empty() {
        findings.push("the main publisher uploads nothing to CodeScene".to_owned());
    }
    if uploads.iter().any(|step| !guarded_to_main(step)) {
        findings.push(format!(
            "an upload step is not guarded by `{MAIN_REF_GUARD}`"
        ));
    }
    findings.extend(token_findings(workflow));
    findings.extend(concurrency_findings(workflow));
    findings
}

/// Returns a step's `with` input as written, if present.
fn input<'a>(step: &'a Mapping, key: &str) -> Option<&'a str> {
    get(step, "with")
        .and_then(Value::as_mapping)
        .and_then(|with| get(with, key))
        .and_then(Value::as_str)
}

/// The inputs an upload may pass as its access token, whitespace normalized.
const TOKEN_INPUTS: [&str; 2] = [
    "${{ env.CS_ACCESS_TOKEN }}",
    "${{ secrets.CS_ACCESS_TOKEN }}",
];

/// Returns the reasons the publisher's upload would not send what it measured.
///
/// Kept apart from [`publisher_findings`] because it compares two steps of a
/// complete publisher: each upload must read the file, in the format, that a
/// coverage step writes, or it uploads nothing useful while every other
/// clause passes; and it must pass the token it was given as its
/// `access-token`, or its own guard holds while the action runs
/// unauthenticated.
pub fn wiring_findings(workflow: &Value) -> Vec<String> {
    let steps = reader::steps(workflow);
    let written: Vec<(Option<&str>, Option<&str>)> = steps
        .iter()
        .filter(|step| is_coverage(step))
        .map(|step| (input(step, "output-path"), input(step, "format")))
        .collect();
    let mut findings = Vec::new();
    for upload in steps
        .iter()
        .filter(|step| uses(step).is_some_and(|r| r.starts_with(UPLOAD_ACTION)))
    {
        let read = (input(upload, "path"), input(upload, "format"));
        if !written.contains(&read) {
            findings.push(format!(
                "the upload reads {read:?}, which no coverage step writes; written: {written:?}"
            ));
        }
        let token = input(upload, "access-token")
            .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "));
        if !token
            .as_deref()
            .is_some_and(|value| TOKEN_INPUTS.contains(&value))
        {
            findings.push(format!(
                "the upload's access-token is {token:?}, not the token it was given"
            ));
        }
    }
    findings
}
