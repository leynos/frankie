Feature: Local repository discovery

  Scenario: Discover repository from SSH origin
    Given a Git repository with origin "git@github.com:octo/repo.git"
    When the discovery is performed
    Then the owner is detected as "octo"
    And the repository is detected as "repo"
    And the API base is "https://api.github.com/"

  Scenario: Discover repository from HTTPS origin
    Given a Git repository with origin "https://github.com/owner/project.git"
    When the discovery is performed
    Then the owner is detected as "owner"
    And the repository is detected as "project"

  Scenario: Warn when origin is missing
    Given a Git repository with no remotes
    When the discovery is performed
    Then the discovery fails with no remotes error

  Scenario: Warn when origin is not parseable
    Given a Git repository with origin "not-a-valid-url"
    When the discovery is performed
    Then the discovery fails with invalid remote URL error

  Scenario: Discover from GitHub Enterprise origin
    Given a Git repository with origin "git@ghe.example.com:org/project.git"
    When the discovery is performed
    Then the owner is detected as "org"
    And the repository is detected as "project"
    And the API base is "https://ghe.example.com/api/v3"
