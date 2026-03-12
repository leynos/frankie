Feature: CLI PR discussion summary

  Scenario: Summary mode groups comments by file and excludes reply duplicates
    Given the GitHub API returns review comments with replies and general discussion
    And a VidaiMock summary server is available
    When the user runs PR discussion summary mode
    Then the command exits successfully
    And stdout contains "File: src/main.rs"
    And stdout contains "Severity: high"
    And stdout contains "Link: frankie://review-comment/1?view=detail"
    And stdout does not contain "review-comment/2?view=detail"
    And stdout contains "File: (general discussion)"

  Scenario: Summary mode rejects empty review-comment sets
    Given the GitHub API returns no review comments
    When the user runs PR discussion summary mode
    Then the command fails with stderr containing "requires at least one review comment"
