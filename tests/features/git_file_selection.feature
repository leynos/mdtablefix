Feature: Select files from a Git repository

  As a maintainer of a Markdown-heavy repository
  I want mdtablefix to act on the repository's own Markdown files
  So that I need not enumerate them by hand or risk touching ignored files

  Background:
    Given a Git repository containing a committed file "docs/guide.md" with a broken table
    And a committed file "src/lib.rs" with a broken table
    And an untracked file "notes.md" with a broken table
    And an ignored file "build/out.md" with a broken table

  Scenario: Reformat tracked Markdown in place and nothing else
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And the file "docs/guide.md" has a reflowed table
    And the file "notes.md" is unchanged
    And the file "build/out.md" is unchanged
    And the file "src/lib.rs" is unchanged

  Scenario: Extend the selection to untracked files on request
    When I run mdtablefix with "--git --include-untracked --in-place"
    Then the command succeeds
    And the file "docs/guide.md" has a reflowed table
    And the file "notes.md" has a reflowed table
    And the file "build/out.md" is unchanged

  Scenario: List the selection without acting
    When I run mdtablefix with "--git --list-files"
    Then the command succeeds
    And stdout is exactly "docs/guide.md"
    And the file "docs/guide.md" is unchanged

  Scenario: Print the formatted documents instead of writing them
    When I run mdtablefix with "--git"
    Then the command succeeds
    And stdout contains "| A   | B   |"
    And the file "docs/guide.md" is unchanged

  Scenario: Never write through a symlink
    Given a committed symlink "docs/alias.md" pointing at "../src/lib.rs"
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And the file "src/lib.rs" is unchanged
    And the file "docs/alias.md" is still a symlink

  Scenario: Restrict the selection to chosen extensions
    Given a committed file "rules.mdc" with a broken table
    When I run mdtablefix with "--git --in-place --md-exts mdc"
    Then the command succeeds
    And the file "rules.mdc" has a reflowed table
    And the file "docs/guide.md" is unchanged

  Scenario: Accept extensions written with a leading dot
    When I run mdtablefix with "--git --in-place --md-exts .md"
    Then the command succeeds
    And the file "docs/guide.md" has a reflowed table

  Scenario: Skip a tracked file deleted from the working tree
    Given the file "docs/guide.md" is deleted from the working tree
    And a committed file "other.md" with a broken table
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And the file "other.md" has a reflowed table

  Scenario: Report a candidate that cannot be classified
    Given the directory "docs" is replaced by a regular file
    When I run mdtablefix with "--git --list-files"
    Then the command exits with status 2
    And stdout is empty
    And stderr contains "guide.md"
    And stderr contains "while selecting files"

  Scenario: Refuse to rewrite a conflicted file mid-merge
    Given an unresolved merge conflict in the tracked file "docs/guide.md"
    When I run mdtablefix with "--git --in-place"
    Then the command fails
    And the file "docs/guide.md" is unchanged
    And stderr contains "conflict markers"

  Scenario: Rewrite a conflicted file when explicitly allowed
    Given an unresolved merge conflict in the tracked file "docs/guide.md"
    When I run mdtablefix with "--git --in-place --allow-conflicted"
    Then the command succeeds
    And the file "docs/guide.md" is rewritten with its conflict markers intact

  Scenario: Report drift across the repository without changing it
    When I run mdtablefix with "--git --check"
    Then the exit status is 1
    And stdout names "docs/guide.md"
    And the file "docs/guide.md" is unchanged

  Scenario: Report a diff across the repository without changing it
    When I run mdtablefix with "--git --diff"
    Then the exit status is 1
    And stdout contains "--- docs/guide.md"
    And stdout contains "| A   | B   |"
    And the file "docs/guide.md" is unchanged

  Scenario: Report a clean repository
    Given every tracked Markdown file is already formatted
    When I run mdtablefix with "--git --check"
    Then the exit status is 0

  Scenario: Exit successfully when nothing is selected
    Given a Git repository containing only the committed file "src/lib.rs"
    When I run mdtablefix with "--git --in-place"
    Then the command succeeds
    And stdout is empty
    And standard input was not read

  Scenario: Scope the selection to the current directory
    When I run mdtablefix from "docs" with "--git --list-files"
    Then the command succeeds
    And stdout is exactly "guide.md"

  Scenario: Report a clear error outside a Git repository
    Given the working directory is not inside a Git repository
    When I run mdtablefix with "--git"
    Then the command fails
    And stderr contains "git ls-files"

  Scenario: Reject an unusable extension
    When I run mdtablefix with "--git --md-exts md,,markdown"
    Then the command exits with status 2
    And stdout is empty
    And stderr contains "extension is empty"

  Scenario: Reject combining --git with explicit file arguments
    When I run mdtablefix with "--git notes.md"
    Then the command exits with status 2
    And stdout is empty

  Scenario: Reject --list-files without --git
    When I run mdtablefix with "--list-files notes.md"
    Then the command exits with status 2
    And stdout is empty
    And stderr contains "--list-files requires --git"
