//! The corpus `tests/check_prediction.rs` runs its prediction check over.
//!
//! A module of its own because the corpus is measured data rather than a test,
//! and because the test file stays inside `AGENTS.md`'s line limit this way.
//!
//! The association between a flag and its document was measured rather than
//! assumed — each of the eight changes its own fixture and leaves the bare
//! command's output alone. The boundary documents drift under the default table
//! rewrite, which no flag governs, so they drift whatever the flag set.
//!
//! `unterminated_clean` is the other kind of boundary: already formatted apart
//! from a missing final terminator, so the whole of its drift is the one byte a
//! change decision made on trimmed text would call equal. It is the fixture the
//! `INV-PREDICTS` negative control fails on; see the plan's
//! `Artefacts and notes → EP-M7 prediction control`. Prose and the empty
//! document are the clean half: a corpus that only ever drifted would leave the
//! silence a clean run must produce untested.

/// One corpus document, with the flag it exists to exercise.
pub struct Fixture {
    /// The name used in failure messages.
    pub name: &'static str,
    /// The document, written to a fresh file for each run.
    pub input: &'static str,
    /// The one flag the document drifts under and the bare command does not.
    ///
    /// A flag whose document never drifts is untested, however many cases name
    /// it, so every one of the eight has a fixture here and the corpus test
    /// asserts the association. `None` marks the boundary and clean documents,
    /// which exist for other reasons.
    pub flag: Option<&'static str>,
}

/// Builds a [`Fixture`] at compile time.
const fn fixture(name: &'static str, input: &'static str, flag: Option<&'static str>) -> Fixture {
    Fixture { name, input, flag }
}

/// The corpus: one document per flag, then the boundary and clean shapes.
pub const CORPUS: &[Fixture] = &[
    fixture(
        "long_prose",
        "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen \
         sixteen seventeen\n",
        Some("--wrap"),
    ),
    fixture(
        "ordered_list",
        "1. alpha\n1. beta\n1. gamma\n",
        Some("--renumber"),
    ),
    fixture("thematic_break", "alpha\n\n***\n\nbeta\n", Some("--breaks")),
    fixture("ellipsis", "alpha ... beta\n", Some("--ellipsis")),
    fixture("long_fence", "````sh\necho hi\n````\n", Some("--fences")),
    fixture(
        "footnotes",
        "A claim.1\n\n1. A note.\n",
        Some("--footnotes"),
    ),
    fixture(
        "code_emphasis",
        "alpha *`beta`* gamma\n",
        Some("--code-emphasis"),
    ),
    fixture("setext_heading", "Title\n=====\n", Some("--headings")),
    fixture("ragged_table", "|A|B|\n|---|---|\n|1|2|\n", None),
    fixture(
        "clean_table",
        "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n",
        None,
    ),
    fixture("crlf_table", "|A|B|\r\n|---|---|\r\n|1|2|\r\n", None),
    fixture("bom_table", "\u{FEFF}|A|B|\n|---|---|\n|1|2|\n", None),
    fixture("unterminated_table", "|A|B|\n|---|---|\n|1|2|", None),
    fixture(
        "unterminated_clean",
        "| A   | B   |\n| --- | --- |\n| 1   | 2   |",
        None,
    ),
    fixture(
        "prose",
        "one two three four five six seven eight nine ten eleven twelve\n",
        None,
    ),
    fixture("empty", "", None),
];
