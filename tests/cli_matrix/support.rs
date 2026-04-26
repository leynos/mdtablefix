//! Support types and runners for the CLI matrix integration test.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};

use assert_cmd::Command;
use tempfile::tempdir;

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

/// Defines one curated base row before wrap and execution-mode expansion.
#[derive(Clone, Copy)]
pub(crate) struct BaseCase {
    /// Stable identifier for the base matrix row.
    pub(crate) id: &'static str,
    /// Fixture filename under `tests/data/cli-matrix/`.
    pub(crate) fixture: &'static str,
    /// Non-wrap transform flags enabled for this base row.
    pub(crate) flags: &'static [TransformFlag],
}

/// Represents whether `--wrap` is active.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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

/// Represents how the binary is invoked.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ExecutionMode {
    /// Writes formatted output to stdout.
    Stdout,
    /// Rewrites the temporary input file with `--in-place`.
    InPlace,
}

impl ExecutionMode {
    fn id_part(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::InPlace => "in_place",
        }
    }
}

/// Represents one base row after wrap expansion.
#[derive(Clone)]
pub(crate) struct LogicalCase {
    /// Stable logical identifier used in snapshot names.
    pub(crate) id: String,
    /// Fixture filename under `tests/data/cli-matrix/`.
    pub(crate) fixture: &'static str,
    /// Whether the logical case includes `--wrap`.
    pub(crate) is_wrapped: bool,
    /// Non-wrap transform flags enabled for the logical case.
    pub(crate) flags: Vec<TransformFlag>,
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

/// Curated pairwise base matrix rows.
pub(crate) const BASE_MATRIX_CASES: &[BaseCase] = &[
    BaseCase {
        id: "row_000",
        fixture: "table-prose.dat",
        flags: &[],
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
    },
];

impl RunResult {
    /// Builds the labelled text snapshot for a physical command run.
    pub(crate) fn envelope(&self, case: &PhysicalCase) -> String {
        let stdout = String::from_utf8_lossy(&self.output.stdout);
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        let file = if case.mode == ExecutionMode::InPlace {
            String::from_utf8_lossy(&self.file_content).into_owned()
        } else {
            "<not applicable>\n".to_string()
        };

        format!(
            "case: {}\nmode: {}\nargs: {}\nstatus: {}\n\n[stdout]\n{}\n[stderr]\n{}\n[file]\n{}",
            case.logical.id,
            case.mode.id_part(),
            case.args().join(" "),
            self.output.status,
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
        if self.mode == ExecutionMode::InPlace {
            args.push("--in-place");
        }
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
            })
        })
        .collect()
}

/// Expands every logical case into stdout and `--in-place` command runs.
pub(crate) fn physical_cases() -> Vec<PhysicalCase> {
    logical_cases()
        .into_iter()
        .flat_map(|logical| {
            [ExecutionMode::Stdout, ExecutionMode::InPlace].map(move |mode| PhysicalCase {
                logical: logical.clone(),
                mode,
            })
        })
        .collect()
}

/// Runs a physical matrix case through the real `mdtablefix` binary.
pub(crate) fn run_physical_case(case: &PhysicalCase) -> RunResult {
    let dir = tempdir().expect("create temporary directory");
    let file_path = dir.path().join(case.logical.fixture);
    fs::copy(fixture_path(case.logical.fixture), &file_path).expect("copy matrix fixture");

    let mut command = Command::cargo_bin("mdtablefix").expect("create mdtablefix command");
    command.args(case.args()).arg(&file_path);
    let output = command.output().expect("run mdtablefix");
    assert!(
        output.status.success(),
        "{} failed with stderr:\n{}",
        case.snapshot_name(),
        String::from_utf8_lossy(&output.stderr),
    );

    let file_content = fs::read(&file_path).expect("read matrix output file");
    RunResult {
        output,
        file_content,
    }
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
