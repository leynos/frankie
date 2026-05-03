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

  Scenario: Invalid template syntax surfaces a readable inline error
    Given a review TUI with one comment, max length 120, and template "{{ reviewer"
    When the user presses "a"
    And the user presses "1"
    Then the TUI error contains "Reply template rendering failed:"

  Scenario: Escaped braces and template-like body text remain literal
    Given a review TUI with one comment body "Please keep {{ nested }} literal", max length 200, and template "{% raw %}{{ reviewer }}{% endraw %} :: {{ body }}"
    When the user presses "a"
    And the user presses "1"
    And the view is rendered
    Then the view contains "{{ reviewer }} :: Please keep {{ nested }} literal"
    And no TUI error is shown

  Scenario: Built-in reply template defaults render through the TUI
    Given a review TUI with one comment and built-in reply-template defaults
    When the user presses "a"
    And the user presses "1"
    And the view is rendered
    Then the view contains "Thanks for the review on src/main.rs:12. I will update this."
    And no TUI error is shown

  Scenario: Built-in reply template slot 2 renders through the TUI
    Given a review TUI with one comment and built-in reply-template defaults
    When the user presses "a"
    And the user presses "2"
    And the view is rendered
    Then the view contains "Good catch, alice. I will address this in the next commit."
    And no TUI error is shown

  Scenario: Built-in reply template slot 3 renders through the TUI
    Given a review TUI with one comment and built-in reply-template defaults
    When the user presses "a"
    And the user presses "3"
    And the view is rendered
    Then the view contains "I have addressed this feedback and pushed an update."
    And no TUI error is shown
