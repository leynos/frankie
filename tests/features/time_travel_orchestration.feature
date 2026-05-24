Feature: Public time-travel orchestration

  Scenario: Load initial state from comment metadata
    Given a time-travel request for commit "abc1234567890" and file "src/main.rs"
    And the requested original line is 10
    And the head SHA is "HEAD1234567890"
    And git loads snapshot "abc1234567890" with message "Newest commit"
    And git returns commit history "abc1234567890", "def5678901234", and "ghi9012345678"
    And git reports an exact line mapping for line 10
    When the initial time-travel state is loaded
    Then the loaded state snapshot SHA is "abc1234567890"
    And the loaded state index is 0
    And the loaded history count is 3
    And the loaded line mapping is exact for line 10

  Scenario: Navigate to an older commit
    Given a loaded time-travel state at history index 0
    And the head SHA is "HEAD1234567890"
    And git loads snapshot "def5678901234" with message "Middle commit"
    When the state is navigated to the previous commit
    Then navigation returns snapshot SHA "def5678901234"
    And navigation returns history index 1

  Scenario: Navigate back to a newer commit
    Given a loaded time-travel state at history index 1
    And git loads snapshot "abc1234567890" with message "Newest commit"
    When the state is navigated to the next commit
    Then navigation returns snapshot SHA "abc1234567890"
    And navigation returns history index 0

  Scenario: Boundary navigation returns no state
    Given a loaded time-travel state at history index 0
    And git loads snapshot "def5678901234" with message "Middle commit"
    When the state is navigated to the next commit
    Then navigation returns no state
    And no git snapshot load is attempted

  Scenario: Navigation surfaces a missing commit unchanged
    Given a loaded time-travel state at history index 0
    And git fails to load snapshot "def5678901234" because the commit is missing
    When the state is navigated to the previous commit
    Then navigation fails with a missing commit for "def5678901234"

  Scenario: Navigation skips line mapping when head SHA is absent
    Given a loaded time-travel state without an original line at history index 0
    And no head SHA is available
    And git loads snapshot "def5678901234" with message "Middle commit"
    When the state is navigated to the previous commit
    Then navigation returns snapshot SHA "def5678901234"
    And the navigated state has no line mapping
