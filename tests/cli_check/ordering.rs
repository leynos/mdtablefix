//! The ordering batch: a report list must follow argument order.

use std::{
    fs,
    path::{Path, PathBuf},
};

use tempfile::tempdir;

use super::{
    CLEAN,
    RAGGED,
    RAGGED_DELETIONS,
    RAGGED_INSERTIONS,
    run_in,
    status_of,
    stderr_of,
    stdout_of,
};

/// One file in the ordering batch.
///
/// `padding` lines of prose are appended so the files differ in size along
/// argument order; the padding is never touched by a default rewrite, and the
/// expected counts are unaffected by it.
struct BatchFile {
    name: &'static str,
    ragged: bool,
    padding: usize,
}

/// The ordering batch, in argument order.
///
/// The eight names are in neither alphabetical nor reverse-alphabetical order,
/// and their sizes increase along argument order, so an implementation that
/// returns files in alphabetical order, or in the order the parallel workers
/// happened to finish, is rejected rather than passing by luck.
const BATCH: &[BatchFile] = &[
    BatchFile {
        name: "zulu.md",
        ragged: false,
        padding: 0,
    },
    BatchFile {
        name: "bravo.md",
        ragged: true,
        padding: 5,
    },
    BatchFile {
        name: "yankee.md",
        ragged: false,
        padding: 10,
    },
    BatchFile {
        name: "alpha.md",
        ragged: true,
        padding: 15,
    },
    BatchFile {
        name: "xray.md",
        ragged: false,
        padding: 20,
    },
    BatchFile {
        name: "charlie.md",
        ragged: true,
        padding: 25,
    },
    BatchFile {
        name: "whiskey.md",
        ragged: false,
        padding: 30,
    },
    BatchFile {
        name: "delta.md",
        ragged: true,
        padding: 35,
    },
];

/// Writes `file` into `directory` and returns its path.
fn write_batch_file(directory: &Path, file: &BatchFile) -> PathBuf {
    let mut content = String::from(if file.ragged { RAGGED } else { CLEAN });
    content.push_str(&"unformatted prose line\n".repeat(file.padding));
    let path = directory.join(file.name);
    fs::write(&path, content).expect("write fixture");
    path
}

/// The report line a drifting file must produce.
fn report_line(file: &BatchFile) -> String {
    format!("{} +{RAGGED_INSERTIONS} -{RAGGED_DELETIONS}", file.name)
}

#[test]
fn reports_every_file_in_order() {
    let dir = tempdir().expect("create temporary directory");
    for file in BATCH {
        write_batch_file(dir.path(), file);
    }
    let names: Vec<&str> = BATCH.iter().map(|file| file.name).collect();

    let mut args: Vec<&str> = vec!["--check"];
    args.extend(names.iter().copied());
    let output = run_in(dir.path(), &args);

    assert_eq!(
        status_of(&output),
        1,
        "drifting files must exit 1: {}",
        stderr_of(&output)
    );
    let mut expected = String::new();
    for file in BATCH.iter().filter(|file| file.ragged) {
        expected.push_str(&report_line(file));
        expected.push('\n');
    }
    assert_eq!(
        stdout_of(&output),
        expected,
        "reports must follow argument order, not alphabetical, size, or completion order"
    );
}
