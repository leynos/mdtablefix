Feature: Show what would change in Markdown files

  Scenario: A drifting file produces a unified diff and fails
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--diff" against those files
    Then the exit status is 1
    And the diff header names "ragged.md" on both sides
    And the diff contains a hunk header
    And the working directory is byte-identical

  Scenario: A clean file produces no diff
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--diff" against those files
    Then the exit status is 0
    And standard output is empty

  Scenario: A drifting file under in-place formatting still succeeds
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--in-place" against those files
    Then the exit status is 0

  Scenario: Diff output is byte-identical across repeated runs
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--diff" against those files ten times
    Then every run produced identical standard output
    And the diff contains a hunk header

  Scenario: An unreadable file yields the error status
    Given a path "missing.md" that does not exist
    When mdtablefix runs with "--diff" against those files
    Then the exit status is 2
    And standard error mentions "missing.md"

  Scenario: Diff mode rejects being combined with check mode
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--diff --check" against those files
    Then the exit status is 2
    And standard error mentions "cannot be used with"
