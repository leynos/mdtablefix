//! Snapshot tests for the tracing events emitted by in-place rewrites.
//!
//! The events are pinned so that an accidental change to their level, target,
//! message, or field set is caught in review.

use std::cell::RefCell;

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::tempdir;
// Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
use test_macros::traced_test;

use super::replace_file;
use crate::wrap::tracing_snapshot_support::normalise_event_lines;

/// Opens a directory capability for `path`.
fn open_dir(path: &std::path::Path) -> Dir {
    let path = Utf8Path::from_path(path).expect("UTF-8 temporary directory");
    Dir::open_ambient_dir(path, ambient_authority()).expect("open directory capability")
}

#[traced_test]
#[test]
fn snapshots_successful_replacement_events() {
    let dir = tempdir().expect("create temporary directory");
    let directory = open_dir(dir.path());
    let path = Utf8Path::new("sample.md");
    directory.write(path, "old").expect("write fixture");

    replace_file(&directory, path, "replacement").expect("replace file");

    let written = RefCell::new(String::new());
    logs_assert(|lines| {
        written.replace(normalise_event_lines(lines, "temporary file written"));
        (!written.borrow().is_empty())
            .then_some(())
            .ok_or_else(|| "expected temporary file written event".to_string())
    });
    let written = written.into_inner();

    let replaced = RefCell::new(String::new());
    logs_assert(|lines| {
        replaced.replace(normalise_event_lines(lines, "target replaced"));
        (!replaced.borrow().is_empty())
            .then_some(())
            .ok_or_else(|| "expected target replaced event".to_string())
    });
    let replaced = replaced.into_inner();

    insta::with_settings!({prepend_module_to_snapshot => false}, {
        insta::assert_snapshot!("in-place-temporary-file-written-event", written);
        insta::assert_snapshot!("in-place-target-replaced-event", replaced);
    });
}

#[traced_test]
#[test]
fn reports_replacement_failure_event() {
    let dir = tempdir().expect("create temporary directory");
    let directory = open_dir(dir.path());
    let path = Utf8Path::new("target.md");
    directory
        .create_dir(path)
        .expect("create a directory target that cannot be replaced");

    replace_file(&directory, path, "replacement").expect_err("renaming over a directory must fail");

    // The failing stage and the platform-specific error kind are deliberately
    // not pinned; the event's presence and its stable category field are.
    logs_assert(|lines| {
        lines
            .iter()
            .any(|line| line.contains("replacement failed") && line.contains("error_category="))
            .then_some(())
            .ok_or_else(|| "expected replacement failed event".to_string())
    });
}

#[cfg(unix)]
#[traced_test]
#[test]
fn snapshots_declined_symlink_event() {
    let dir = tempdir().expect("create temporary directory");
    let directory = open_dir(dir.path());
    let path = Utf8Path::new("link.md");
    directory
        .write(Utf8Path::new("real.md"), "old")
        .expect("write fixture");
    std::os::unix::fs::symlink("real.md", dir.path().join("link.md")).expect("create symlink");

    replace_file(&directory, path, "replacement").expect_err("symlink must be declined");

    let declined = RefCell::new(String::new());
    logs_assert(|lines| {
        declined.replace(normalise_event_lines(lines, "rewrite declined"));
        (!declined.borrow().is_empty())
            .then_some(())
            .ok_or_else(|| "expected rewrite declined event".to_string())
    });
    let declined = declined.into_inner();

    insta::with_settings!({prepend_module_to_snapshot => false}, {
        insta::assert_snapshot!("in-place-rewrite-declined-event", declined);
    });
}
