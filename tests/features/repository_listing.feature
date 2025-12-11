Feature: Repository pull request listing

  Scenario: List pull requests for a repository
    Given a mock GitHub API server with 50 open PRs for owner/repo
    And a personal access token "valid-token"
    When the client lists pull requests for "http://SERVER/owner/repo" page 1
    Then the response includes 50 pull requests
    And the current page is 1

  Scenario: Paginate through multiple pages of PRs
    Given a mock GitHub API server with 150 PRs across 3 pages for owner/repo
    And a personal access token "valid-token"
    When the client lists pull requests for "http://SERVER/owner/repo" page 2
    Then the response includes 50 pull requests
    And the pagination indicates page 2 of 3
    And pagination has next page
    And pagination has previous page

  Scenario: Handle rate limit headers gracefully
    Given a mock GitHub API server with rate limit headers showing 100 remaining
    And a personal access token "valid-token"
    When the client lists pull requests for "http://SERVER/owner/repo" page 1
    Then no error is raised

  Scenario: Handle rate limit exhaustion without panic
    Given a mock GitHub API server returning 403 rate limit exceeded
    And a personal access token "valid-token"
    When the client lists pull requests for "http://SERVER/owner/repo" page 1
    Then the error indicates rate limit exceeded
