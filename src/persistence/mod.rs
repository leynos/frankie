//! Local persistence and database migrations.
//!
//! Frankie uses `SQLite` for local-first caching and persistence. The schema is
//! managed with Diesel migrations so the database can be created and upgraded
//! consistently across machines.

mod error;
mod migrator;

pub use error::PersistenceError;
pub use migrator::{INITIAL_SCHEMA_VERSION, SchemaVersion, migrate_database};
