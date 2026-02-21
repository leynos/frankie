Feature: Template-based inline reply drafting

  Scenario: Start reply drafting and insert a template via keyboard
    Given a review TUI with one comment, max length 120, and template "Thanks {{ reviewer }}"
    When the user presses "a"
    And the user presses "1"
    And the view is rendered
    Then the view contains "Reply draft:"
    And the view contains "Thanks alice"
    And the view contains "/120"

  Scenario: Template content remains editable before send intent
    Given a review TUI with one comment, max length 120, and template "Ack"
    When the user presses "a"
    And the user presses "1"
    And the user presses "!"
    And the user presses "Enter"
    And the view is rendered
    Then the view contains "Ack!"
    And the view contains "ready to send"
    And no TUI error is shown

  Scenario: Template insertion is blocked when it exceeds configured length
    Given a review TUI with one comment, max length 5, and template "abcdef"
    When the user presses "a"
    And the user presses "1"
    And the view is rendered
    Then the TUI error contains "exceeds configured limit 5"
    And the view contains "(empty)"

  Scenario: Selecting an unconfigured template slot surfaces an error
    Given a review TUI with one comment, max length 120, and template "Template one"
    When the user presses "a"
    And the user presses "2"
    Then the TUI error contains "Reply template 2 is not configured"

  Scenario: Reply drafting requires a selected comment
    Given a review TUI with no comments and max length 120
    When the user presses "a"
    Then the TUI error contains "requires a selected comment"
