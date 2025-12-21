Feature: Pull request metadata caching

  Frankie can optionally cache pull request metadata in the local SQLite
  database when `--database-url` is configured. Cached data is reused across
  sessions and revalidated using ETag / Last-Modified once the TTL expires.

  Scenario: Fresh cache avoids refetching pull request metadata
    Given a temporary database file with migrations applied
    And a cache TTL of 86400 seconds
    And a mock GitHub API server that serves pull request 42 titled Add cache without validators with 0 comments
    And a personal access token "valid-token"
    When the cached client loads pull request "http://SERVER/owner/repo/pull/42" for the first time
    And the cached client loads pull request "http://SERVER/owner/repo/pull/42" again
    Then the response includes the title "Add cache"
    And the GitHub API mocks are satisfied

  Scenario: Expired cache revalidates via ETag and Last-Modified
    Given a temporary database file with migrations applied
    And a cache TTL of 0 seconds
    And a mock GitHub API server that serves pull request 7 titled Revalidate with ETag "etag-1" and Last-Modified "Mon, 01 Jan 2025 00:00:00 GMT" with 0 comments
    And a personal access token "valid-token"
    When the cached client loads pull request "http://SERVER/owner/repo/pull/7" for the first time
    And the cached client loads pull request "http://SERVER/owner/repo/pull/7" again
    Then the response includes the title "Revalidate"
    And the revalidation request includes If-None-Match "etag-1" and If-Modified-Since "Mon, 01 Jan 2025 00:00:00 GMT"
    And the GitHub API mocks are satisfied

  Scenario: Changed ETag invalidates cached pull request metadata
    Given a temporary database file with migrations applied
    And a cache TTL of 0 seconds
    And a mock GitHub API server that updates pull request 9 from title Old to title New with ETag "etag-1" then "etag-2" and 0 comments
    And a personal access token "valid-token"
    When the cached client loads pull request "http://SERVER/owner/repo/pull/9" for the first time
    And the cached client loads pull request "http://SERVER/owner/repo/pull/9" again
    Then the response includes the title "New"
    And the GitHub API mocks are satisfied

  Scenario: Cache requires an initialised database schema
    Given a temporary database file without migrations
    And a cache TTL of 86400 seconds
    And a personal access token "valid-token"
    When the cached client loads pull request "https://github.com/owner/repo/pull/1" for the first time
    Then a configuration error mentions running migrations

  Scenario: 304 on uncached pull request returns an API error
    Given a temporary database file with migrations applied
    And a cache TTL of 0 seconds
    And a mock GitHub API server that returns 304 Not Modified for pull request 11 with 0 comments
    And a personal access token "valid-token"
    When the cached client loads pull request "http://SERVER/owner/repo/pull/11" for the first time
    Then an API error mentions an unexpected 304 response
    And the GitHub API mocks are satisfied
