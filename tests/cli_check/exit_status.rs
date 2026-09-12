//! The exit-status contract across modes, drift, and error.

use std::fs;

use tempfile::tempdir;

use super::{CLEAN, RAGGED, run_in, status_of, stderr_of};

/// How the files of one exit-status case are shaped.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Files {
    /// Every file is already formatted.
    Clean,
    /// Some files drift; the rest are clean.
    SomeDrift,
    /// Every file drifts.
    AllDrift,
}

/// The modes the command line exposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliMode {
    /// No mode flag: formatted output goes to standard output.
    Print,
    /// `--in-place`.
    InPlace,
    /// `--check`.
    Check,
    /// `--diff`.
    Diff,
}

impl CliMode {
    /// The arguments selecting this mode.
    fn args(self) -> &'static [&'static str] {
        match self {
            Self::Print => &[],
            Self::InPlace => &["--in-place"],
            Self::Check => &["--check"],
            Self::Diff => &["--diff"],
        }
    }

    /// Whether drift in this mode is reported through the exit status.
    fn reports(self) -> bool { matches!(self, Self::Check | Self::Diff) }

    /// The status this mode must yield for the given observations.
    ///
    /// An error outranks drift in every mode, drift is a status only under the
    /// two reporting modes, and a successful `--in-place` over drifting files
    /// succeeds.
    fn expected_status(self, files: Files, with_error: bool) -> i32 {
        if with_error {
            2
        } else {
            i32::from(files != Files::Clean && self.reports())
        }
    }
}

#[test]
fn exit_status_matrix() {
    let shapes = [
        ("all_clean", Files::Clean),
        ("some_drift", Files::SomeDrift),
        ("all_drift", Files::AllDrift),
    ];
    let modes = [
        CliMode::Print,
        CliMode::InPlace,
        CliMode::Check,
        CliMode::Diff,
    ];

    for (shape_name, files) in shapes {
        for mode in modes {
            for with_error in [false, true] {
                let dir = tempdir().expect("create temporary directory");
                let mut names = Vec::new();
                for index in 0..2 {
                    let ragged = match files {
                        Files::Clean => false,
                        Files::SomeDrift => index == 0,
                        Files::AllDrift => true,
                    };
                    let name = format!("file{index}.md");
                    fs::write(dir.path().join(&name), if ragged { RAGGED } else { CLEAN })
                        .expect("write fixture");
                    names.push(name);
                }
                if with_error {
                    names.push(String::from("missing.md"));
                }

                let mut args: Vec<&str> = mode.args().to_vec();
                args.extend(names.iter().map(String::as_str));
                let output = run_in(dir.path(), &args);

                assert_eq!(
                    status_of(&output),
                    mode.expected_status(files, with_error),
                    "{shape_name} under {mode:?} (with_error: {with_error}) exited {}, stderr: {}",
                    status_of(&output),
                    stderr_of(&output)
                );
            }
        }
    }
}

/// A mode flag demands an input source, and at most one mode flag is accepted.
///
/// The parser's half of the exit-status contract: because the `mode` group
/// requires the `inputs` group, a mode flag that reaches the driver always has
/// files to act on, so no mode has to ask whether an empty file list meant
/// standard input. The rejections are exit `2`, like every other operational
/// failure; a mode flag alone would otherwise be silently ignored while the
/// run fell through to standard input.
#[test]
fn mode_flags_require_an_input_source() {
    let dir = tempdir().expect("create temporary directory");
    fs::write(dir.path().join("clean.md"), CLEAN).expect("write fixture");
    fs::write(dir.path().join("also.md"), CLEAN).expect("write fixture");

    for mode in [CliMode::InPlace, CliMode::Check, CliMode::Diff] {
        // Two files, not one: `files` is a multi-value positional, so a group
        // that admitted only a single use of it would fail right here.
        let mut with_inputs: Vec<&str> = mode.args().to_vec();
        with_inputs.extend(["clean.md", "also.md"]);
        assert_eq!(
            status_of(&run_in(dir.path(), &with_inputs)),
            mode.expected_status(Files::Clean, false),
            "{mode:?} over named files must be accepted"
        );

        let without_input = mode.args().to_vec();
        let output = run_in(dir.path(), &without_input);
        assert_eq!(
            status_of(&output),
            2,
            "{mode:?} without an input source must be refused, stderr: {}",
            stderr_of(&output)
        );
    }

    let conflicting = ["--check", "--diff", "clean.md"];
    let output = run_in(dir.path(), &conflicting);
    assert_eq!(
        status_of(&output),
        2,
        "two mode flags must conflict, stderr: {}",
        stderr_of(&output)
    );

    // Naming a file with no mode flag still prints it: the historical contract,
    // and the reason `inputs` is a group of its own rather than a requirement
    // the mode flags impose on `files` directly.
    assert_eq!(
        status_of(&run_in(dir.path(), &["clean.md"])),
        0,
        "naming a file with no mode flag must still print it"
    );
}
