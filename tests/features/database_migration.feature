Feature: Database migrations

  Frankie uses Diesel migrations to manage its local SQLite schema. Migrations
  must apply cleanly, and the resulting schema version must be recorded in
  telemetry for troubleshooting.

  Scenario: Running migrations on an in-memory database records schema version
    Given an in-memory database
    When database migrations are run
    Then the schema version is "20251214000000"
    And telemetry records the schema version

  Scenario: Running migrations with a blank database URL fails fast
    Given a blank database URL
    When database migrations are run
    Then a persistence error "database URL must not be blank" is reported
    And no telemetry is recorded

  Scenario: Running migrations against a directory path fails
    Given a directory database path
    When database migrations are run
    Then a persistence error starts with "failed to connect to SQLite database:"
    And no telemetry is recorded

  Scenario: Running migrations twice is safe
    Given a temporary database file
    When database migrations are run
    And database migrations are run again
    Then the schema version is "20251214000000"
    And telemetry records the schema version twice
