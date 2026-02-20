Feature: Codex execution integration

  Scenario: Successful Codex run streams status and saves transcript
    Given a Codex run that streams progress and completes successfully
    When Codex execution is started from the review TUI
    And the Codex poll tick is processed
    Then the status bar contains "progress: event: turn.started"
    And no TUI error is shown
    When a wait of 200 milliseconds elapses
    And the Codex poll tick is processed
    Then the status bar contains "Codex execution completed"
    And the status bar contains "transcript:"
    And the transcript file exists
    And the transcript file contains "turn.started"

  Scenario: Non-zero Codex exit is surfaced in the TUI
    Given a Codex run that exits non-zero with transcript
    When Codex execution is started from the review TUI
    And a wait of 100 milliseconds elapses
    And the Codex poll tick is processed
    Then the TUI error contains "exit code: 17"
    And the TUI error contains "Transcript:"

  Scenario: Malformed stream line is surfaced without panic
    Given a Codex run that emits a malformed stream line
    When Codex execution is started from the review TUI
    And the Codex poll tick is processed
    Then the status bar contains "non-JSON event"

  Scenario: Transcript write failure is surfaced clearly
    Given a Codex run that fails because transcript writing failed
    When Codex execution is started from the review TUI
    And a wait of 100 milliseconds elapses
    And the Codex poll tick is processed
    Then the TUI error contains "failed to write transcript"
