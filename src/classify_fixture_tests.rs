//! Fixture-wide compatibility oracle for structural line classification.

use std::{collections::VecDeque, error::Error, fmt::Write as _};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::{ClassifyCtx, LineClass, classify_line, classify_line_with_body};
use crate::classify_kernel::OpenFence;

#[path = "classify_reference_tests.rs"]
mod reference;

/// One fixture read through a directory capability rooted at `tests/data`.
///
/// For example, the entry for `nested/example.md` stores that relative path
/// and the file's contents.
struct Fixture {
    path: Utf8PathBuf,
    contents: String,
}

/// Reads every fixture below `tests/data` in deterministic path order.
///
/// For example, if the relative paths are `a.md` and `nested/b.md`, the
/// returned vector lists them in that order.
fn fixtures() -> Result<Vec<Fixture>, Box<dyn Error>> {
    let root = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let directory = Dir::open_ambient_dir(&root, ambient_authority())?;
    let mut pending = VecDeque::from([(directory, Utf8PathBuf::new())]);
    let mut fixtures = Vec::new();

    while let Some((current, prefix)) = pending.pop_front() {
        for candidate in current.entries()? {
            let entry = candidate?;
            let name = entry.file_name()?;
            let path = prefix.join(&name);
            if entry.file_type()?.is_dir() {
                pending.push_back((current.open_dir(&name)?, path));
            } else {
                fixtures.push(Fixture {
                    path,
                    contents: current.read_to_string(&name)?,
                });
            }
        }
    }
    fixtures.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    Ok(fixtures)
}

/// Returns the scanner prefix used when comparing adjacent Setext lines.
///
/// For example, `structural_prefix("  > title")` returns `"  > "`.
fn structural_prefix(line: &str) -> &str {
    let classified = classify_line_with_body(line, &ClassifyCtx::default());
    &line[..line.len() - classified.body.len()]
}

/// Extracts the opening marker represented by a classified fence line.
///
/// For example, ``opening_fence("```rust")`` returns a backtick marker
/// with length three.
fn opening_fence(line: &str) -> Option<OpenFence> {
    let classified = classify_line_with_body(line, &ClassifyCtx::default());
    // Quote-prefix scanning retains content indentation in the structural body.
    // Fence runs begin after that indentation, not at the first body scalar.
    let body = classified.body.trim_matches([' ', '\t']);
    let marker = body.chars().next()?;
    let marker_len = body
        .chars()
        .take_while(|candidate| *candidate == marker)
        .count();
    Some(OpenFence { marker, marker_len })
}

/// Stable FNV-1a hash keeps the oracle tied to the exact fixture line.
///
/// For example, `line_hash("a")` returns `0xaf63dc4c8601ec8c`.
fn line_hash(line: &str) -> u64 {
    line.as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

/// Renders fixture paths independently of the host path separator.
///
/// For example, `snapshot_path(Utf8Path::new(r"nested\fixture.txt"))`
/// returns `"nested/fixture.txt"`.
fn snapshot_path(path: &Utf8Path) -> String { path.as_str().replace('\\', "/") }

/// Actual scanner state, intentionally separate from the regex reference state.
#[derive(Default)]
struct ProductionState {
    open_fence: Option<OpenFence>,
    previous: Option<LineClass>,
    previous_prefix: String,
}

impl ProductionState {
    /// Carries actual scanner results; `title` followed by `---` is Setext.
    fn observe(&mut self, line: &str) -> LineClass {
        let prefix = structural_prefix(line);
        let context = self.open_fence.map_or_else(
            || {
                self.previous.map_or_else(ClassifyCtx::default, |class| {
                    ClassifyCtx::following(class, prefix == self.previous_prefix)
                })
            },
            ClassifyCtx::in_fence,
        );
        let class = classify_line(line, &context);
        if class == LineClass::FenceMarker {
            self.open_fence = if self.open_fence.is_some() {
                None
            } else {
                opening_fence(line)
            };
        }
        self.previous = Some(class);
        self.previous_prefix = prefix.to_owned();
        class
    }
}

/// Classifies a complete fixture while carrying both independent state machines.
///
/// For example, a fixture containing `Title` followed by `---` appends two
/// oracle rows with classes `ParagraphText` and `SetextUnderline`, after the
/// independent reference agrees with the production state machine.
fn classify_fixture(fixture: &Fixture, oracle: &mut String) {
    let mut production = ProductionState::default();
    let mut reference = reference::ReferenceState::default();

    for (line_index, line) in fixture.contents.lines().enumerate() {
        let class = production.observe(line);
        let reference_class = reference.observe(line);
        assert_eq!(
            class,
            reference_class,
            "independent reference differs at {}:{}: {line:?}",
            snapshot_path(&fixture.path),
            line_index + 1,
        );
        writeln!(
            oracle,
            "{}\t{}\t{:016x}\t{class:?}",
            snapshot_path(&fixture.path),
            line_index + 1,
            line_hash(line),
        )
        .expect("writing to a String is infallible");
    }
}

#[test]
fn snapshot_paths_use_forward_slashes() {
    assert_eq!(
        snapshot_path(Utf8Path::new(r"nested\fixture.txt")),
        "nested/fixture.txt"
    );
}

/// Every repository fixture retains its checked-in per-line classification.
///
/// An empty fixture is still traversed but contributes no oracle row because
/// it contains no source line to classify.
#[test]
fn fixture_lines_match_the_classification_oracle() -> Result<(), Box<dyn Error>> {
    let fixtures = fixtures()?;
    assert!(!fixtures.is_empty(), "tests/data must contain fixtures");
    let mut oracle = String::new();
    for fixture in &fixtures {
        classify_fixture(fixture, &mut oracle);
    }

    insta::assert_snapshot!("fixture_line_classification", oracle);
    Ok(())
}
