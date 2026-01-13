Feature: Comment detail view with inline code context

  Scenario: Comment detail displays author and file path
    Given a TUI with a review comment by alice on src/main.rs at line 42
    When the view is rendered
    Then the detail pane shows author alice
    And the detail pane shows file path src/main.rs
    And the detail pane shows line number 42

  Scenario: Comment detail displays body text
    Given a TUI with a review comment with body "Please refactor this"
    When the view is rendered
    Then the detail pane shows the body text

  Scenario: Comment detail displays code context from diff hunk
    Given a TUI with a review comment with a diff hunk
    When the view is rendered
    Then the detail pane shows code context

  Scenario: Code context wraps long lines to 80 columns
    Given a TUI with a review comment with a 120-character code line
    When the view is rendered with max width 80
    Then all code lines are at most 80 characters wide

  Scenario: Fallback to plain text when highlighting fails
    Given a TUI with a review comment on a file with unknown extension
    When the view is rendered
    Then the code context is displayed as plain text
    And all code lines are at most 80 characters wide

  Scenario: Detail pane shows placeholder when no diff hunk
    Given a TUI with a review comment without diff hunk
    When the view is rendered
    Then the detail pane shows no-context placeholder

  Scenario: Detail pane shows placeholder when no comment selected
    Given a TUI with no comments
    When the view is rendered
    Then the detail pane shows no-selection placeholder
