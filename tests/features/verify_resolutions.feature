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

  Scenario: Verification marks deleted lines as verified and persists results
    Given a migrated database for verification
    And a local repository where the referenced line is deleted between commits
    And the GitHub API returns a review comment pointing at the old commit
    When the user runs resolution verification
    Then the CLI output marks the comment as verified
    And the verification status is persisted in the local cache

  Scenario: Verification marks unknown commit mappings as unverified
    Given a migrated database for verification
    And a local repository where the referenced line changes between commits
    And the GitHub API returns a review comment pointing at an unknown commit
    When the user runs resolution verification
    Then the CLI output marks the comment as unverified
    And the CLI output explains repository data is unavailable

  Scenario: Verification cache reuse keeps a single row per comment and target
    Given a migrated database for verification
    And a local repository where the referenced line changes between commits
    And the GitHub API returns a review comment pointing at the old commit
    When the user runs resolution verification twice
    Then the CLI output marks the comment as verified
    And the verification status is persisted in the local cache
    And the cache contains one verification row for the comment and target

  Scenario: Verification accepts a bare PR number when --repo-path is provided
    Given a migrated database for verification
    And a local repository where the referenced line changes between commits
    And the GitHub API returns a review comment pointing at the old commit
    When the user runs resolution verification with a positional PR number
    Then the CLI output marks the comment as verified
    And the verification status is persisted in the local cache
