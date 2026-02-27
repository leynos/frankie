Feature: AI reply draft rewrite

  Scenario: AI expand generates preview and can be applied
    Given a review TUI with AI rewrite succeeding to "Expanded response"
    When the user starts reply drafting and types "ok"
    And the user requests AI "expand" rewrite
    And the AI rewrite command is executed
    And the view is rendered
    Then the view contains "AI rewrite preview (expand):"
    And the view contains "Origin: AI-originated"
    When the user applies the AI preview
    And the view is rendered
    Then the view contains "Expanded response"
    And the view contains "Origin: AI-originated"

  Scenario: AI rewrite failure falls back gracefully
    Given a review TUI with AI rewrite failing with "timeout"
    When the user starts reply drafting and types "ok"
    And the user requests AI "reword" rewrite
    And the AI rewrite command is executed
    And the view is rendered
    Then the TUI error contains "AI request failed"
    And the view contains "ok"

  Scenario: AI preview can be discarded without mutating draft text
    Given a review TUI with AI rewrite succeeding to "Alternative wording"
    When the user starts reply drafting and types "original"
    And the user requests AI "reword" rewrite
    And the AI rewrite command is executed
    And the user discards the AI preview
    And the view is rendered
    Then the view does not contain "AI rewrite preview (reword):"
    And the view contains "original"
