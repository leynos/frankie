Feature: TUI PR discussion summary

  Scenario: Generating a PR discussion summary opens the summary view and can jump to the linked comment
    Given a review TUI with PR discussion summary succeeding
    When the user requests a PR discussion summary
    And the PR discussion summary command is executed
    And the summary view is rendered
    Then the summary view contains "File: src/main.rs"
    And the summary view contains "Severity: high"
    And the summary view contains "frankie://review-comment/1?view=detail"
    When the user opens the selected summary link
    And the summary view is rendered
    Then the selected comment id is 1
    And the summary view contains "[alice] src/main.rs:12"

  Scenario: PR discussion summary failure surfaces an error
    Given a review TUI with PR discussion summary failing with "timeout"
    When the user requests a PR discussion summary
    And the PR discussion summary command is executed
    Then the TUI summary error contains "timeout"
    When the summary view is rendered
    Then the summary view does not contain "Severity: high"
