Feature: Report which Markdown files would be reformatted

  Scenario: A clean file reports no drift and succeeds
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--check" against those files
    Then the exit status is 0
    And standard output is empty
    And the summary reads "1 file left unchanged."
    And the working directory is byte-identical

  Scenario: A drifting file is reported with its line counts
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--check" against those files
    Then the exit status is 1
    And standard output is "ragged.md +3 -3"
    And the working directory is byte-identical

  Scenario: Every supplied file is reported in argument order
    Given a Markdown file "clean.md" that is already formatted
    And a Markdown file "zebra.md" with an unaligned table
    And a Markdown file "alpha.md" with an unaligned table
    When mdtablefix runs with "--check" against those files
    Then the exit status is 1
    And standard output lists "zebra.md" before "alpha.md"
    And the summary reads "2 files would be reformatted, 1 file left unchanged."

  Scenario: An unreadable file yields the error status, not the drift status
    Given a Markdown file "ragged.md" with an unaligned table
    And a path "missing.md" that does not exist
    When mdtablefix runs with "--check" against those files
    Then the exit status is 2
    And standard error mentions "missing.md"
    And the summary reports 1 file could not be read

  Scenario: A CRLF file needing no Markdown changes reports clean
    Given a Markdown file "windows.md" already formatted with CRLF endings
    When mdtablefix runs with "--check" against those files
    Then the exit status is 0
    And standard output is empty

  Scenario: A byte-order-marked ragged file is not reported as clean
    Given a Markdown file "bom.md" with a byte-order mark and an unaligned table
    When mdtablefix runs with "--check" against those files
    Then the exit status is 1
    And standard output is "bom.md +3 -3"

  Scenario: In-place formatting of a drifting file still succeeds
    Given a Markdown file "ragged.md" with an unaligned table
    When mdtablefix runs with "--in-place" against those files
    Then the exit status is 0
    And "ragged.md" is reformatted

  Scenario: Check mode rejects being combined with in-place mode
    Given a Markdown file "clean.md" that is already formatted
    When mdtablefix runs with "--check --in-place" against those files
    Then the exit status is 2
    And standard error mentions "cannot be used with"
