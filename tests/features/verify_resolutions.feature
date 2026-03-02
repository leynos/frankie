Feature: Automated resolution verification

  Scenario: Verification marks changed lines as verified and persists results
    Given a migrated database for verification
    And a local repository where the referenced line changes between commits
    And the GitHub API returns a review comment pointing at the old commit
    When the user runs resolution verification
    Then the CLI output marks the comment as verified
    And the verification status is persisted in the local cache

  Scenario: Verification marks unchanged lines as unverified and persists results
    Given a migrated database for verification
    And a local repository where the referenced line is unchanged between commits
    And the GitHub API returns a review comment pointing at the old commit
    When the user runs resolution verification
    Then the CLI output marks the comment as unverified
    And the verification status is persisted in the local cache

