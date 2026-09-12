//! The matrix catalogue: the transform flags, the curated rows, and the name
//! every case stages its fixture under.
//!
//! This is measured data rather than harness logic, and it lives apart from the
//! harness for the same reason `tests/check_prediction/corpus.rs` lives apart
//! from its test: `AGENTS.md` takes large inline data out of the file that
//! drives it. `super` re-exports every item, so a caller still names one module.

use super::{BaseCase, ExecutionMode, TransformFlag};

/// The name every matrix case stages its fixture under.
///
/// A reporting mode names the file it reports, and that name has to survive
/// into a snapshot, so the command runs in the temporary directory and is given
/// this relative name rather than a path the temporary directory invented. It
/// carries the `.dat` extension every matrix fixture uses, which the harness's
/// own self-test pins.
pub(crate) const STAGED_FILE: &str = "input.dat";

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
pub(crate) const REPORTING_MODES: &[ExecutionMode] = &[ExecutionMode::Check, ExecutionMode::Diff];

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
