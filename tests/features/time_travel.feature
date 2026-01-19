Feature: Time-travel navigation

  Scenario: Enter time-travel mode
    Given a TUI with review comments that have commit SHAs
    And a local repository is available
    When time-travel mode is entered for the selected comment
    And the view is rendered
    Then the view shows the time-travel header
    And the view shows the commit message
    And the view shows the file path

  Scenario: Display line mapping verification
    Given a TUI with review comments that have commit SHAs
    And a local repository is available
    And the line mapping shows exact match
    When time-travel mode is entered for the selected comment
    And the view is rendered
    Then the view shows line mapping status

  Scenario: Navigate to previous commit
    Given a TUI with review comments that have commit SHAs
    And a local repository is available
    And time-travel mode is entered for the selected comment
    When the previous commit is navigated to
    And the view is rendered
    Then the view shows commit position 2/3

  Scenario: Navigate to next commit
    Given a TUI with review comments that have commit SHAs
    And a local repository is available
    And time-travel mode is entered for the selected comment
    And the previous commit is navigated to
    When the next commit is navigated to
    And the view is rendered
    Then the view shows commit position 1/3

  Scenario: Handle missing commit gracefully
    Given a TUI with review comments that have commit SHAs
    And a local repository is available
    And the commit is not found in the repository
    When time-travel mode is entered for the selected comment
    And the view is rendered
    Then the view shows commit not found error

  Scenario: Handle missing local repository
    Given a TUI with review comments that have commit SHAs
    And no local repository is available
    When time-travel mode is entered for the selected comment
    And the view is rendered
    Then the view shows no repository error

  Scenario: Exit time-travel mode
    Given a TUI with review comments that have commit SHAs
    And a local repository is available
    And time-travel mode is entered for the selected comment
    When time-travel mode is exited
    And the view is rendered
    Then the review list is visible
