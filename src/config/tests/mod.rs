//! Unit tests for configuration loading and precedence.
//!
//! Tests are organised into modules by functional area:
//! - `helpers`: Shared test utilities
//! - `precedence`: Layer precedence tests
//! - `operation_mode`: Operation mode determination tests
//! - `field_resolution`: Token, PR URL, and repository info resolution tests
//! - `ttl_loading`: `pr_metadata_cache_ttl_seconds` loading tests
//! - `local_discovery_config`: `no_local_discovery` configuration tests
//! - `validation`: Configuration consistency validation tests

mod field_resolution;
mod helpers;
mod local_discovery_config;
mod operation_mode;
mod precedence;
mod reply_drafting;
mod ttl_loading;
mod validation;
