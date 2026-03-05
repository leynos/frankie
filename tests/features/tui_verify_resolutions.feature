Feature: TUI resolution verification

  Scenario: Verifying a selected comment annotates the review list
    Given a review TUI with verification cache configured returning "verified"
    When the user requests verification for the selected comment
    And the verification command completes
    Then the review list shows the comment as verified

  Scenario: Verification requires a configured cache
    Given a review TUI with no verification cache
    When the user requests verification for the selected comment
    Then an error is shown explaining the missing database
