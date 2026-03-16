Feature: Time-travel parameter extraction from review comments

  Scenario: Derive parameters from a complete review comment
    Given a review comment with commit SHA "abc123" and file path "src/main.rs"
    And the comment has line number 42 and original line number 40
    When time-travel parameters are extracted
    Then extraction succeeds
    And the commit SHA is "abc123"
    And the file path is "src/main.rs"
    And the line number is 42

  Scenario: Fall back to original line when current line is missing
    Given a review comment with commit SHA "abc123" and file path "src/main.rs"
    And the comment has no current line number but original line number 40
    When time-travel parameters are extracted
    Then extraction succeeds
    And the line number is 40

  Scenario: Fail when the commit SHA is missing
    Given a review comment without a commit SHA
    When time-travel parameters are extracted
    Then extraction fails with a missing commit SHA error

  Scenario: Fail when the file path is missing
    Given a review comment with commit SHA "abc123" but no file path
    When time-travel parameters are extracted
    Then extraction fails with a missing file path error
