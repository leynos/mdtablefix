//! Contract tests for the `RUSTFLAGS` the workflows assign outside the Makefile.
//!
//! The build standard's own contract (`build_standard_contract.rs`) holds the
//! development recipes and the `setup-rust` steps. Five workflow places assign
//! `RUSTFLAGS` themselves, and an assigned value replaces every `rustflags`
//! table in `.cargo/config.toml`, so each is pinned here to the value it needs:
//!
//! - the Windows atomic-write job runs `cargo` directly on a runner without mold, so its job-level
//!   `env:` restates the parallel frontend beside the warning deny (`-D warnings -Zthreads=8`);
//! - the two coverage steps are measurements, so they take the deny alone;
//! - the two stable release-build steps must never see the nightly-only `-Zthreads`, so each states
//!   the deny alone in its own `env:` block.
//!
//! A workflow step is judged by its own `env:` block and a job by its own
//! job-level one, so a sibling's environment, a comment and an inline comment
//! neither supply nor hide a value, and a job-level assignment does not stand
//! in for a step's. Fixtures come first, so no rule passes by finding nothing.

/// A workflow file, with its text.
#[derive(Clone, Copy)]
struct Workflow {
    file: &'static str,
    text: &'static str,
}

/// Where an assignment must sit.
#[derive(Clone, Copy)]
enum Scope {
    /// The `env:` block of the step with this `name:`.
    Step(&'static str),
    /// The job-level `env:` block of the job with this id.
    Job(&'static str),
}

/// What one place must assign.
struct Expected {
    workflow: Workflow,
    scope: Scope,
    rustflags: &'static str,
}

const CI: Workflow = Workflow {
    file: "ci.yml",
    text: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/.github/workflows/ci.yml"
    )),
};

const COVERAGE_MAIN: Workflow = Workflow {
    file: "coverage-main.yml",
    text: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/.github/workflows/coverage-main.yml"
    )),
};

const RELEASE: Workflow = Workflow {
    file: "release.yml",
    text: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/.github/workflows/release.yml"
    )),
};

const EXPECTED: &[Expected] = &[
    Expected {
        workflow: CI,
        scope: Scope::Job("windows-atomic-contract"),
        rustflags: "-D warnings -Zthreads=8",
    },
    Expected {
        workflow: CI,
        scope: Scope::Step("Test and Measure Coverage"),
        rustflags: "-D warnings",
    },
    Expected {
        workflow: CI,
        scope: Scope::Step("Build the release binary"),
        rustflags: "-D warnings",
    },
    Expected {
        workflow: COVERAGE_MAIN,
        scope: Scope::Step("Test and Measure Coverage"),
        rustflags: "-D warnings",
    },
    Expected {
        workflow: RELEASE,
        scope: Scope::Step("Build release binary"),
        rustflags: "-D warnings",
    },
];

/// Returns the number of leading spaces on a line.
fn indent(line: &str) -> usize { line.len() - line.trim_start().len() }

/// Returns whether a line carries nothing a block is judged by.
fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// Returns a value without any inline YAML comment or quotes.
fn plain(value: &str) -> &str {
    value
        .split(" #")
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches(['"', '\''])
}

/// Returns the lines of every step with the given `name:`: from its list item to
/// the line before the next item at the same indentation or a dedent.
fn steps_named<'a>(text: &'a str, name: &str) -> Vec<Vec<&'a str>> {
    let lines: Vec<&str> = text.lines().collect();
    let opens = |line: &str| {
        let item = line.trim_start().strip_prefix("- ").unwrap_or_default();
        item.strip_prefix("name:")
            .is_some_and(|rest| rest.trim().trim_matches(['"', '\'']) == name)
    };
    let ends_at = |item_indent: usize, from: usize| {
        lines
            .iter()
            .enumerate()
            .skip(from)
            .find(|(_, next)| {
                !is_blank_or_comment(next)
                    && (indent(next) < item_indent
                        || (indent(next) == item_indent && next.trim_start().starts_with("- ")))
            })
            .map_or(lines.len(), |(index, _)| index)
    };
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| opens(line))
        .map(|(at, line)| {
            let end = ends_at(indent(line), at + 1);
            lines.get(at..end).unwrap_or_default().to_vec()
        })
        .collect()
}

/// Returns the lines of the job with the given id: from its key to the line
/// before the next key at the same indentation or a dedent.
fn job_lines<'a>(text: &'a str, id: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = text.lines().collect();
    let key = format!("{id}:");
    let Some(at) = lines
        .iter()
        .position(|line| indent(line) == 2 && line.trim() == key)
    else {
        return Vec::new();
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(at + 1)
        .find(|(_, next)| !is_blank_or_comment(next) && indent(next) <= 2)
        .map_or(lines.len(), |(index, _)| index);
    lines.get(at..end).unwrap_or_default().to_vec()
}

/// Returns the entries of the `env:` block that sits `depth` spaces in from the
/// start of the given lines' first line: for a step, the block under its keys;
/// for a job, the block directly under the job.
fn env_of(block: &[&str], key_indent: usize) -> Vec<(String, String)> {
    let Some(at) = block
        .iter()
        .position(|line| line.trim() == "env:" && indent(line) == key_indent)
    else {
        return Vec::new();
    };
    block
        .iter()
        .skip(at + 1)
        .take_while(|line| is_blank_or_comment(line) || indent(line) > key_indent)
        .filter(|line| !is_blank_or_comment(line))
        .filter_map(|line| line.trim().split_once(':'))
        .map(|(key, value)| (key.trim().to_owned(), plain(value).to_owned()))
        .collect()
}

/// Returns the `RUSTFLAGS` a scope assigns in its own `env:` block, or the
/// complaint when the scope is absent or repeated.
fn assigned(workflow: Workflow, scope: Scope) -> Result<Option<String>, String> {
    let (block, key_indent) = match scope {
        Scope::Step(name) => {
            let named = steps_named(workflow.text, name);
            let [step] = named.as_slice() else {
                return Err(format!(
                    "{}: expected one step named {name:?}, found {}",
                    workflow.file,
                    named.len()
                ));
            };
            // A step's own keys sit two spaces in from its `- ` item.
            let item_indent = step.first().map_or(0, |line| indent(line));
            (step.clone(), item_indent + 2)
        }
        Scope::Job(id) => {
            let job = job_lines(workflow.text, id);
            if job.is_empty() {
                return Err(format!("{}: no job {id:?}", workflow.file));
            }
            (job, 4)
        }
    };
    let env = env_of(&block, key_indent);
    Ok(env
        .into_iter()
        .rev()
        .find(|(key, _)| key == "RUSTFLAGS")
        .map(|(_, value)| value))
}

/// Returns the complaint about one expectation against a workflow text, if any.
fn problem(expected: &Expected) -> Option<String> {
    let place = match expected.scope {
        Scope::Step(name) => format!("step {name:?}"),
        Scope::Job(id) => format!("job {id:?}"),
    };
    match assigned(expected.workflow, expected.scope) {
        Err(reason) => Some(reason),
        Ok(Some(found)) if found == expected.rustflags => None,
        Ok(found) => Some(format!(
            "{}: {place} has RUSTFLAGS={found:?}, not {:?}",
            expected.workflow.file, expected.rustflags
        )),
    }
}

/// Returns the complaints about a list of expectations.
fn problems(expectations: &[Expected]) -> Vec<String> {
    expectations.iter().filter_map(problem).collect()
}

/// Turns a list of complaints into a test result.
fn none_of(found: &[String]) -> Result<(), String> {
    if found.is_empty() {
        Ok(())
    } else {
        Err(format!("{found:#?}"))
    }
}

/// A one-place expectation over a fixture text.
fn on(text: &'static str, scope: Scope, rustflags: &'static str) -> Expected {
    Expected {
        workflow: Workflow {
            file: "fixture.yml",
            text,
        },
        scope,
        rustflags,
    }
}

const STEP_OK: &str = "\
jobs:
  build:
    steps:
      - name: Build
        env:
          # the deny alone
          RUSTFLAGS: -D warnings # stable
        run: cargo build
";

const STEP_NO_ENV: &str = "\
jobs:
  build:
    steps:
      - name: Build
        run: cargo build
";

const STEP_WRONG: &str = "\
jobs:
  build:
    steps:
      - name: Build
        env:
          RUSTFLAGS: -D warnings -Zthreads=8
";

const STEP_COMMENTED: &str = "\
jobs:
  build:
    steps:
      - name: Build
        env:
          # RUSTFLAGS: -D warnings
          OTHER: x
";

const STEP_BORROWING: &str = "\
jobs:
  build:
    steps:
      - name: Build
        run: cargo build
      - name: Sibling
        env:
          RUSTFLAGS: -D warnings
";

const STEP_TWICE: &str = "\
jobs:
  build:
    steps:
      - name: Build
        env:
          RUSTFLAGS: -D warnings
      - name: Build
        run: true
";

const STEP_UNDER_A_JOB_ENV: &str = "\
jobs:
  build:
    env:
      RUSTFLAGS: -D warnings
    steps:
      - name: Build
        run: cargo build
";

const JOB_OK: &str = "\
jobs:
  windows:
    env:
      CARGO_TERM_COLOR: always
      # restates the frontend flag
      RUSTFLAGS: \"-D warnings -Zthreads=8\"
    steps:
      - name: Build
        run: cargo build
";

const JOB_NO_ENV: &str = "\
jobs:
  windows:
    steps:
      - name: Build
        env:
          RUSTFLAGS: -D warnings -Zthreads=8
";

const JOB_LOSES_FRONTEND: &str = "\
jobs:
  windows:
    env:
      RUSTFLAGS: -D warnings
    steps: []
";

#[test]
fn the_real_workflows_hold_their_assignments() -> Result<(), String> {
    none_of(&problems(EXPECTED))
}

#[test]
fn a_step_that_assigns_the_value_is_accepted() -> Result<(), String> {
    none_of(&problems(&[on(
        STEP_OK,
        Scope::Step("Build"),
        "-D warnings",
    )]))
}

#[test]
fn a_job_that_assigns_the_value_is_accepted() -> Result<(), String> {
    none_of(&problems(&[on(
        JOB_OK,
        Scope::Job("windows"),
        "-D warnings -Zthreads=8",
    )]))
}

#[test]
fn a_step_that_loses_or_changes_its_value_is_refused() {
    for (label, text) in [
        ("no env", STEP_NO_ENV),
        ("a wider value", STEP_WRONG),
        ("a commented-out assignment", STEP_COMMENTED),
        ("an assignment on a sibling", STEP_BORROWING),
        ("a job-level assignment", STEP_UNDER_A_JOB_ENV),
    ] {
        assert_eq!(
            problems(&[on(text, Scope::Step("Build"), "-D warnings")]).len(),
            1,
            "{label}"
        );
    }
}

#[test]
fn a_job_that_loses_or_changes_its_value_is_refused() {
    for (label, text) in [
        ("an assignment on a step instead", JOB_NO_ENV),
        ("the frontend flag dropped", JOB_LOSES_FRONTEND),
    ] {
        assert_eq!(
            problems(&[on(text, Scope::Job("windows"), "-D warnings -Zthreads=8")]).len(),
            1,
            "{label}"
        );
    }
}

#[test]
fn a_place_that_is_absent_or_repeated_proves_nothing() {
    assert_eq!(
        problems(&[on("jobs: {}\n", Scope::Step("Build"), "-D warnings")]).len(),
        1
    );
    assert_eq!(
        problems(&[on(STEP_TWICE, Scope::Step("Build"), "-D warnings")]).len(),
        1
    );
    assert_eq!(
        problems(&[on("jobs: {}\n", Scope::Job("windows"), "-D warnings")]).len(),
        1
    );
}
