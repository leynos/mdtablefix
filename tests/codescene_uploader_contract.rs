//! What the `CodeScene` uploader may no longer be asked for.
//!
//! At the pinned revision the shared uploader's committed `cli-manifest.json`
//! is the trust anchor for the `CodeScene` CLI archive, and the action *rejects*
//! a non-empty `installer-checksum` with a hard failure rather than ignoring
//! it. A workflow that still passes the repository variable therefore breaks
//! its `CodeScene` step the moment that variable holds anything, and the
//! variable can only ever repeat the manifest's own digest.
//!
//! Four concerns are asserted, each in its own test so a failure names the
//! defect rather than a bundle. Every one of them ranges over a collection
//! whose contents are checked first: a contract over an empty collection is
//! satisfied by deleting the thing it guards, so deleting the workflow
//! directory or the upload step fails these tests rather than passing them.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};

/// The one approved revision of the shared uploader.
///
/// Asserted as an allowlist rather than as a floor. Ordering two commit SHAs
/// cannot be computed from a checkout, so naming the approved revision is what
/// keeps the contract hermetic; it fails closed on any other value, including
/// a tag or a branch name.
const APPROVED_PIN: &str = "a5765019912a8ab6882b12db049c7cde635f3a85";

/// The uploader reference, without its revision. The `@` separator is part of
/// the marker so a differently owned action whose path merely starts with the
/// same text cannot match.
const UPLOADER_MARKER: &str = "leynos/shared-actions/.github/actions/upload-codescene-coverage@";

/// The deprecated input. It carried the SHA-256 of an installer script the
/// action no longer downloads.
const DEPRECATED_INPUT: &str = "installer-checksum";

/// The repository variable whose only consumer was that input.
const DEPRECATED_VARIABLE: &str = "CODESCENE_CLI_SHA256";

/// The `workflow_dispatch` that hashed the installer script and wrote the
/// variable back through the API, named without an extension. Other
/// repositories in the estate carry it; this contract keeps it from arriving
/// here under either GitHub extension.
const REFRESH_WORKFLOW_STEM: &str = "get-codescene-sha";

/// The extensions GitHub accepts for a workflow document. The reader and the
/// absence clause share this one list, so neither can range over a narrower
/// set than the platform actually runs.
const WORKFLOW_EXTENSIONS: [&str; 2] = ["yml", "yaml"];

fn workflow_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".github")
        .join("workflows")
}

/// Every workflow's file name and source text, in file-name order.
///
/// Both GitHub extensions are read: a workflow written with the other one
/// would otherwise escape every contract here without failing anything.
fn workflow_sources() -> Result<Vec<(String, String)>> {
    let directory = workflow_directory();
    let entries = fs::read_dir(&directory)
        .with_context(|| format!("{} could not be read", directory.display()))?;
    let mut sources = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("could not read an entry in {}", directory.display()))?
            .path();
        let is_workflow = path.extension().is_some_and(|extension| {
            WORKFLOW_EXTENSIONS
                .iter()
                .any(|accepted| extension.eq_ignore_ascii_case(accepted))
        });
        if !is_workflow {
            continue;
        }
        let name = path
            .file_name()
            .with_context(|| format!("{} has no file name", path.display()))?
            .to_string_lossy()
            .into_owned();
        let source = fs::read_to_string(&path)
            .with_context(|| format!("{} could not be read", path.display()))?;
        sources.push((name, source));
    }
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    ensure!(
        !sources.is_empty(),
        "no workflow was found under {}, so every contract in this file would pass having read \
         nothing",
        directory.display()
    );
    Ok(sources)
}

/// The revision of every uploader reference, paired with the workflow naming it.
///
/// Splitting on the marker rather than indexing past it keeps the reader off
/// byte offsets: a workflow is arbitrary UTF-8, and a slice taken at a
/// computed offset would panic on a multi-byte character rather than fail the
/// contract it was meant to check.
fn uploader_references() -> Result<Vec<(String, String)>> {
    let sources = workflow_sources()?;
    Ok(sources
        .into_iter()
        .flat_map(|(name, source)| {
            source
                .split(UPLOADER_MARKER)
                .skip(1)
                .map(|tail| {
                    let revision: String = tail
                        .chars()
                        .take_while(|character| !character.is_whitespace())
                        .collect();
                    (name.clone(), revision)
                })
                .collect::<Vec<_>>()
        })
        .collect())
}

/// The workflows whose source contains ``needle``, in file-name order.
///
/// Shared by the two absence contracts below. They assert different things and
/// fail apart, but the search itself is one operation, and writing it twice
/// would leave two readers to keep in step.
fn workflows_containing(needle: &str) -> Result<Vec<String>> {
    let sources = workflow_sources()?;
    Ok(sources
        .into_iter()
        .filter(|(_, source)| source.contains(needle))
        .map(|(name, _)| name)
        .collect())
}

/// The uploader rejects a non-empty value, so no workflow may pass it.
///
/// This is not tidying. The action fails its own input validation on a
/// non-empty value, so the coverage upload stops working the moment the
/// variable behind the input holds anything.
#[test]
fn no_workflow_passes_the_deprecated_installer_checksum() -> Result<()> {
    let offenders = workflows_containing(DEPRECATED_INPUT)?;
    ensure!(
        offenders.is_empty(),
        "{DEPRECATED_INPUT} is deprecated and rejected by the uploader at {APPROVED_PIN}; remove \
         it from {offenders:?}"
    );
    Ok(())
}

/// The variable existed only to feed the rejected input, so it must go.
///
/// Held apart from the input contract because the two regress apart: an `env`
/// line or a guard can name the variable in a workflow that passes no input at
/// all, and such a reference is what a later reader would take as evidence
/// that the variable is still wanted.
#[test]
fn no_workflow_references_the_deprecated_checksum_variable() -> Result<()> {
    let offenders = workflows_containing(DEPRECATED_VARIABLE)?;
    ensure!(
        offenders.is_empty(),
        "{DEPRECATED_VARIABLE} fed {DEPRECATED_INPUT} and has no remaining consumer; remove it \
         from {offenders:?}"
    );
    Ok(())
}

/// One approved revision, so a stale pin cannot reintroduce the input.
///
/// The references are checked for content before they are checked for
/// compliance. Deleting the `CodeScene` steps would otherwise satisfy this
/// contract instead of failing it, and this repository uploads its coverage
/// through that action.
#[test]
fn every_uploader_reference_is_pinned_to_the_approved_revision() -> Result<()> {
    let references = uploader_references()?;
    ensure!(
        !references.is_empty(),
        "no upload-codescene-coverage reference was found, so the pin assertion would pass \
         vacuously; this repository is expected to call the uploader"
    );
    let wrong: Vec<&(String, String)> = references
        .iter()
        .filter(|(_, revision)| revision != APPROVED_PIN)
        .collect();
    ensure!(
        wrong.is_empty(),
        "every upload-codescene-coverage reference must be pinned to {APPROVED_PIN}; found \
         {wrong:?}"
    );
    Ok(())
}

/// Nothing reads the variable it would write, so it is dead code here.
///
/// Asserted against the filesystem rather than the parsed workflows: a
/// dispatch-only workflow appears in no job or step list another contract
/// reads, so its absence is the only property that can be stated.
///
/// Both extensions are checked. A placeholder of that name which references
/// no variable is exactly the shape this clause exists to catch, and under
/// one extension only it would have passed.
#[test]
fn the_checksum_refresh_workflow_is_absent() {
    let dir = workflow_directory();
    // Both extensions, aligned with the reader above. Checking only `.yml`
    // would let a `.yaml` placeholder that names no variable satisfy this
    // clause, which is precisely the shape the clause exists to catch. A real
    // refresh workflow under either extension also fails the variable clause,
    // because it names the variable, but this one must not lean on that.
    let present: Vec<String> = WORKFLOW_EXTENSIONS
        .iter()
        .map(|extension| format!("{REFRESH_WORKFLOW_STEM}.{extension}"))
        .filter(|name| dir.join(name).exists())
        .collect();
    assert!(
        present.is_empty(),
        "{present:?} maintains {DEPRECATED_VARIABLE}, which no workflow reads; delete it rather \
         than keeping a dispatch that writes an unread repository variable"
    );
}
