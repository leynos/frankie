//! Local persistence and database migrations.
//!
//! Frankie uses a local sqlite database for local-first caching and
//! persistence. The schema is
//! managed with Diesel migrations so the database can be created and upgraded
//! consistently across machines.

mod error;
mod migrator;
mod pr_metadata_cache;

pub use error::PersistenceError;
pub use migrator::{
    CURRENT_SCHEMA_VERSION, INITIAL_SCHEMA_VERSION, SchemaVersion, migrate_database,
};
pub use pr_metadata_cache::{
    CachedPullRequestMetadata, PullRequestMetadataCache, PullRequestMetadataCacheWrite,
};
