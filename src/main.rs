//! Binary entry point for `mdtablefix`.
//!
//! Parses command-line arguments and coordinates Markdown formatting. When
//! file paths are supplied, processing occurs in parallel and files may be
//! rewritten in place. Without paths the tool reads from standard input and
//! prints results to stdout while preserving the input order.

use std::{
    borrow::Cow,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, File, OpenOptions, Permissions},
};
use clap::Parser;
use mdtablefix::{
    Options,
    format_breaks,
    process::{process_stream_inner, process_with_frontmatter},
    renumber_lists,
};
use rayon::prelude::*;

#[derive(Parser)]
#[command(version, about = "Reflow broken markdown tables")]
struct Cli {
    /// Rewrite files in place
    #[arg(long = "in-place", requires = "files")]
    in_place: bool,
    #[command(flatten)]
    opts: FormatOpts,
    /// Markdown files to fix
    files: Vec<PathBuf>,
}

#[derive(clap::Args, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI exposes independent flags via separate switches"
)]
struct FormatOpts {
    /// Wrap paragraphs and list items to 80 columns
    #[arg(long = "wrap")]
    wrap: bool,
    /// Renumber ordered list items
    #[arg(long = "renumber")]
    renumber: bool,
    /// Reformat thematic breaks as underscores
    #[arg(long = "breaks")]
    breaks: bool,
    /// Replace "..." with the ellipsis character
    #[arg(long = "ellipsis")]
    ellipsis: bool,
    /// Normalise fence delimiters to three backticks
    #[arg(long = "fences")]
    fences: bool,
    /// Convert bare numeric references and the final numbered list to
    /// Markdown footnote links
    #[arg(long = "footnotes")]
    footnotes: bool,
    /// Fix emphasis markers adjacent to inline code
    #[arg(long = "code-emphasis")]
    code_emphasis: bool,
    /// Convert Setext-style headings to hash-prefixed headings
    #[arg(long = "headings")]
    headings: bool,
}

impl From<FormatOpts> for Options {
    fn from(opts: FormatOpts) -> Self {
        Self {
            wrap: opts.wrap,
            ellipsis: opts.ellipsis,
            fences: opts.fences,
            footnotes: opts.footnotes,
            code_emphasis: opts.code_emphasis,
            headings: opts.headings,
        }
    }
}

fn process_lines(lines: &[String], opts: FormatOpts) -> Vec<String> {
    process_with_frontmatter(lines, |body| {
        let mut out = process_stream_inner(body, opts.into());
        if opts.renumber {
            out = renumber_lists(&out);
        }
        if opts.breaks {
            out = format_breaks(&out)
                .into_iter()
                .map(Cow::into_owned)
                .collect();
        }
        out
    })
}

/// Opens a file's parent directory and returns its relative UTF-8 path.
///
/// This is the only ambient filesystem boundary for CLI file processing. The
/// returned directory capability restricts subsequent handler I/O to the
/// selected file's parent directory.
fn open_file_parent(path: &Path) -> anyhow::Result<(Dir, Utf8PathBuf)> {
    let path = Utf8Path::from_path(path)
        .with_context(|| format!("converting {} to a UTF-8 path", path.display()))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .unwrap_or(Utf8Path::new("."));
    let file_name = path
        .file_name()
        .with_context(|| format!("selecting file name from {path}"))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .with_context(|| format!("opening {parent}"))?;

    Ok((directory, Utf8PathBuf::from(file_name)))
}

/// Reads and formats a capability-scoped file without modifying it.
fn format_to_string(directory: &Dir, path: &Utf8Path, opts: FormatOpts) -> anyhow::Result<String> {
    let content = directory
        .read_to_string(path)
        .with_context(|| format!("reading {path}"))?;
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    let fixed = process_lines(&lines, opts);
    // Keep file output newline-terminated, matching the CLI stdout contract.
    Ok(if fixed.is_empty() {
        String::new()
    } else {
        fixed.join("\n") + "\n"
    })
}

/// Counter that keeps temporary file names unique within a process.
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Candidate temporary names to try before conceding that a stale temporary
/// file from an earlier killed run is in the way.
const TEMP_FILE_ATTEMPTS: u32 = 16;

/// Reads, formats, and atomically replaces a capability-scoped file in place.
///
/// The formatted output is written to a temporary file beside the target and
/// renamed over it, so a failure before the rename leaves the original file
/// byte-identical rather than truncated.
fn rewrite_in_place(directory: &Dir, path: &Utf8Path, opts: FormatOpts) -> anyhow::Result<()> {
    let output = format_to_string(directory, path, opts)?;
    let permissions = directory
        .metadata(path)
        .with_context(|| format!("reading metadata for {path}"))?
        .permissions();
    replace_file(directory, path, &output, permissions).with_context(|| format!("writing {path}"))
}

/// Replaces `path` with `contents` through a same-directory temporary file.
///
/// A freshly created temporary file does not inherit the target's permissions,
/// so they are copied across explicitly before the rename. Any failure after
/// the temporary file exists removes it again, so no debris is left behind.
fn replace_file(
    directory: &Dir,
    path: &Utf8Path,
    contents: &str,
    permissions: Permissions,
) -> io::Result<()> {
    let (temp_path, file) = create_temporary_file(directory, path)?;
    let outcome = write_and_swap(directory, &temp_path, path, contents, permissions, file);
    if outcome.is_err() {
        // Best effort: failing to clean up must not mask the original error,
        // and the next run retries past any stale name it finds.
        let _ = directory.remove_file(&temp_path);
    }
    outcome
}

/// Writes `contents` to `temp_path`, copies `permissions` across, and renames
/// the result over `path`.
fn write_and_swap(
    directory: &Dir,
    temp_path: &Utf8Path,
    path: &Utf8Path,
    contents: &str,
    permissions: Permissions,
    mut file: File,
) -> io::Result<()> {
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    file.sync_all()?;
    // Close the handle before renaming: Windows refuses to replace a
    // destination that another handle holds open without delete sharing.
    drop(file);
    directory.set_permissions(temp_path, permissions)?;
    directory.rename(temp_path, directory, path)
}

/// Creates a new temporary file beside `path` inside `directory`.
///
/// The name carries the process id and a per-process counter, so concurrent
/// writers in the same directory never collide. A temporary file left behind by
/// a killed run can still occupy a candidate name, so the counter advances and
/// the attempt is retried.
fn create_temporary_file(directory: &Dir, path: &Utf8Path) -> io::Result<(Utf8PathBuf, File)> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    for _ in 0..TEMP_FILE_ATTEMPTS {
        let temp_path = temporary_file_name(path);
        match directory.open_with(&temp_path, &options) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("no free temporary file name beside {path}"),
    ))
}

/// Builds a candidate temporary file name beside `path`.
///
/// The name is relative to the directory capability that owns `path`, which
/// keeps the whole replacement inside the existing filesystem boundary.
fn temporary_file_name(path: &Utf8Path) -> Utf8PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stem = path.file_name().unwrap_or_default();
    Utf8PathBuf::from(format!(
        "{stem}.mdtablefix-{}-{counter}.tmp",
        std::process::id()
    ))
}

fn report_results<T, F>(results: Vec<anyhow::Result<T>>, mut on_ok: F) -> anyhow::Result<()>
where
    F: FnMut(T),
{
    let mut first_err: Option<anyhow::Error> = None;
    for res in results {
        match res {
            Ok(val) => on_ok(val),
            Err(e) => {
                eprintln!("{e}");
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    if let Some(err) = first_err {
        Err(err)
    } else {
        Ok(())
    }
}

/// Entry point for the command-line tool that reflows broken markdown tables.
///
/// Parses command-line arguments to determine whether to process files in place, print fixed output
/// to standard output, or read from standard input. Handles file I/O and error propagation as
/// needed.
///
/// # Returns
///
/// Returns `Ok(())` if all operations complete successfully; otherwise, returns an error if
/// argument validation or file processing fails.
///
/// # Examples
///
/// ```sh
/// # Fix tables in a file and print to stdout
/// mdtablefix myfile.md
///
/// # Fix tables in place
/// mdtablefix --in-place myfile.md
///
/// # Fix tables from standard input
/// cat myfile.md | mdtablefix
/// ```
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.files.is_empty() {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let lines: Vec<String> = input.lines().map(str::to_string).collect();
        let fixed = process_lines(&lines, cli.opts);
        println!("{}", fixed.join("\n"));
        return Ok(());
    }

    if cli.in_place {
        let results: Vec<anyhow::Result<()>> = cli
            .files
            .par_iter()
            .map(|path| {
                let (directory, file_name) = open_file_parent(path)?;
                rewrite_in_place(&directory, &file_name, cli.opts)
            })
            .collect();
        report_results(results, |()| {})?;
    } else {
        let results: Vec<anyhow::Result<String>> = cli
            .files
            .par_iter()
            .map(|path| {
                let (directory, file_name) = open_file_parent(path)?;
                format_to_string(&directory, &file_name, cli.opts)
            })
            .collect();
        report_results(results, |out| print!("{out}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit and property tests for the binary's file-output contracts.

    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use camino::{Utf8Path, Utf8PathBuf};
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use proptest::prelude::*;
    use tempfile::tempdir;

    use super::{FormatOpts, format_to_string, rewrite_in_place, temporary_file_name};

    /// Format options with every transformation disabled.
    fn no_opts() -> FormatOpts {
        FormatOpts {
            wrap: false,
            renumber: false,
            breaks: false,
            ellipsis: false,
            fences: false,
            footnotes: false,
            code_emphasis: false,
            headings: false,
        }
    }

    /// Opens a directory capability on an ambient path, mirroring the CLI's
    /// only filesystem boundary.
    fn open_dir(path: &std::path::Path) -> std::io::Result<Dir> {
        let utf8 = Utf8PathBuf::from_path_buf(path.to_path_buf())
            .map_err(|path| std::io::Error::other(format!("non-UTF-8 path: {}", path.display())))?;
        Dir::open_ambient_dir(&utf8, ambient_authority())
    }

    /// Lists the sorted names of the entries in `path`.
    fn entry_names(path: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(path)
            .expect("read directory")
            .map(|entry| {
                entry
                    .expect("read directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[cfg(unix)]
    fn can_write_as_root() -> bool {
        // SAFETY: `geteuid()` has no side effects and is safe to call in tests.
        let uid = unsafe { libc::geteuid() };
        uid == 0
    }

    #[cfg(unix)]
    fn set_mode(path: &std::path::Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set permissions");
    }

    #[test]
    fn rewrite_in_place_leaves_no_temporary_file() {
        let dir = tempdir().expect("create temporary directory");
        let path = Utf8Path::new("sample.md");
        let directory = open_dir(dir.path()).expect("open directory capability");
        directory
            .write(path, "|A|B|\n|1|2|")
            .expect("write fixture");

        rewrite_in_place(&directory, path, no_opts()).expect("rewrite in place");

        assert_eq!(entry_names(dir.path()), vec!["sample.md"]);
    }

    #[test]
    fn temporary_file_name_is_relative_to_the_target_directory() {
        let name = temporary_file_name(Utf8Path::new("sample.md"));
        assert_eq!(
            name.components().count(),
            1,
            "a bare name resolves through the same directory capability as the target"
        );
        assert!(
            name.as_str().starts_with("sample.md"),
            "temporary name should extend the target name"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rewrite_in_place_preserves_file_mode() {
        let dir = tempdir().expect("create temporary directory");
        let path = Utf8Path::new("sample.md");
        let absolute = dir.path().join(path.as_str());
        let directory = open_dir(dir.path()).expect("open directory capability");
        directory
            .write(path, "|A|B|\n|1|2|")
            .expect("write fixture");
        set_mode(&absolute, 0o640);

        rewrite_in_place(&directory, path, no_opts()).expect("rewrite in place");

        let mode = fs::metadata(&absolute)
            .expect("read metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o640, "rewrite must preserve the original mode");
    }

    #[cfg(unix)]
    #[test]
    fn rewrite_in_place_keeps_original_when_write_fails() {
        let dir = tempdir().expect("create temporary directory");
        let path = Utf8Path::new("sample.md");
        let absolute = dir.path().join(path.as_str());
        let original = "|A|B|\n|1|2|";
        let directory = open_dir(dir.path()).expect("open directory capability");
        directory.write(path, original).expect("write fixture");
        // A read-only directory blocks the temporary file, which is the point
        // at which the replacement would otherwise begin.
        set_mode(dir.path(), 0o555);

        let result = rewrite_in_place(&directory, path, no_opts());

        set_mode(dir.path(), 0o755);
        if can_write_as_root() {
            // Root ignores directory permission bits, so the failure path
            // cannot be induced and the assertions below would be vacuous.
            return;
        }
        assert!(result.is_err(), "expected the write to fail");
        assert_eq!(
            fs::read_to_string(&absolute).expect("read original"),
            original,
            "a failed rewrite must leave the original byte-identical"
        );
        assert_eq!(entry_names(dir.path()), vec!["sample.md"]);
    }

    fn prose_word_strategy() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop_oneof![
                Just("alpha".to_string()),
                Just("beta".to_string()),
                Just("gamma".to_string()),
                Just("delta".to_string()),
                Just("evidence".to_string()),
                Just("formatting".to_string()),
            ],
            1..20,
        )
        .prop_map(|words| words.join(" "))
    }

    proptest! {
        #[test]
        fn formatting_matches_in_place_output(
            prose in prose_word_strategy(),
            table_cell in prose_word_strategy(),
        ) {
            let input = format!(
                "{prose}\n\n| Name | Notes |\n|---|---|\n| {table_cell} | value |\n"
            );
            let directory = tempdir()
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let directory = open_dir(directory.path())
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let formatted_path = Utf8Path::new("formatted.md");
            let rewritten_path = Utf8Path::new("rewritten.md");
            directory.write(formatted_path, &input)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            directory.write(rewritten_path, input)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;

            let formatted = format_to_string(&directory, formatted_path, no_opts())
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            rewrite_in_place(&directory, rewritten_path, no_opts())
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let rewritten = directory.read_to_string(rewritten_path)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;

            prop_assert_eq!(formatted, rewritten);
        }
    }
}
