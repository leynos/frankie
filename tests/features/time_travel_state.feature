Feature: Public time-travel state API

  Scenario: Construct a time-travel state and read public accessors
    Given a snapshot for commit SHA "abc1234567890" with message "Fix login validation"
    And the snapshot contains file content "fn login() {\n    validate();\n}"
    And the file path is "src/auth.rs"
    And the original line is 2
    And the line mapping is an exact match for line 2
    And the commit history is "abc1234567890", "def5678901234", and "ghi9012345678"
    And the current history index is 0
    When the time-travel state is constructed
    Then the snapshot SHA is "abc1234567890"
    And the snapshot message is "Fix login validation"
    And the public file path is "src/auth.rs"
    And the public original line is 2
    And the state exposes an exact line mapping for line 2
    And the state reports 3 commits in history
    And the current index is 0
    And previous navigation is available
    And next navigation is unavailable
    And the previous commit SHA is "def5678901234"
    And no next commit SHA is available

  Scenario: Inspect navigation from the middle of commit history
    Given a snapshot for commit SHA "def5678901234" with message "Refactor validation"
    And the snapshot contains file content "fn login() {\n    validate();\n}"
    And the file path is "src/auth.rs"
    And there is no original line
    And there is no line mapping
    And the commit history is "abc1234567890", "def5678901234", and "ghi9012345678"
    And the current history index is 1
    When the time-travel state is constructed
    Then the state reports 3 commits in history
    And the current index is 1
    And the next commit SHA is "abc1234567890"
    And the previous commit SHA is "ghi9012345678"

  Scenario: Update a snapshot and clamp the requested index
    Given a snapshot for commit SHA "abc1234567890" with message "Fix login validation"
    And the snapshot contains file content "fn login() {\n    validate();\n}"
    And the file path is "src/auth.rs"
    And there is no original line
    And there is no line mapping
    And the commit history is "abc1234567890", "def5678901234", and "ghi9012345678"
    And the current history index is 0
    When the time-travel state is constructed
    And the snapshot is updated to SHA "ghi9012345678" with message "Initial implementation" and index 99
    Then the snapshot SHA is "ghi9012345678"
    And the snapshot message is "Initial implementation"
    And the current index is 2
