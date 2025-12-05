Feature: Pull request intake

  Scenario: Load metadata and comments for a valid pull request URL
    Given a mock GitHub API server with pull request 42 titled Add search and 2 comments
    And a personal access token "valid-token"
    When the client loads pull request "http://SERVER/owner/repo/pull/42"
    Then the response includes the title "Add search"
    And the response includes 2 comments

  Scenario: Surface authentication errors
    Given a mock GitHub API server that rejects token for pull request 7
    And a personal access token "bad-token"
    When the client loads pull request "http://SERVER/owner/repo/pull/7"
    Then the error message mentions authentication failure
