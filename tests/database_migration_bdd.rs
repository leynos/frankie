//! Behavioural tests for local database migrations and schema telemetry.

use std::path::Path;
use std::sync::{Arc, Mutex};

use frankie::persistence::{PersistenceError, migrate_database};
use frankie::telemetry::{TelemetryEvent, TelemetrySink};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use tempfile::TempDir;

#[derive(Debug, Clone, Default)]
struct RecordingTelemetrySink {
    events: Arc<Mutex<Vec<TelemetryEvent>>>,
}

impl RecordingTelemetrySink {
    fn take(&self) -> Vec<TelemetryEvent> {
        self.events
            .lock()
            .unwrap_or_else(|error| panic!("events mutex should be available: {error}"))
            .drain(..)
            .collect()
    }
}

impl TelemetrySink for RecordingTelemetrySink {
    fn record(&self, event: TelemetryEvent) {
        self.events
            .lock()
            .unwrap_or_else(|error| panic!("events mutex should be available: {error}"))
            .push(event);
    }
}

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
    let temp_dir = TempDir::new().unwrap_or_else(|error| {
        panic!("failed to create temporary directory for database path: {error}");
    });
    let database_url = path_to_string(temp_dir.path());
    migration_state.temp_dir.set(temp_dir);
    migration_state.database_url.set(database_url);
}

#[given("a temporary database file")]
fn temporary_database_file(migration_state: &MigrationState) {
    let temp_dir = TempDir::new().unwrap_or_else(|error| {
        panic!("failed to create temporary directory for database file: {error}");
    });
    let db_path = temp_dir.path().join("frankie.sqlite");
    let database_url = path_to_string(&db_path);
    migration_state.temp_dir.set(temp_dir);
    migration_state.database_url.set(database_url);
}

// --- When steps ---

#[when("database migrations are run")]
fn run_migrations(migration_state: &MigrationState) {
    let telemetry = migration_state
        .telemetry
        .with_ref(Clone::clone)
        .unwrap_or_else(|| {
            let telemetry = RecordingTelemetrySink::default();
            migration_state.telemetry.set(telemetry.clone());
            telemetry
        });

    let database_url = migration_state
        .database_url
        .with_ref(Clone::clone)
        .unwrap_or_else(|| panic!("database URL not initialised"));

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

#[then("the schema version is {expected}")]
fn schema_version_is(migration_state: &MigrationState, expected: String) {
    let expected_clean = expected.trim_matches('"');

    let actual = migration_state
        .schema_version
        .with_ref(Clone::clone)
        .unwrap_or_else(|| panic!("schema version missing"));

    assert_eq!(actual, expected_clean, "schema version mismatch");
}

#[then("telemetry records the schema version")]
fn telemetry_records_schema_version(migration_state: &MigrationState) {
    let events = migration_state
        .telemetry
        .with_ref(RecordingTelemetrySink::take)
        .unwrap_or_else(|| panic!("telemetry sink not initialised"));

    let Some(TelemetryEvent::SchemaVersionRecorded { schema_version }) = events.first() else {
        panic!("expected SchemaVersionRecorded event, got {events:?}");
    };

    assert!(
        !schema_version.is_empty(),
        "schema_version should not be empty in telemetry"
    );
}

#[then("telemetry records the schema version twice")]
fn telemetry_records_schema_version_twice(migration_state: &MigrationState) {
    let events = migration_state
        .telemetry
        .with_ref(RecordingTelemetrySink::take)
        .unwrap_or_else(|| panic!("telemetry sink not initialised"));

    let count = events
        .iter()
        .filter(|event| matches!(event, TelemetryEvent::SchemaVersionRecorded { .. }))
        .count();

    assert_eq!(
        count, 2,
        "expected SchemaVersionRecorded twice, got {events:?}"
    );
}

#[then("a persistence error {expected} is reported")]
fn persistence_error_is(migration_state: &MigrationState, expected: String) {
    let expected_clean = expected.trim_matches('"');

    let error = migration_state
        .error
        .with_ref(Clone::clone)
        .unwrap_or_else(|| panic!("expected persistence error"));

    assert_eq!(error.to_string(), expected_clean);
}

#[then("a persistence error starts with {expected_prefix}")]
fn persistence_error_starts_with(migration_state: &MigrationState, expected_prefix: String) {
    let expected_clean = expected_prefix.trim_matches('"');

    let error = migration_state
        .error
        .with_ref(Clone::clone)
        .unwrap_or_else(|| panic!("expected persistence error"));

    assert!(
        error.to_string().starts_with(expected_clean),
        "expected error to start with {expected_clean:?}, got {error}"
    );
}

#[then("no telemetry is recorded")]
fn no_telemetry_is_recorded(migration_state: &MigrationState) {
    let events = migration_state
        .telemetry
        .with_ref(RecordingTelemetrySink::take)
        .unwrap_or_else(|| panic!("telemetry sink not initialised"));

    assert!(
        events.is_empty(),
        "expected no telemetry events, got {events:?}"
    );
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
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
