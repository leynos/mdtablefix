//! Unit tests for report rendering.

use std::io;

use camino::Utf8Path;
use rstest::rstest;

use super::{DiffOptions, render_summary, write_unified_diff};

/// The message [`FailingWriter`] injects and the caller must receive.
///
/// Named rather than written twice: the renderer's contract is that the
/// *writer's* error reaches the caller, so an error re-wrapped downstream
/// with the same kind but a message of its own has to fail the test below.
/// A repeated literal would match that substitution and pass.
const WRITER_FAILURE_MESSAGE: &str = "closed";

/// A writer that accepts `budget` bytes and then fails every write.
///
/// The budget makes the failure land at a chosen point in the stream, so
/// one type covers both a writer that is already closed and one that fails
/// partway through the rendered diff.
struct FailingWriter {
    budget: usize,
}

impl io::Write for FailingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.budget {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                WRITER_FAILURE_MESSAGE,
            ));
        }
        self.budget -= buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[rstest]
#[case(0, 0, 0, "No files were analysed.")]
#[case(0, 0, 1, "1 file could not be read.")]
#[case(0, 0, 2, "2 files could not be read.")]
#[case(0, 1, 0, "1 file left unchanged.")]
#[case(0, 1, 1, "1 file left unchanged, 1 file could not be read.")]
#[case(0, 1, 2, "1 file left unchanged, 2 files could not be read.")]
#[case(0, 2, 0, "2 files left unchanged.")]
#[case(0, 2, 1, "2 files left unchanged, 1 file could not be read.")]
#[case(0, 2, 2, "2 files left unchanged, 2 files could not be read.")]
#[case(1, 0, 0, "1 file would be reformatted.")]
#[case(1, 0, 1, "1 file would be reformatted, 1 file could not be read.")]
#[case(1, 0, 2, "1 file would be reformatted, 2 files could not be read.")]
#[case(1, 1, 0, "1 file would be reformatted, 1 file left unchanged.")]
#[case(
    1,
    1,
    1,
    "1 file would be reformatted, 1 file left unchanged, 1 file could not be read."
)]
#[case(
    1,
    1,
    2,
    "1 file would be reformatted, 1 file left unchanged, 2 files could not be read."
)]
#[case(1, 2, 0, "1 file would be reformatted, 2 files left unchanged.")]
#[case(
    1,
    2,
    1,
    "1 file would be reformatted, 2 files left unchanged, 1 file could not be read."
)]
#[case(
    1,
    2,
    2,
    "1 file would be reformatted, 2 files left unchanged, 2 files could not be read."
)]
#[case(2, 0, 0, "2 files would be reformatted.")]
#[case(2, 0, 1, "2 files would be reformatted, 1 file could not be read.")]
#[case(2, 0, 2, "2 files would be reformatted, 2 files could not be read.")]
#[case(2, 1, 0, "2 files would be reformatted, 1 file left unchanged.")]
#[case(
    2,
    1,
    1,
    "2 files would be reformatted, 1 file left unchanged, 1 file could not be read."
)]
#[case(
    2,
    1,
    2,
    "2 files would be reformatted, 1 file left unchanged, 2 files could not be read."
)]
#[case(2, 2, 0, "2 files would be reformatted, 2 files left unchanged.")]
#[case(
    2,
    2,
    1,
    "2 files would be reformatted, 2 files left unchanged, 1 file could not be read."
)]
#[case(
    2,
    2,
    2,
    "2 files would be reformatted, 2 files left unchanged, 2 files could not be read."
)]
fn summary_renders_every_combination(
    #[case] changed: usize,
    #[case] unchanged: usize,
    #[case] errored: usize,
    #[case] expected: &str,
) {
    assert_eq!(render_summary(changed, unchanged, errored), expected);
}

/// An original whose last line has no terminator keeps the marker that
/// says so, and only on the deletions side.
///
/// The formatter terminates every line it writes, so a marker beside an
/// added line would mean the renderer had marked the wrong side — or had
/// read the original's terminator as part of the line. Both halves are
/// asserted here because the second is invisible in the first: a run that
/// lost the marker and one that placed it wrongly print different bytes,
/// and this pins the bytes.
#[test]
fn an_unterminated_original_marks_the_deleted_line() {
    let mut out = Vec::new();
    write_unified_diff(
        &mut out,
        Utf8Path::new("ragged.md"),
        "|A|B|\n|---|---|\n|1|2|",
        "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n",
        DiffOptions {
            context_radius: 3,
            patience_threshold: 1000,
        },
    )
    .expect("writing to a Vec cannot fail");

    assert_eq!(
        String::from_utf8(out).expect("diff is UTF-8"),
        concat!(
            "--- ragged.md\n",
            "+++ ragged.md\n",
            "@@ -1,3 +1,3 @@\n",
            "-|A|B|\n",
            "-|---|---|\n",
            "-|1|2|\n",
            "\\ No newline at end of file\n",
            "+| A   | B   |\n",
            "+| --- | --- |\n",
            "+| 1   | 2   |\n",
        )
    );
}

/// Byte-equal texts render as nothing at all, headers included: `similar`
/// writes the header alongside the first hunk, and equal texts have no
/// hunk. A document that is already formatted therefore cannot render as
/// an empty diff carrying a file name.
///
/// The driver still guards its diff arm on a change being present, so this
/// test is what keeps that guard's absence a cost rather than a
/// difference in output. See `EP-M6` in the `ExecPlan`.
#[test]
fn equal_texts_render_nothing() {
    let mut out = Vec::new();
    write_unified_diff(
        &mut out,
        Utf8Path::new("clean.md"),
        "| A | B |\n",
        "| A | B |\n",
        DiffOptions {
            context_radius: 3,
            patience_threshold: 1000,
        },
    )
    .expect("writing to a Vec cannot fail");
    assert!(out.is_empty(), "a clean file has no diff: {out:?}");
}

/// A failing output writer's own error reaches the caller.
///
/// The doc comment promises that a failure to write is returned, and this
/// is the only test that can show it: every other renderer test writes
/// into a `Vec`, which cannot fail. `budget` places the failure before any
/// byte is written and inside the rendered body, after the headers have
/// been accepted.
///
/// The message is asserted as well as the kind, because the kind alone is
/// a weaker claim than it looks: a `to_writer` that re-wrapped the writer's
/// failure as `io::Error::new(BrokenPipe, "diff failed")` would keep the
/// kind and still lose what the writer said. The message is what separates
/// passing an error through from manufacturing one.
#[rstest]
#[case(0)]
#[case(16)]
fn a_failing_writer_error_reaches_the_caller(#[case] budget: usize) {
    let mut out = FailingWriter { budget };
    let error = write_unified_diff(
        &mut out,
        Utf8Path::new("ragged.md"),
        "|A|B|\n",
        "| A | B |\n",
        DiffOptions {
            context_radius: 3,
            patience_threshold: 1000,
        },
    )
    .expect_err("a writer that fails must fail the render");

    assert_eq!(
        error.kind(),
        io::ErrorKind::BrokenPipe,
        "the writer's own error must reach the caller: {error}"
    );
    assert_eq!(
        error.to_string(),
        WRITER_FAILURE_MESSAGE,
        "the writer's own message must survive, not just its kind: {error}"
    );
}

#[test]
fn summary_grammar_is_snapshot_stable() {
    let mut rendered = String::new();
    for changed in 0..3 {
        for unchanged in 0..3 {
            for errored in 0..3 {
                rendered.push_str(&render_summary(changed, unchanged, errored));
                rendered.push('\n');
            }
        }
    }
    insta::assert_snapshot!("summary_grammar", rendered);
}
