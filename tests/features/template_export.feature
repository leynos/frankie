Feature: Template-driven comment export

  Scenario: Export with simple template renders all placeholders
    Given a mock GitHub API server with 2 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    And a template "{% for c in comments %}{{ c.reviewer }}: {{ c.body }}\n{% endfor %}"
    When the client exports comments using the template for "http://SERVER/owner/repo/pull/42"
    Then the output contains "reviewer1"
    And the output contains "reviewer2"
    And the output contains "Review comment 1"
    And the output contains "Review comment 2"

  Scenario: Template with document-level variables
    Given a mock GitHub API server with 1 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    And a template "PR: {{ pr_url }}\nCount: {{ comments | length }}"
    When the client exports comments using the template for "http://SERVER/owner/repo/pull/42"
    Then the output contains "PR: http://"
    And the output contains "owner/repo/pull/42"
    And the output contains "Count: 1"

  Scenario: Template renders file and line placeholders
    Given a mock GitHub API server with 1 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    And a template "{% for c in comments %}{{ c.file }}:{{ c.line }}{% endfor %}"
    When the client exports comments using the template for "http://SERVER/owner/repo/pull/42"
    Then the output contains "src/file1.rs:10"

  Scenario: Status placeholder shows reply for threaded comments
    Given a mock GitHub API server with a reply comment for owner/repo/pull/42
    And a personal access token "valid-token"
    And a template "{% for c in comments %}Status: {{ c.status }}{% endfor %}"
    When the client exports comments using the template for "http://SERVER/owner/repo/pull/42"
    Then the output contains "Status: reply"

  Scenario: Status placeholder shows comment for root comments
    Given a mock GitHub API server with 1 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    And a template "{% for c in comments %}Status: {{ c.status }}{% endfor %}"
    When the client exports comments using the template for "http://SERVER/owner/repo/pull/42"
    Then the output contains "Status: comment"

  Scenario: Empty comment list with template produces document-only output
    Given a mock GitHub API server with 0 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    And a template "Header\n{% for c in comments %}{{ c.body }}{% endfor %}Footer"
    When the client exports comments using the template for "http://SERVER/owner/repo/pull/42"
    Then the output contains "Header"
    And the output contains "Footer"

  Scenario: Invalid template syntax produces error
    Given a mock GitHub API server with 1 review comments for owner/repo/pull/42
    And a personal access token "valid-token"
    And a template "{% for x in %}broken{% endfor %}"
    When the client exports comments using the template for "http://SERVER/owner/repo/pull/42"
    Then the error indicates invalid template syntax
