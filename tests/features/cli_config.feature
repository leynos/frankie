Feature: CLI configuration loading

  Configuration values can be provided via CLI flags, environment variables,
  or configuration files. Higher precedence sources override lower ones.

  Scenario: Load PR URL from command line flag
    Given a configuration with no pr_url set
    When the CLI receives pr_url "https://github.com/owner/repo/pull/1"
    Then the configuration pr_url is "https://github.com/owner/repo/pull/1"

  Scenario: Load token from command line flag
    Given a configuration with no token set
    When the CLI receives token "cli-token"
    Then the resolved token is "cli-token"

  Scenario: CLI pr_url overrides environment
    Given a configuration with environment pr_url "env-url"
    When the CLI receives pr_url "cli-url"
    Then the configuration pr_url is "cli-url"

  Scenario: CLI token overrides environment
    Given a configuration with environment token "env-token"
    When the CLI receives token "cli-token"
    Then the resolved token is "cli-token"

  Scenario: Environment token used when CLI not provided
    Given a configuration with environment token "env-token"
    When the CLI receives no token
    Then the resolved token is "env-token"

  Scenario: Missing PR URL produces error
    Given a configuration with no pr_url set
    When the CLI receives no pr_url
    Then requiring pr_url returns an error

  Scenario: Missing token produces error
    Given a configuration with no token set
    And no GITHUB_TOKEN environment variable
    When the CLI receives no token
    Then resolving token returns an error

  Scenario: GITHUB_TOKEN fallback when no other token
    Given a configuration with no token set
    And a GITHUB_TOKEN environment variable set to "legacy-token"
    When the CLI receives no token
    Then the resolved token is "legacy-token"

  Scenario: FRANKIE_TOKEN takes precedence over GITHUB_TOKEN
    Given a configuration with environment token "frankie-token"
    And a GITHUB_TOKEN environment variable set to "legacy-token"
    When the CLI receives no token
    Then the resolved token is "frankie-token"
