//! Database migration operations.

use frankie::persistence::{PersistenceError, migrate_database};
use frankie::telemetry::StderrJsonlTelemetrySink;
use frankie::{FrankieConfig, IntakeError};

/// Runs database migrations.
///
/// # Errors
///
/// Returns [`IntakeError::Configuration`] if the database URL is missing or blank.
/// Returns [`IntakeError::Io`] for connection or migration failures.
pub fn run(config: &FrankieConfig) -> Result<(), IntakeError> {
    let database_url =
        config
            .database_url
            .as_deref()
            .ok_or_else(|| IntakeError::Configuration {
                message: PersistenceError::MissingDatabaseUrl.to_string(),
            })?;

    let telemetry = StderrJsonlTelemetrySink;
    migrate_database(database_url, &telemetry)
        .map(drop)
        .map_err(|error| map_persistence_error(&error))
}

/// Maps a persistence error to an intake error.
///
/// Configuration-related errors (blank URL) become [`IntakeError::Configuration`],
/// while runtime errors (connection, migration, query failures) become
/// [`IntakeError::Io`].
fn map_persistence_error(error: &PersistenceError) -> IntakeError {
    if is_configuration_error(error) {
        IntakeError::Configuration {
            message: error.to_string(),
        }
    } else {
        IntakeError::Io {
            message: error.to_string(),
        }
    }
}

/// Returns true if the persistence error is a configuration problem.
const fn is_configuration_error(error: &PersistenceError) -> bool {
    matches!(error, PersistenceError::BlankDatabaseUrl)
}

#[cfg(test)]
mod tests {
    use frankie::persistence::PersistenceError;
    use frankie::{FrankieConfig, IntakeError};
    use rstest::rstest;

    use super::{is_configuration_error, map_persistence_error, run};

    #[test]
    fn persistence_error_classification_distinguishes_missing_from_blank() {
        assert!(
            !is_configuration_error(&PersistenceError::MissingDatabaseUrl),
            "MissingDatabaseUrl is handled before persistence runs"
        );
        assert!(
            is_configuration_error(&PersistenceError::BlankDatabaseUrl),
            "BlankDatabaseUrl is a configuration issue"
        );

        assert!(
            matches!(
                map_persistence_error(&PersistenceError::MissingDatabaseUrl),
                IntakeError::Io { .. }
            ),
            "MissingDatabaseUrl should not be treated as a persistence configuration error"
        );
        assert!(
            matches!(
                map_persistence_error(&PersistenceError::BlankDatabaseUrl),
                IntakeError::Configuration { .. }
            ),
            "BlankDatabaseUrl should map to IntakeError::Configuration"
        );
    }

    #[rstest]
    #[case::missing_database_url(None, "database URL is required")]
    #[case::blank_database_url(Some("   ".to_owned()), "database URL must not be blank")]
    fn migrate_db_rejects_invalid_database_url(
        #[case] database_url: Option<String>,
        #[case] expected_message_prefix: &str,
    ) {
        let config = FrankieConfig {
            database_url,
            migrate_db: true,
            ..Default::default()
        };

        let result = run(&config);

        match result {
            Err(IntakeError::Configuration { message }) => {
                assert!(
                    message.starts_with(expected_message_prefix),
                    "expected message starting with {expected_message_prefix:?}, got {message:?}"
                );
            }
            other => panic!("expected Configuration error, got {other:?}"),
        }
    }
}
