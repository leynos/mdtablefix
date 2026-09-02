//! Protects the release workflow's binary provenance and publication contract.

const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release.yml");

#[test]
fn cross_comes_from_a_verified_official_archive() {
    assert!(RELEASE_WORKFLOW.contains("cross-official-v${{ env.CROSS_VERSION }}"));
    assert!(RELEASE_WORKFLOW.contains("cross-rs/cross/releases/download"));
    assert!(
        RELEASE_WORKFLOW
            .contains("642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad")
    );
    assert!(RELEASE_WORKFLOW.contains("sha256sum --check --status"));
    assert!(!RELEASE_WORKFLOW.contains("cargo install cross"));
}

#[test]
fn successful_targets_publish_without_waiting_for_the_matrix() {
    assert!(RELEASE_WORKFLOW.contains("fail-fast: false"));
    assert!(RELEASE_WORKFLOW.contains("Publish this target's release assets"));
    assert!(RELEASE_WORKFLOW.contains("gh release upload"));
    assert!(RELEASE_WORKFLOW.contains("--clobber --repo"));
    assert!(!RELEASE_WORKFLOW.contains("actions/download-artifact"));
}

#[test]
fn manual_dispatch_builds_the_requested_release_tag() {
    assert!(RELEASE_WORKFLOW.contains("workflow_dispatch:"));
    assert!(RELEASE_WORKFLOW.contains("release_tag:"));
    assert!(RELEASE_WORKFLOW.contains("RELEASE_TAG: ${{ inputs.release_tag"));
    assert!(RELEASE_WORKFLOW.contains("ref: ${{ env.RELEASE_TAG }}"));
    let verify_index = RELEASE_WORKFLOW
        .find("Verify release tag matches Cargo.toml")
        .expect("release-tag verification step must exist");
    let create_index = RELEASE_WORKFLOW
        .find("Create the GitHub release when absent")
        .expect("release-creation step must exist");
    assert!(verify_index < create_index);
}
