# Architectural decision record (ADR) 0010: Select files through `git ls-files`

## Status

Accepted.

## Date

2026-09-12.

## Context and problem statement

`mdtablefix` acts only on the paths it is given. That default is the right one
— a formatter that decides for itself which files to read is a formatter that
rewrites a file nobody asked it to touch — but it leaves the tool awkward to
use across a repository. The idiom it forces is a shell pipeline:

```sh
git ls-files -z --deduplicate --cached --others --exclude-standard \
    | xargs -0 mdtablefix --wrap --in-place
```

Such a pipeline is sound only by accident of how it is spelled. `xargs` splits
the listing into as many invocations as it needs, so the formatter runs several
times over disjoint batches and prints several summaries; a path may be counted
twice; the status a continuous-integration job reads is `xargs`'s rather than
the formatter's; and against a repository large enough to exceed the platform's
command-line limit, the batch boundaries become behaviour rather than
implementation detail. Each user re-invents the pipeline, and each re-invention
has to rediscover the same facts about `git ls-files`: its output is not
globally sorted, it names paths that are no longer on disk, it is scoped to the
directory it runs in, and without `-z` it C-quotes any path that is not plain
ASCII.

The question this record answers is therefore: which component decides which
files a run touches, given that Git — not this tool — is the authority on what
a repository holds?

Three properties are not negotiable, and they are what make this an
architectural decision rather than a feature request.

- **A selection must not write outside itself.** `--git` writes only to regular
  files that the configured `git ls-files` invocation reported and whose
  extension is in the configured set. The guarantee is stated in terms of
  inodes written rather than paths selected, because a symlink is a reported
  path that writes to a different inode.
- **A selection must not corrupt a conflict resolution.** Reflowing a file that
  carries conflict markers restructures text on both sides of the boundary, so
  the user would resolve against corrupted content and commit it into a
  rewritten history, where `git rebase --abort` is no longer available.
- **The capability boundary must hold.** Every content read and write in the
  binary flows through the directory capability that `open_file_parent`
  returns. A selection that opened files by its own route would be a second,
  unguarded path to the filesystem, inside the code whose entire purpose is to
  decide what may be written.

## Decision drivers

- Git is the authority on what a repository holds. Re-implementing index
  parsing, `.gitignore` resolution, or submodule handling in this crate would
  make the tool's answer drift from Git's.
- No new dependency. `git2`, `gix`, `ignore`, `walkdir`, and `glob` are all
  excluded, so candidates are discovered by a process run through
  `std::process::Command`.
- Determinism. The same repository state selects the same files in the same
  order, whatever the platform, the scheduler, or Git's own output order.
- A machine-readable answer to "what would this run touch?". Tracing cannot
  give one, because the selection's tracing fields may not carry paths.
- The selection is binary-private. It adds no public library API, so it makes
  no semver commitment for what is, today, a single consumer.
- Extension policy is this tool's own. Which suffixes count as Markdown is a
  formatter's question rather than a repository's.

## Options considered

### Option A: the tool runs `git ls-files`

`--git` runs `git ls-files -z --deduplicate --cached`, optionally extended with
`--others --exclude-standard`, in the working directory, and treats its output
as the candidate set.

Advantages: equivalence with the reference pipeline is by construction rather
than by reimplementation, and every Git configuration input is inherited for
free. The index is also authoritative in a way a walk cannot be:
`git add --force` puts a path into the index although `.gitignore` names it, so
`--cached` reports it — while a walk honouring ignore rules is structurally
unable to see it, however carefully that walk is written. Disadvantages: `git`
must be on `PATH`, and a run outside a repository fails rather than selecting
nothing.

### Option B: positional directories, walked by the tool

A positional argument that names a directory is expanded by walking it, with
Git's ignore rules applied through the `ignore` crate.

Advantages: no subprocess, and one argument covers a whole tree. Disadvantages:
this is the option that cannot see a force-added ignored file, so the walk and
`git ls-files --cached` would disagree about the same repository; it introduces
the first dependency for file discovery; and it changes the existing positional
contract, which selection does not need in order to exist. This is the natural
successor if recursive selection is wanted later, and it should then be
designed against the index rather than against a walk.

### Option C: link a Git library

Select through `git2` or `gix` rather than through a subprocess.

Advantages: no `PATH` requirement, and structured access to the index.
Disadvantages: a heavy dependency inside a text-processing crate, and libgit2
diverges from Git on nested `.gitignore` negation, so equivalence with Git
would be approximate — precisely the drift this design exists to avoid.

### Option D: document the pipeline and change nothing

Advantages: no surface at all. Disadvantages: the exit status remains
`xargs`'s, the summary is printed once per batch, and every user keeps
re-deriving the framing for themselves.

| Topic                           | Option A | Option B | Option C      | Option D |
| ------------------------------- | -------- | -------- | ------------- | -------- |
| Agrees with the index           | yes      | no       | mostly        | yes      |
| One process, one exit status    | yes      | yes      | yes           | no       |
| New dependency                  | none     | a walker | a Git library | none     |
| Selection policy is this tool's | yes      | yes      | no            | no       |

_Table 1: Comparison of file-selection designs._

## Decision outcome

Option A. The command line gains one flag that selects, four that modify the
selection, and one mode that reports it:

- `--git` selects the files Git reports beneath the current directory.
- `--include-untracked` extends the candidate set with the files Git would
  commit, which is to say the untracked ones it does not ignore.
- `--md-exts EXT[,EXT...]` replaces the default extension set, `md`, `mdc`, and
  `markdown`. A leading dot is optional and surrounding whitespace is ignored.
- `--allow-conflicted` rewrites files that the conflict guard would otherwise
  refuse.
- `--list-files` prints the resolved selection, one path per line, and exits
  without reading or writing any file content.

`src/git_inputs.rs` is the composition root: it turns the command line into
that question, and the answer back into paths. The selection itself lives in
`src/select/`, a private module tree that `src/lib.rs` does not name.

### The selection is a rule, not a procedure

Candidates come from one subprocess: `git ls-files -z --deduplicate --cached`,
run with the working directory as its `current_dir`; `--include-untracked` adds
`--others --exclude-standard`. The argv is fixed and assembled in this crate.
The program is `git`, and nothing a user typed on this command line — a path,
an extension, a repository name — is ever handed to the child or parsed by a
shell, because `Command::new` is used rather than a shell string. This is the
first half of the security stance; the second half concerns Git's output, below.

`-z` is mandatory rather than an optimisation. Without it, Git C-quotes a path
containing a space, a quote, or a non-ASCII byte, and the quoted form is not a
path this tool should attempt to unquote. `--deduplicate` collapses the three
listings an unresolved merge produces — one per index stage — and the domain
deduplicates again on file identity, because a correct upstream flag is not a
reason to weaken an invariant.

Git's order is not preserved. The selection is sorted byte-wise before it is
acted on, so `--list-files` and the concatenated output of a print-mode run are
deterministic functions of repository state rather than of the order in which
two separately sorted passes happened to be merged. This is the one respect in
which a `--git` run differs from a run over positional paths, which reports in
argument order.

The policy is a pure function of a candidate list and a `PathProbe`. A
candidate is kept only if its last extension is in the configured set — matched
ASCII case-insensitively, with a leading dot accepted — and the probe reports a
regular file. An absent candidate (a staged deletion), a symbolic link, and
anything else are skipped silently, because each is an ordinary repository
state rather than a user error. The probe's answer carries a `FileIdentity`,
which is the canonicalized path: two names for one file collapse, and two hard
links to one inode do not. Keying identity on `(st_dev, st_ino)` would be the
opposite mistake, because `--in-place` replaces a file through a temporary file
and a rename, so formatting one of a pair of hard links would leave the other
pointing at the stale, unformatted content.

`PathProbe` is the one driven port, and policy depends on it and on nothing
else; the adapters and the composition root depend on policy. The extension is
tested before the filesystem is touched, so a run probes one path per distinct
Markdown candidate, and never a `.rs` file. Selection reads metadata only: it
never opens a file, and the writer still decides what to do about a path that
cannot be read.

### The command line states its own dependencies

`--git` and the positional `files` are alternatives in one clap argument group,
so they are mutually exclusive and `--in-place` is satisfied by either; a
positional path that no mode flag consumes is still a path to print. The four
selection modifiers that only make sense under `--git` are checked after
parsing rather than by `requires = "git"`, because on clap 4.6.6 that attribute
is unreliable for a valueless flag when a `Vec` positional shares the group:
`--list-files a.md` was accepted with no `--git` in sight. The post-parse check
raises a clap error, so the exit status and the usage footer remain clap's, and
`--md-exts` is checked by the source of its value rather than by its value,
since its default means it always carries one.

Selecting nothing is success. An empty selection is a file selection that
happens to be empty, not a fall-through to standard input: a run that began
reading standard input would hang a pipeline that expected to do nothing. The
`inputs` group is what makes a mode flag arrive with a source of files at all.

### A rewrite is guarded, and the guard is paid for only when it can matter

The conflict guard is consulted by a run that can write, and by no other. Under
`--check`, `--diff`, and `--list-files` it is inert and the Git directory is
never resolved, so those modes cost one subprocess. Under `--in-place` the Git
directory is resolved through `git rev-parse --absolute-git-dir` rather than by
looking for a `.git` entry: a linked worktree and a submodule both hold a
`.git` file naming a directory elsewhere, and `GIT_DIR` may point anywhere, so
a search would misresolve exactly the repositories in which an operation is
most likely to be paused.

If the repository is mid-merge, mid-rebase, or mid-cherry-pick, a selected file
carrying conflict markers is refused and named on standard error. Measured on a
repository paused mid-merge:

```console
$ mdtablefix --git --in-place
writing docs.md

Caused by:
    refusing to rewrite docs.md: it contains conflict markers and a merge,
    rebase, or cherry-pick is in progress. Resolve it first, or pass
    --allow-conflicted to rewrite it anyway.
$ echo $?
2
```

_The tool's own line is wrapped here to fit this page: one cause is printed on
one line, however long it is._

The refusal is per file, so every other selected file is still rewritten.
Marker detection requires all three forms — a line of seven `<`, a line of seven
`=`, and a line of seven `>` — each with an exact seven-character run, so a
document that merely discusses conflict markers is not mistaken for a
conflicted one, and a setext heading underline is not read as a separator. All
three are scanned only while an operation is in progress, because a fenced
example quoting all three is otherwise indistinguishable from a conflict, and
refusing to rewrite it would be a false alarm about a file nothing is merging.
`--allow-conflicted` is the escape hatch for the case where the verdict is
wrong.

### Git's diagnostics are untrusted input

Git's standard error is relayed, because a user reading a failure wants Git's
own reason beside this tool's, but it is scrubbed on the way through. Every
control character becomes a space, so an escape sequence chosen by a repository
name cannot drive the terminal that reads the diagnostic; a multi-line message
is folded to one line, so a repository cannot forge additional lines of this
tool's standard error; bytes that are not UTF-8 become the replacement
character, because the text is a diagnostic rather than a path and nothing acts
on it; and the relayed run is capped at 1024 characters, with a visible
ellipsis when it is cut, so a flood cannot bury the message it is supposed to
support. The tool's own wording is the part a test asserts on, and Git's own
text is relayed beside it rather than folded into it.

A failure anywhere in the selection prints one line to standard error and exits
`2`, with Git's diagnostic appended where Git supplied one. Measured outside
any repository:

```console
$ mdtablefix --git --check
mdtablefix: `git ls-files` failed with exit status: 128: fatal: not a git
repository (or any of the parent directories): .git
$ echo $?
2
```

_The one line is wrapped here to fit this page._

### Paths that cannot be represented

A selected path that is not valid UTF-8 cannot become a `Utf8PathBuf`, and
writing an unrepresentable path to a terminal is exactly the operation the tool
declined to perform. Such paths are counted, and the count — never the paths
themselves — is reported on standard error:

```console
$ mdtablefix --git --in-place
mdtablefix: 1 file(s) not selected: their names are not valid UTF-8
```

It goes to standard error rather than standard output because standard output
under `--list-files` is the selection itself, and a reader parsing it would
otherwise meet a line that is not a path.

## Consequences

- A repository-wide run is one invocation: one summary, one exit status, and no
  command-line length limit, which is what the reference pipeline could not
  offer. `mdtablefix --git --in-place --wrap` replaces the `xargs` idiom, and
  `mdtablefix --git --check` is the gate the feature exists to enable.
- The exit-status contract is unchanged from ADR 0009: `0` when every file was
  analysed and no reporting mode found drift, `1` when a reporting mode did, and
  `2` when a file could not be read or rewritten or the selection itself
  failed. A failed selection is reported before any file is analysed.
- A selection costs one `git` process, and a second only for a run that will
  write. Both are read-only queries.
- A `--git` run outside a repository is a failure with Git's own diagnostic
  relayed, not an empty success.
- The order a selection is reported in is byte-wise over paths, which is not
  Git's order; a script that consumes `--list-files` should treat it as a set
  unless it has asked for this tool's ordering.
- A selection can be large, so analysis results are drained in chunks of 256
  rather than in one pass. The chunking is unobservable: the chunks partition
  the already-sorted selection, and order is restored within each, so the
  output is the same as if it had never been chunked.
- The selection adds no public library API. `src/lib.rs` is untouched, so no
  semver commitment follows from it.
- Because `--check`, `--diff`, and `--in-place` were already one assessment
  rendered per mode, `--git` reaches the same capability-scoped writer as a
  positional path: the two sources differ in how the paths were found and in
  nothing else.

## Known risks and limitations

- Selection is by last extension only, so `docs/guide.md.bak` is not selected
  and neither is a directory named `data.md`, which is not a regular file. Case
  folding is ASCII, matching the extension matcher elsewhere in the tool.
- A symlink is skipped rather than followed, and skipped silently, so a
  repository that keeps its Markdown behind links selects nothing from them.
  This is the same refusal `--in-place` makes for a positional path, moved to
  selection, where it can no longer fail a run.
- The conflict guard is a heuristic in both directions. It refuses a document
  that quotes all three marker forms inside a fence during a real operation,
  and it does not detect conflict markers left behind by an operation that has
  already been completed. `--allow-conflicted` addresses the first;
  `git status` remains the authority for the second.
- Marker state is read at most once per run, so a merge that begins while a long
  `--in-place` run is already under way is not noticed by that run.
- Line endings are chosen by this tool's own majority rule and not by
  `.gitattributes`, and detection covers the whole document including fenced
  code. A predominantly CRLF file whose code samples use LF therefore has those
  samples rewritten to CRLF by a `--git --in-place` run, which is a change to
  the content of the code rather than to its formatting. The user's guide
  records this before the sections that use it.
- A repository-wide `--in-place` applies every requested transform to every
  selected file in one command, so a transform that is not a fixed point is
  correspondingly wider in effect. Issue #478 records one such residual, where
  `--code-emphasis` settles on a second pass rather than the first.
- Whether `std::fs::canonicalize` folds case on macOS and Windows is asserted
  from documentation rather than measured on those platforms, so the identity
  rule is only proved case-insensitive-by-canonicalization on Linux. If it is
  shown not to hold, the fallback is a comparison on `(st_dev, st_ino)` plus
  parent-directory identity.
