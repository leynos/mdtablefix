//! Support types and runners for the CLI matrix integration test.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};

use assert_cmd::Command;
use tempfile::tempdir;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum TransformFlag {
    Renumber,
    Breaks,
    Ellipsis,
    Fences,
    Footnotes,
    CodeEmphasis,
    Headings,
}

impl TransformFlag {
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
pub(crate) struct BaseCase {
    pub(crate) id: &'static str,
    pub(crate) fixture: &'static str,
    pub(crate) flags: &'static [TransformFlag],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WrapVariant {
    Wrapped,
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
pub(crate) enum ExecutionMode {
    Stdout,
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

#[derive(Clone)]
pub(crate) struct LogicalCase {
    pub(crate) id: String,
    pub(crate) fixture: &'static str,
    pub(crate) is_wrapped: bool,
    pub(crate) flags: Vec<TransformFlag>,
}

pub(crate) struct PhysicalCase {
    pub(crate) logical: LogicalCase,
    pub(crate) mode: ExecutionMode,
}

pub(crate) struct RunResult {
    pub(crate) output: Output,
    pub(crate) file_content: Vec<u8>,
}

pub(crate) const ALL_FLAGS: &[TransformFlag] = &[
    TransformFlag::Renumber,
    TransformFlag::Breaks,
    TransformFlag::Ellipsis,
    TransformFlag::Fences,
    TransformFlag::Footnotes,
    TransformFlag::CodeEmphasis,
    TransformFlag::Headings,
];

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
    pub(crate) fn snapshot_name(&self) -> String {
        format!("{}_{}", self.logical.id, self.mode.id_part())
    }

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

pub(crate) fn run_physical_case(case: &PhysicalCase) -> RunResult {
    let dir = tempdir().expect("create temporary directory");
    let file_path = dir.path().join("input.md");
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

pub(crate) fn fixture_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("cli-matrix")
        .join(file_name)
}

pub(crate) fn is_case_id(id: &str) -> bool {
    !id.is_empty()
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

pub(crate) fn non_wrap_signature(fixture: &str, flags: &[TransformFlag]) -> String {
    let args = flags
        .iter()
        .map(|flag| flag.as_arg())
        .collect::<Vec<_>>()
        .join(",");
    format!("{fixture}:{args}")
}

pub(crate) fn has_flag(case: &BaseCase, flag: TransformFlag) -> bool { case.flags.contains(&flag) }
