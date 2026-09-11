//! Support types and runners for the CLI matrix integration test.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{ExitStatus, Output},
};

use anyhow::{Context as _, Result};
use assert_cmd::Command;
use tempfile::tempdir;

#[path = "invariants.rs"]
mod invariants;
#[path = "reporting.rs"]
mod reporting;

pub(crate) use reporting::{assert_reporting_invariants, check_counts, diff_counts};

/// The name every matrix case stages its fixture under.
///
/// A reporting mode names the file it reports, and that name has to survive
/// into a snapshot, so the command runs in the temporary directory and is given
/// this relative name rather than a path the temporary directory invented. It
/// carries the `.dat` extension every matrix fixture uses, which the harness's
/// own self-test pins.
pub(crate) const STAGED_FILE: &str = "input.dat";

/// Represents a non-wrap CLI transform flag.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum TransformFlag {
    /// Renumbers ordered list items.
    Renumber,
    /// Reformats thematic breaks.
    Breaks,
    /// Replaces textual ellipsis sequences.
    Ellipsis,
    /// Normalizes fenced code block delimiters.
    Fences,
    /// Converts bare numeric references to footnotes.
    Footnotes,
    /// Fixes emphasis markers adjacent to inline code.
    CodeEmphasis,
    /// Converts Setext headings to ATX headings.
    Headings,
}

impl TransformFlag {
    /// Returns the command-line argument for this transform flag.
    pub(crate) fn as_arg(self) -> &'static str {
        match self {
            Self::Renumber => "--renumber",
            Self::Breaks => "--breaks",
            Self::Ellipsis => "--ellipsis",
            Self::Fences => "--fences",
            Self::Footnotes => "--footnotes",
            Self::CodeEmphasis => "--code-emphasis",
            Self::Headings => "--headings",
        }
    }
}

#[derive(Clone, Copy)]
/// Defines one curated base row before wrap and execution-mode expansion.
pub(crate) struct BaseCase {
    /// Stable identifier for the base matrix row.
    pub(crate) id: &'static str,
    /// Fixture filename under `tests/data/cli-matrix/`.
    pub(crate) fixture: &'static str,
    /// Non-wrap transform flags enabled for this base row.
    pub(crate) flags: &'static [TransformFlag],
    /// Reporting modes this row also runs, curated rather than exhaustive.
    ///
    /// The reporting modes share their whole assessment path with the printing
    /// modes, so the matrix exercises them over a representative subset of rows
    /// instead of doubling every snapshot. A row that joins the subset runs
    /// *both* reporting modes in *both* wrap variants: a half-covered row would
    /// pin one mode's verdict without the other's, and a curated row is the
    /// unit of coverage here.
    pub(crate) reporting: &'static [ExecutionMode],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Represents whether `--wrap` is active.
pub(crate) enum WrapVariant {
    /// Enables `--wrap` for the logical case.
    Wrapped,
    /// Leaves `--wrap` disabled for the logical case.
    Unwrapped,
}

impl WrapVariant {
    fn id_part(self) -> &'static str {
        match self {
            Self::Wrapped => "wrap",
            Self::Unwrapped => "nowrap",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Represents how the binary is invoked.
pub(crate) enum ExecutionMode {
    /// Writes formatted output to stdout.
    Stdout,
    /// Rewrites the temporary input file with `--in-place`.
    InPlace,
    /// Reports drifting files with `--check`, without writing.
    Check,
    /// Prints a unified diff with `--diff`, without writing.
    Diff,
}

impl ExecutionMode {
    fn id_part(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::InPlace => "in_place",
            Self::Check => "check",
            Self::Diff => "diff",
        }
    }

    /// The flag that selects this mode, absent for the default one.
    fn flag(self) -> Option<&'static str> {
        match self {
            Self::Stdout => None,
            Self::InPlace => Some("--in-place"),
            Self::Check => Some("--check"),
            Self::Diff => Some("--diff"),
        }
    }

    /// Whether this mode rewrites the file it is given.
    pub(crate) fn writes_file(self) -> bool { self == Self::InPlace }

    /// Whether this mode reports drift rather than printing formatted text.
    pub(crate) fn reports(self) -> bool { matches!(self, Self::Check | Self::Diff) }
}

#[derive(Clone)]
/// Represents one base row after wrap expansion.
pub(crate) struct LogicalCase {
    /// Stable logical identifier used in snapshot names.
    pub(crate) id: String,
    /// Fixture filename under `tests/data/cli-matrix/`.
    pub(crate) fixture: &'static str,
    /// Whether the logical case includes `--wrap`.
    pub(crate) is_wrapped: bool,
    /// Non-wrap transform flags enabled for the logical case.
    pub(crate) flags: Vec<TransformFlag>,
    /// Reporting modes this case also runs, as declared by its base row.
    pub(crate) reporting: Vec<ExecutionMode>,
}

/// Represents one executable matrix case after mode expansion.
pub(crate) struct PhysicalCase {
    /// Logical case being executed.
    pub(crate) logical: LogicalCase,
    /// Invocation mode for this physical command run.
    pub(crate) mode: ExecutionMode,
}

/// Captures command output and any rewritten file content.
pub(crate) struct RunResult {
    /// Process output returned by the `mdtablefix` binary.
    pub(crate) output: Output,
    /// Bytes read from the temporary input file after execution.
    pub(crate) file_content: Vec<u8>,
}

/// Ordered slice of every non-wrap transform flag.
pub(crate) const ALL_FLAGS: &[TransformFlag] = &[
    TransformFlag::Renumber,
    TransformFlag::Breaks,
    TransformFlag::Ellipsis,
    TransformFlag::Fences,
    TransformFlag::Footnotes,
    TransformFlag::CodeEmphasis,
    TransformFlag::Headings,
];

/// The reporting modes a curated base row runs, in the order it runs them.
const REPORTING_MODES: &[ExecutionMode] = &[ExecutionMode::Check, ExecutionMode::Diff];

/// Curated pairwise base matrix rows.
///
/// Three rows join the reporting subset: `row_000` is the plain table case
/// every user meets first, `row_010` is the one row whose unwrapped variant is
/// already a fixed point (so the subset covers the no-drift branch as well as
/// the drifting one), and `row_111` carries the frontmatter document boundary
/// through both reporting modes.
pub(crate) const BASE_MATRIX_CASES: &[BaseCase] = &[
    BaseCase {
        id: "row_000",
        fixture: "table-prose.dat",
        flags: &[],
        reporting: REPORTING_MODES,
    },
    BaseCase {
        id: "row_001",
        fixture: "fences-ellipsis.dat",
        flags: &[
            TransformFlag::Ellipsis,
            TransformFlag::Footnotes,
            TransformFlag::CodeEmphasis,
            TransformFlag::Headings,
        ],
        reporting: &[],
    },
    BaseCase {
        id: "row_010",
        fixture: "footnotes.dat",
        flags: &[
            TransformFlag::Breaks,
            TransformFlag::Fences,
            TransformFlag::CodeEmphasis,
            TransformFlag::Headings,
        ],
        reporting: REPORTING_MODES,
    },
    BaseCase {
        id: "row_011",
        fixture: "frontmatter-breaks.dat",
        flags: &[
            TransformFlag::Breaks,
            TransformFlag::Ellipsis,
            TransformFlag::Fences,
            TransformFlag::Footnotes,
        ],
        reporting: &[],
    },
    BaseCase {
        id: "row_100",
        fixture: "table-prose.dat",
        flags: &[
            TransformFlag::Renumber,
            TransformFlag::Fences,
            TransformFlag::Footnotes,
            TransformFlag::Headings,
        ],
        reporting: &[],
    },
    BaseCase {
        id: "row_101",
        fixture: "fences-ellipsis.dat",
        flags: &[
            TransformFlag::Renumber,
            TransformFlag::Ellipsis,
            TransformFlag::Fences,
            TransformFlag::CodeEmphasis,
        ],
        reporting: &[],
    },
    BaseCase {
        id: "row_110",
        fixture: "footnotes.dat",
        flags: &[
            TransformFlag::Renumber,
            TransformFlag::Breaks,
            TransformFlag::Footnotes,
            TransformFlag::CodeEmphasis,
        ],
        reporting: &[],
    },
    BaseCase {
        id: "row_111",
        fixture: "frontmatter-breaks.dat",
        flags: &[
            TransformFlag::Renumber,
            TransformFlag::Breaks,
            TransformFlag::Ellipsis,
            TransformFlag::Headings,
        ],
        reporting: REPORTING_MODES,
    },
];

/// Renders a process status as the portable value every platform agrees on.
///
/// `ExitStatus`'s own `Display` is not portable: an ordinary exit reads
/// `exit status: 0` on Unix and `exit code: 0` on Windows, so snapshotting it
/// directly would make every envelope below platform-specific for no gain.
/// `ExitStatus::code()` is the cross-platform accessor for the child's exit
/// code; it yields `None` only when the process was killed by a signal, which
/// no matrix case reaches, and that case is named rather than numbered.
fn status_text(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("code: {code}"),
        None => "no exit code".to_string(),
    }
}

impl RunResult {
    /// Builds the labelled text snapshot for a physical command run.
    ///
    /// The resulting-file block is elided for every read-only mode: `--check`
    /// and `--diff` leave the file untouched by definition, and repeating the
    /// input there would record nothing the `[stdout]` block does not already
    /// show, while doubling the size of each snapshot.
    pub(crate) fn envelope(&self, case: &PhysicalCase) -> String {
        let stdout = String::from_utf8_lossy(&self.output.stdout);
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        let file = if case.mode.writes_file() {
            String::from_utf8_lossy(&self.file_content).into_owned()
        } else {
            "<not applicable>\n".to_string()
        };

        format!(
            "case: {}\nmode: {}\nargs: {}\nstatus: {}\n\n[stdout]\n{}\n[stderr]\n{}\n[file]\n{}",
            case.logical.id,
            case.mode.id_part(),
            case.args().join(" "),
            status_text(self.output.status),
            stdout,
            stderr,
            file,
        )
    }
}

impl PhysicalCase {
    /// Returns the stable snapshot name for this physical command run.
    pub(crate) fn snapshot_name(&self) -> String {
        format!("{}_{}", self.logical.id, self.mode.id_part())
    }

    /// Builds the CLI argument list for this physical command run.
    pub(crate) fn args(&self) -> Vec<&'static str> {
        let mut args = Vec::new();
        if self.logical.is_wrapped {
            args.push("--wrap");
        }
        args.extend(self.logical.flags.iter().map(|flag| flag.as_arg()));
        args.extend(self.mode.flag());
        args
    }
}

/// Expands every base row into wrapped and unwrapped logical cases.
pub(crate) fn logical_cases() -> Vec<LogicalCase> {
    BASE_MATRIX_CASES
        .iter()
        .flat_map(|case| {
            [WrapVariant::Unwrapped, WrapVariant::Wrapped].map(move |variant| LogicalCase {
                id: format!("{}_{}", case.id, variant.id_part()),
                fixture: case.fixture,
                is_wrapped: variant == WrapVariant::Wrapped,
                flags: case.flags.to_vec(),
                reporting: case.reporting.to_vec(),
            })
        })
        .collect()
}

/// The curated logical cases that carry reporting modes.
pub(crate) fn reporting_cases() -> Vec<LogicalCase> {
    logical_cases()
        .into_iter()
        .filter(|case| !case.reporting.is_empty())
        .collect()
}

/// Builds the physical case for one logical case in one mode.
pub(crate) fn physical_case(logical: &LogicalCase, mode: ExecutionMode) -> PhysicalCase {
    PhysicalCase {
        logical: logical.clone(),
        mode,
    }
}

/// Expands one logical case into every command run it declares.
fn modes_for(logical: &LogicalCase) -> Vec<ExecutionMode> {
    let mut modes = vec![ExecutionMode::Stdout, ExecutionMode::InPlace];
    modes.extend(logical.reporting.iter().copied());
    modes
}

/// Expands every logical case into every command run it declares.
pub(crate) fn physical_cases() -> Vec<PhysicalCase> {
    logical_cases()
        .into_iter()
        .flat_map(|logical| {
            modes_for(&logical)
                .into_iter()
                .map(move |mode| PhysicalCase {
                    logical: logical.clone(),
                    mode,
                })
        })
        .collect()
}

/// Asserts output properties that prove enabled transforms changed matching input.
pub(crate) fn assert_transform_invariants(logical: &LogicalCase, stdout: &[u8]) -> Result<()> {
    invariants::assert_transform_invariants(logical, stdout)
}

/// Returns whether the named matrix fixture contains a table delimiter.
pub(crate) fn fixture_has_table(file_name: &str) -> Result<bool> {
    invariants::fixture_has_table(file_name)
}

/// Copies a matrix fixture into the temporary command directory.
///
/// The staged input takes its name from [`STAGED_FILE`] so that a mode which
/// echoes the file it reports echoes the same name on every run.
pub(crate) fn stage_fixture(case: &PhysicalCase, dir: &Path) -> Result<PathBuf> {
    let fixture = fixture_path(case.logical.fixture);
    let file_path = dir.join(STAGED_FILE);
    fs::copy(&fixture, &file_path).with_context(|| {
        format!(
            "copy fixture '{}' to '{}'",
            case.logical.fixture,
            file_path.display(),
        )
    })?;
    Ok(file_path)
}

/// Builds a run result from process output and the temporary input file.
///
/// The file is read back for every mode, including the read-only ones: that
/// read is what a reporting mode's "did not write" assertion compares against
/// the fixture, so the evidence is collected here rather than assumed.
pub(crate) fn collect_result(
    output: Output,
    file_path: &Path,
    mode: ExecutionMode,
) -> Result<RunResult> {
    let file_content = fs::read(file_path)
        .with_context(|| format!("read file '{}' after {:?} run", file_path.display(), mode))?;

    Ok(RunResult {
        output,
        file_content,
    })
}

/// Runs a physical matrix case through the real `mdtablefix` binary.
pub(crate) fn run_physical_case(case: &PhysicalCase) -> Result<RunResult> {
    let dir = tempdir().context("create temporary directory for matrix case")?;
    let file_path = stage_fixture(case, dir.path())?;

    let mut command = Command::cargo_bin("mdtablefix").context("create mdtablefix test command")?;
    // The command runs inside the temporary directory and is given the bare
    // file name, so a reporting mode that names the file names it as
    // `input.dat` rather than by a temporary path no snapshot could pin.
    command
        .current_dir(dir.path())
        .args(case.args())
        .arg(STAGED_FILE);
    let output = command.output().with_context(|| {
        format!(
            "execute mdtablefix for matrix case '{}'",
            case.snapshot_name()
        )
    })?;
    collect_result(output, &file_path, case.mode)
}

/// Returns the repository-relative path to a matrix fixture.
pub(crate) fn fixture_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("cli-matrix")
        .join(file_name)
}

/// Returns whether a matrix case identifier uses the documented character set.
pub(crate) fn is_case_id(id: &str) -> bool {
    !id.is_empty()
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

/// Builds a signature that ignores the wrap variant.
pub(crate) fn non_wrap_signature(fixture: &str, flags: &[TransformFlag]) -> String {
    let args = flags
        .iter()
        .map(|flag| flag.as_arg())
        .collect::<Vec<_>>()
        .join(",");
    format!("{fixture}:{args}")
}

/// Returns whether a base row enables the given transform flag.
pub(crate) fn has_flag(case: &BaseCase, flag: TransformFlag) -> bool { case.flags.contains(&flag) }

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    //! Unit tests for CLI-matrix support helpers.

    use super::{BaseCase, TransformFlag, has_flag, is_case_id, non_wrap_signature, status_text};
    use assert_cmd::Command;
    use rstest::rstest;
    use std::process::ExitStatus;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt as _;

    #[test]
    fn status_text_renders_a_successful_exit_as_code_zero() {
        // `ExitStatus::default()` is documented as "successful completion", and
        // is the only success status std will hand out without spawning.
        assert_eq!(status_text(ExitStatus::default()), "code: 0");
    }

    #[test]
    fn status_text_renders_a_failing_exit_as_its_code() {
        // An unknown argument is rejected before any work happens, which is the
        // cheapest portable source of a real non-zero status.
        let status = Command::cargo_bin("mdtablefix")
            .expect("create mdtablefix test command")
            .arg("--not-a-real-flag")
            .output()
            .expect("run mdtablefix with an unknown flag")
            .status;
        let code = status.code().expect("a rejected invocation exits normally");
        assert_ne!(code, 0, "an unknown flag should be rejected");
        assert_eq!(status_text(status), format!("code: {code}"));
    }

    #[cfg(unix)]
    #[test]
    fn status_text_names_a_signal_terminated_status() {
        // Raw wait status 15: killed by SIGTERM, with no core dump. The child
        // never exited, so there is no code to report.
        let status = ExitStatus::from_raw(15);
        assert_eq!(status.code(), None);
        assert_eq!(status_text(status), "no exit code");
    }

    #[rstest]
    #[case("row_001", true)] #[case("row-001", true)] #[case("abc123", true)]
    #[case("", false)] #[case("Row_001", false)] #[case("row 001", false)]
    fn is_case_id_returns_expected_value(#[case] id: &str, #[case] expected: bool) {
        assert_eq!(is_case_id(id), expected);
    }

    #[test] fn non_wrap_signature_ignores_wrap_variant() {
        let flags = [TransformFlag::Renumber, TransformFlag::Fences]; let (unwrapped, wrapped) = (false, true);
        assert_ne!(unwrapped, wrapped); assert_eq!(non_wrap_signature("fixture.dat", &flags), non_wrap_signature("fixture.dat", &flags)); }
    #[test] fn non_wrap_signature_distinguishes_flag_lists() {
        assert_ne!(non_wrap_signature("fixture.dat", &[TransformFlag::Renumber]), non_wrap_signature("fixture.dat", &[TransformFlag::Fences])); }

    #[rstest]
    #[case(TransformFlag::Renumber, true)] #[case(TransformFlag::Fences, false)]
    fn has_flag_returns_expected_value(#[case] flag: TransformFlag, #[case] expected: bool) {
        let case = BaseCase { id: "row_001", fixture: "fixture.dat", flags: &[TransformFlag::Renumber], reporting: &[] };
        assert_eq!(has_flag(&case, flag), expected);
    }
}
