Feature: Codex session resumption

  Scenario: Resume prompt is shown for interrupted session
    Given an interrupted Codex session is detected
    Then the status bar shows a resume prompt

  Scenario: Accepting resume starts a resumed execution
    Given an interrupted Codex session is detected
    When the user accepts the resume prompt
    And I wait 200 milliseconds
    And the Codex poll tick is processed
    Then the status bar contains "Codex execution completed"
    And no TUI error is shown

  Scenario: Declining resume starts a fresh execution
    Given an interrupted Codex session is detected
    When the user declines the resume prompt
    And I wait 200 milliseconds
    And the Codex poll tick is processed
    Then the status bar contains "Codex execution completed"
    And no TUI error is shown

  Scenario: No resume prompt when no interrupted session exists
    Given a Codex run that streams progress and completes successfully
    When Codex execution is started from the review TUI
    And I wait 200 milliseconds
    And the Codex poll tick is processed
    Then the status bar contains "Codex execution completed"
    And no TUI error is shown
