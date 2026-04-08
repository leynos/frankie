Feature: Configurable commit history limit

  Scenario: Default commit history limit uses 50 commits
    Given a git operations mock expecting a commit history limit of 50
    And a comment with SHA "abc1234567890" and file "src/main.rs"
    When the time-travel state is loaded with the default limit
    Then the loaded history contains 3 commits

  Scenario: Overridden commit history limit is respected
    Given a git operations mock expecting a commit history limit of 5
    And a comment with SHA "abc1234567890" and file "src/main.rs"
    When the time-travel state is loaded with a limit of 5
    Then the loaded history contains 3 commits

  Scenario: Minimum limit of 1 produces a single-entry history
    Given a git operations mock expecting a commit history limit of 1
    And a comment with SHA "abc1234567890" and file "src/main.rs"
    When the time-travel state is loaded with a limit of 1
    Then the loaded history contains 1 commits

  Scenario: Commit history limit of 0 is clamped to 1
    Given a git operations mock expecting a commit history limit of 1
    And a comment with SHA "abc1234567890" and file "src/main.rs"
    When the time-travel state is loaded with a limit of 0
    Then the loaded history contains 1 commits
