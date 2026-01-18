Feature: Full-screen diff context

  Scenario: Open full-screen diff context
    Given a TUI with review comments that contain diff hunks
    When the full-screen diff context is opened
    And the view is rendered
    Then the view shows hunk position 1/2
    And the view shows file path src/main.rs

  Scenario: Jump to the next hunk
    Given a TUI with review comments that contain diff hunks
    When the full-screen diff context is opened
    And the next hunk is selected
    And the view is rendered
    Then the view shows hunk position 2/2

  Scenario: Jump to previous hunk at start
    Given a TUI with review comments that contain diff hunks
    When the full-screen diff context is opened
    And the previous hunk is selected
    And the view is rendered
    Then the view shows hunk position 1/2

  Scenario: Navigation keys are blocked in diff context
    Given a TUI with review comments that contain diff hunks
    When the full-screen diff context is opened
    And the next hunk is selected
    And a navigation key is pressed in diff context
    And the view is rendered
    Then the view shows hunk position 2/2

  Scenario: Filter keys are blocked in diff context
    Given a TUI with review comments that contain diff hunks
    When the full-screen diff context is opened
    And a filter key is pressed in diff context
    And the view is rendered
    Then the view shows hunk position 1/2

  Scenario: Placeholder when no diff hunks
    Given a TUI with review comments without diff hunks
    When the full-screen diff context is opened
    And the view is rendered
    Then the view shows no diff context placeholder

  Scenario: Exit diff context returns to list
    Given a TUI with review comments that contain diff hunks
    When the full-screen diff context is opened
    And the diff context is closed
    And the view is rendered
    Then the review list is visible

  Scenario: Exit diff context preserves selection
    Given a TUI with review comments that contain diff hunks
    And the second review comment is selected
    When the full-screen diff context is opened
    And the diff context is closed
    And the view is rendered
    Then the second review comment remains selected
