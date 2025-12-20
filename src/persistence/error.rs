//! Error types for local persistence operations.

use thiserror::Error;

/// Errors returned while initialising or migrating the local sqlite database.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PersistenceError {
    /// No database URL/path was provided.
    #[error("database URL is required (use --database-url or FRANKIE_DATABASE_URL)")]
    MissingDatabaseUrl,

    /// The database URL/path was present but blank.
    #[error("database URL must not be blank")]
    BlankDatabaseUrl,

    /// Establishing a sqlite connection failed.
    #[error("failed to connect to SQLite database: {message}")]
    ConnectionFailed {
        /// Error detail from Diesel.
        message: String,
    },

    /// Running pending migrations failed.
    #[error("failed to run database migrations: {message}")]
    MigrationFailed {
        /// Error detail from Diesel migrations.
        message: String,
    },

    /// Enabling foreign key enforcement failed.
    #[error("failed to enable foreign keys: {message}")]
    ForeignKeysEnableFailed {
        /// Error detail from the PRAGMA execution.
        message: String,
    },

    /// Reading the schema version from the migration table failed.
    #[error("failed to read schema version after migrations: {message}")]
    SchemaVersionQueryFailed {
        /// Error detail from Diesel query execution.
        message: String,
    },

    /// The migrations completed but no schema version could be found.
    #[error("no schema version recorded after migrations ran")]
    MissingSchemaVersion,

    /// The database is present but does not contain the required schema.
    ///
    /// This typically happens when `--database-url` points at a fresh file and
    /// migrations have not been applied yet.
    #[error(
        "database schema is missing required tables; run `frankie --migrate-db --database-url <PATH>`"
    )]
    SchemaNotInitialised,

    /// Querying the database failed.
    #[error("database query failed: {message}")]
    QueryFailed {
        /// Error detail from Diesel.
        message: String,
    },

    /// Writing to the database failed.
    #[error("database write failed: {message}")]
    WriteFailed {
        /// Error detail from Diesel.
        message: String,
    },
}
