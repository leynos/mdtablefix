//! The ordering batch: a report list must follow argument order.

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
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

/// Writes `file` into the `directory` capability.
fn write_batch_file(directory: &Dir, file: &BatchFile) {
    let mut content = String::from(if file.ragged { RAGGED } else { CLEAN });
    content.push_str(&"unformatted prose line\n".repeat(file.padding));
    directory.write(file.name, content).expect("write fixture");
}

/// The report line a drifting file must produce.
fn report_line(file: &BatchFile) -> String {
    format!("{} +{RAGGED_INSERTIONS} -{RAGGED_DELETIONS}", file.name)
}

/// Writes every batch file into the `directory` capability and returns their
/// names in argument order.
fn write_batch(directory: &Dir) -> Vec<&'static str> {
    for file in BATCH {
        write_batch_file(directory, file);
    }
    BATCH.iter().map(|file| file.name).collect()
}

/// The stdout a `--check` run over the whole batch must produce: one report
/// line per drifting file, in argument order.
fn expected_reports() -> String {
    let lines: Vec<String> = BATCH
        .iter()
        .filter(|file| file.ragged)
        .map(|file| format!("{}\n", report_line(file)))
        .collect();
    lines.concat()
}

#[test]
fn reports_every_file_in_order() {
    let dir = tempdir().expect("create temporary directory");
    // The one ambient boundary: the capability is opened here, and every
    // fixture is written relative to it. The child process still takes the
    // directory as its working directory.
    let capability = Dir::open_ambient_dir(
        Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8"),
        ambient_authority(),
    )
    .expect("open the temporary directory");
    let names = write_batch(&capability);

    let mut args: Vec<&str> = vec!["--check"];
    args.extend(names.iter().copied());
    let output = run_in(dir.path(), &args);

    assert_eq!(
        status_of(&output),
        1,
        "drifting files must exit 1: {}",
        stderr_of(&output)
    );
    let expected = expected_reports();
    assert_eq!(
        stdout_of(&output),
        expected,
        "reports must follow argument order, not alphabetical, size, or completion order"
    );
}
