Feature: Incremental review sync

  Background:
    Given a recording telemetry sink

  Scenario: Background sync merges new comments without losing selection
    Given a TUI with 3 review comments
    And the cursor is on comment 2
    When a sync completes with 4 comments including comment 2
    Then the cursor remains on comment 2
    And the filtered count is 4

  Scenario: Background sync clamps cursor when selected comment deleted
    Given a TUI with 3 review comments
    And the cursor is on comment 3
    When a sync completes with 2 comments without comment 3
    Then the cursor is on comment 2
    And the filtered count is 2

  Scenario: Sync latency is logged to telemetry
    Given a TUI with 2 review comments
    When a sync completes in 200ms with 3 comments
    Then a SyncLatencyRecorded event is logged
    And the event shows latency_ms 200
    And the event shows comment_count 3
    And the event shows incremental true
