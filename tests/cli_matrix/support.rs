//! Support types and runners for the CLI matrix integration test.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{ExitStatus, Output},
};

use anyhow::{Context as _, Result};
use assert_cmd::Command;
use tempfile::tempdir;

#[path = "cases.rs"]
mod cases;
#[path = "invariants.rs"]
mod invariants;
#[path = "reporting.rs"]
mod reporting;
#[path = "support_tests.rs"]
mod support_tests;

pub(crate) use cases::{ALL_FLAGS, BASE_MATRIX_CASES, STAGED_FILE};
pub(crate) use reporting::{assert_reporting_invariants, check_counts, diff_counts};

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
