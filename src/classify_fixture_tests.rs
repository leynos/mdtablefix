//! Fixture-wide compatibility oracle for structural line classification.

use std::{collections::VecDeque, error::Error, fmt::Write as _};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::{ClassifyCtx, LineClass, classify_line, classify_line_with_body};
use crate::classify_kernel::OpenFence;

/// One fixture read through a directory capability rooted at `tests/data`.
struct Fixture {
    path: Utf8PathBuf,
    contents: String,
}

/// Reads every fixture below `tests/data` in deterministic path order.
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
fn structural_prefix(line: &str) -> &str {
    let classified = classify_line_with_body(line, &ClassifyCtx::default());
    &line[..line.len() - classified.body.len()]
}

/// Extracts the opening marker represented by a classified fence line.
fn opening_fence(line: &str) -> Option<OpenFence> {
    let classified = classify_line_with_body(line, &ClassifyCtx::default());
    let marker = classified.body.chars().next()?;
    let marker_len = classified
        .body
        .chars()
        .take_while(|candidate| *candidate == marker)
        .count();
    Some(OpenFence { marker, marker_len })
}

/// Stable FNV-1a hash keeps the oracle tied to the exact fixture line.
fn line_hash(line: &str) -> u64 {
    line.as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

/// Classifies a complete fixture while carrying fence and preceding-line state.
fn classify_fixture(fixture: &Fixture, oracle: &mut String) {
    let mut open_fence = None;
    let mut previous = None;
    let mut previous_prefix = String::new();

    for (line_index, line) in fixture.contents.lines().enumerate() {
        let prefix = structural_prefix(line);
        let context = open_fence.map_or_else(
            || {
                previous.map_or_else(ClassifyCtx::default, |class| {
                    ClassifyCtx::following(class, prefix == previous_prefix)
                })
            },
            ClassifyCtx::in_fence,
        );
        let class = classify_line(line, &context);
        writeln!(
            oracle,
            "{}\t{}\t{:016x}\t{class:?}",
            fixture.path,
            line_index + 1,
            line_hash(line),
        )
        .expect("writing to a String is infallible");

        if class == LineClass::FenceMarker {
            if open_fence.is_some() {
                open_fence = None;
            } else {
                open_fence = opening_fence(line);
            }
        }
        previous = Some(class);
        previous_prefix.clear();
        previous_prefix.push_str(prefix);
    }
}

/// Every repository fixture retains its checked-in per-line classification.
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
