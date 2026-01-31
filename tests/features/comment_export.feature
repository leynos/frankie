Feature: Structured comment export

  Scenario: Export comments in Markdown format
    Given a mock GitHub API server with 2 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    When the client exports comments for "http://SERVER/owner/repo/pull/42" in markdown format
    Then the output has header "# Review Comments Export"
    And the output has PR URL containing "owner/repo/pull/42"
    And the output has 2 comment sections

  Scenario: Export comments in JSONL format
    Given a mock GitHub API server with 2 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    When the client exports comments for "http://SERVER/owner/repo/pull/42" in jsonl format
    Then the output has 2 JSON lines
    And each JSON line is valid JSON with an id field

  Scenario: Export with stable ordering by file and line
    Given a mock GitHub API server with comments in random order for owner/repo/pull/42
    And a personal access token "valid-token"
    When the client exports comments for "http://SERVER/owner/repo/pull/42" in jsonl format
    Then the first comment is for file "src/a.rs" line 10
    And the second comment is for file "src/a.rs" line 20
    And the third comment is for file "src/b.rs" line 5

  Scenario: Export empty comment list produces minimal output
    Given a mock GitHub API server with 0 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    When the client exports comments for "http://SERVER/owner/repo/pull/42" in markdown format
    Then the output has header "# Review Comments Export"
    And the output has 0 comment sections

  Scenario: Export empty comment list in JSONL produces empty output
    Given a mock GitHub API server with 0 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    When the client exports comments for "http://SERVER/owner/repo/pull/42" in jsonl format
    Then the output is empty

  Scenario: Invalid export format produces error
    Given a mock GitHub API server with 1 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    When the client exports comments for "http://SERVER/owner/repo/pull/42" in xml format
    Then the error indicates unsupported export format
