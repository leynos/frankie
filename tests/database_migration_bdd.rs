//! Behavioural tests for local database migrations and schema telemetry.

mod support;

use std::path::Path;

use frankie::persistence::{CURRENT_SCHEMA_VERSION, PersistenceError, migrate_database};
use frankie::telemetry::TelemetryEvent;
use frankie::telemetry::test_support::RecordingTelemetrySink;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use tempfile::TempDir;

use support::create_temp_dir;

#[derive(ScenarioState, Default)]
struct MigrationState {
    database_url: Slot<String>,
    temp_dir: Slot<TempDir>,
    schema_version: Slot<String>,
    error: Slot<PersistenceError>,
    telemetry: Slot<RecordingTelemetrySink>,
}

#[fixture]
fn migration_state() -> MigrationState {
    MigrationState::default()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

// --- Given steps ---

#[given("an in-memory database")]
fn in_memory_database(migration_state: &MigrationState) {
    migration_state.database_url.set(":memory:".to_owned());
}

#[given("a blank database URL")]
fn blank_database_url(migration_state: &MigrationState) {
    migration_state.database_url.set("   ".to_owned());
}

#[given("a directory database path")]
fn directory_database_path(migration_state: &MigrationState) {
    let temp_dir = create_temp_dir()
        .unwrap_or_else(|error| panic!("failed to create temporary directory: {error}"));
    let database_url = path_to_string(temp_dir.path());
    migration_state.temp_dir.set(temp_dir);
    migration_state.database_url.set(database_url);
}

#[given("a temporary database file")]
fn temporary_database_file(migration_state: &MigrationState) {
    let temp_dir = create_temp_dir()
        .unwrap_or_else(|error| panic!("failed to create temporary directory: {error}"));
    let db_path = temp_dir.path().join("frankie.sqlite");
    let database_url = path_to_string(&db_path);
    migration_state.temp_dir.set(temp_dir);
    migration_state.database_url.set(database_url);
}

#[given("a telemetry sink")]
fn telemetry_sink(migration_state: &MigrationState) {
    migration_state
        .telemetry
        .set(RecordingTelemetrySink::default());
}

// --- When steps ---

#[expect(clippy::expect_used, reason = "test code; panics are acceptable")]
#[when("database migrations are run")]
fn run_migrations(migration_state: &MigrationState) {
    let telemetry = migration_state
        .telemetry
        .with_ref(Clone::clone)
        .expect("telemetry sink not initialised");

    let database_url = migration_state
        .database_url
        .with_ref(Clone::clone)
        .expect("database URL not initialised");

    match migrate_database(&database_url, &telemetry) {
        Ok(version) => {
            migration_state
                .schema_version
                .set(version.as_str().to_owned());
        }
        Err(error) => {
            migration_state.error.set(error);
        }
    }
}

#[when("database migrations are run again")]
fn run_migrations_again(migration_state: &MigrationState) {
    run_migrations(migration_state);
}

// --- Then steps ---

#[expect(clippy::expect_used, reason = "test code; panics are acceptable")]
#[then("the schema version is {expected}")]
fn schema_version_is(migration_state: &MigrationState, expected: String) {
    let expected_clean = expected.trim_matches('"');

    let actual = migration_state
        .schema_version
        .with_ref(Clone::clone)
        .expect("schema version missing");

    assert_eq!(actual, expected_clean, "schema version mismatch");
}

#[expect(clippy::expect_used, reason = "test code; panics are acceptable")]
#[then("telemetry records the schema version")]
fn telemetry_records_schema_version(migration_state: &MigrationState) {
    let events = migration_state
        .telemetry
        .with_ref(RecordingTelemetrySink::events)
        .expect("telemetry sink not initialised");

    let Some(TelemetryEvent::SchemaVersionRecorded { schema_version }) = events.first() else {
        panic!("expected SchemaVersionRecorded event, got {events:?}");
    };

    assert!(
        !schema_version.is_empty(),
        "schema_version should not be empty in telemetry"
    );
}

#[expect(clippy::expect_used, reason = "test code; panics are acceptable")]
#[then("telemetry records the schema version twice")]
fn telemetry_records_schema_version_twice(migration_state: &MigrationState) {
    let events = migration_state
        .telemetry
        .with_ref(RecordingTelemetrySink::events)
        .expect("telemetry sink not initialised");

    let schema_versions: Vec<&str> = events
        .iter()
        .map(|event| match event {
            TelemetryEvent::SchemaVersionRecorded { schema_version } => schema_version.as_str(),
        })
        .collect();

    assert_eq!(
        schema_versions.len(),
        2,
        "expected exactly two SchemaVersionRecorded events, got {events:?}"
    );

    assert_eq!(
        schema_versions.first(),
        schema_versions.get(1),
        "expected idempotent migration to record the same schema_version twice"
    );

    assert_eq!(
        schema_versions.first().copied(),
        Some(CURRENT_SCHEMA_VERSION),
        "expected recorded schema_version to match CURRENT_SCHEMA_VERSION"
    );
}

#[expect(clippy::expect_used, reason = "test code; panics are acceptable")]
#[then("a persistence error {expected} is reported")]
fn persistence_error_is(migration_state: &MigrationState, expected: String) {
    let expected_clean = expected.trim_matches('"');

    let error = migration_state
        .error
        .with_ref(Clone::clone)
        .expect("expected persistence error");

    assert_eq!(error.to_string(), expected_clean);
}

#[expect(clippy::expect_used, reason = "test code; panics are acceptable")]
#[then("a persistence error starts with {expected_prefix}")]
fn persistence_error_starts_with(migration_state: &MigrationState, expected_prefix: String) {
    let expected_clean = expected_prefix.trim_matches('"');

    let error = migration_state
        .error
        .with_ref(Clone::clone)
        .expect("expected persistence error");

    assert!(
        error.to_string().starts_with(expected_clean),
        "expected error to start with {expected_clean:?}, got {error}"
    );
}

#[expect(clippy::expect_used, reason = "test code; panics are acceptable")]
#[then("no telemetry is recorded")]
fn no_telemetry_is_recorded(migration_state: &MigrationState) {
    let events = migration_state
        .telemetry
        .with_ref(RecordingTelemetrySink::events)
        .expect("telemetry sink not initialised");

    assert!(
        events.is_empty(),
        "expected no telemetry events, got {events:?}"
    );
}

#[scenario(path = "tests/features/database_migration.feature", index = 0)]
fn migrations_record_schema_version(migration_state: MigrationState) {
    let _ = migration_state;
}

#[scenario(path = "tests/features/database_migration.feature", index = 1)]
fn migrations_fail_on_blank_database_url(migration_state: MigrationState) {
    let _ = migration_state;
}

#[scenario(path = "tests/features/database_migration.feature", index = 2)]
fn migrations_fail_on_directory_path(migration_state: MigrationState) {
    let _ = migration_state;
}

#[scenario(path = "tests/features/database_migration.feature", index = 3)]
fn migrations_are_idempotent(migration_state: MigrationState) {
    let _ = migration_state;
}
