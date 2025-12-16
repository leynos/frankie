//! Diesel-backed migration runner for the local `SQLite` database.

use diesel::Connection;
use diesel::OptionalExtension;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use diesel::sql_query;
use diesel::sql_types::Text;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::telemetry::{TelemetryEvent, TelemetrySink};

use super::PersistenceError;

/// Embedded Diesel migrations shipped with the binary.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Initial schema version recorded by the first migration in this repository.
pub const INITIAL_SCHEMA_VERSION: &str = "20251214000000";

/// A Diesel migration version string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaVersion(String);

impl SchemaVersion {
    /// Returns the inner version string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Runs pending database migrations and records the resulting schema version
/// in telemetry.
///
/// # Errors
///
/// Returns [`PersistenceError`] when the database cannot be opened, migrations
/// fail, or the resulting schema version cannot be read.
pub fn migrate_database(
    database_url: &str,
    telemetry: &dyn TelemetrySink,
) -> Result<SchemaVersion, PersistenceError> {
    let database_url_trimmed = database_url.trim();
    if database_url_trimmed.is_empty() {
        return Err(PersistenceError::BlankDatabaseUrl);
    }

    let mut connection = SqliteConnection::establish(database_url_trimmed).map_err(|error| {
        PersistenceError::ConnectionFailed {
            message: error.to_string(),
        }
    })?;

    enable_foreign_keys(&mut connection)?;

    connection
        .run_pending_migrations(MIGRATIONS)
        .map_err(|error| PersistenceError::MigrationFailed {
            message: error.to_string(),
        })?;

    let schema_version = read_schema_version(&mut connection)?;
    telemetry.record(TelemetryEvent::SchemaVersionRecorded {
        schema_version: schema_version.as_str().to_owned(),
    });

    Ok(schema_version)
}

fn enable_foreign_keys(connection: &mut SqliteConnection) -> Result<(), PersistenceError> {
    sql_query("PRAGMA foreign_keys = ON;")
        .execute(connection)
        .map(drop)
        .map_err(|error| PersistenceError::ForeignKeysEnableFailed {
            message: error.to_string(),
        })
}

fn read_schema_version(
    connection: &mut SqliteConnection,
) -> Result<SchemaVersion, PersistenceError> {
    #[derive(Debug, QueryableByName)]
    struct Row {
        #[diesel(sql_type = Text)]
        version: String,
    }

    let result: Option<Row> =
        sql_query("SELECT version FROM __diesel_schema_migrations ORDER BY version DESC LIMIT 1;")
            .get_result(connection)
            .optional()
            .map_err(|error| PersistenceError::SchemaVersionQueryFailed {
                message: error.to_string(),
            })?;

    let Some(row) = result else {
        return Err(PersistenceError::MissingSchemaVersion);
    };

    Ok(SchemaVersion(row.version))
}

#[cfg(test)]
mod tests {
    use super::{INITIAL_SCHEMA_VERSION, migrate_database};
    use crate::telemetry::TelemetryEvent;
    use crate::telemetry::test_support::RecordingSink;

    #[test]
    fn migrate_database_records_schema_version_telemetry() {
        let telemetry = RecordingSink::default();

        let schema_version =
            migrate_database(":memory:", &telemetry).expect("migration should succeed");

        assert_eq!(schema_version.as_str(), INITIAL_SCHEMA_VERSION);
        assert_eq!(
            telemetry.take(),
            vec![TelemetryEvent::SchemaVersionRecorded {
                schema_version: INITIAL_SCHEMA_VERSION.to_owned(),
            }]
        );
    }
}
