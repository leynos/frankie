//! Scenario state and shared utilities for PR metadata cache BDD tests.

use frankie::{IntakeError, PullRequestDetails};
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use tempfile::TempDir;
use wiremock::MockServer;

pub(crate) use crate::pr_metadata_cache_helpers::{MockInvalidationConfig, MockRevalidationConfig};
pub(crate) use crate::runtime::SharedRuntime;

#[derive(ScenarioState, Default)]
pub(crate) struct CacheState {
    pub(crate) runtime: Slot<SharedRuntime>,
    pub(crate) server: Slot<MockServer>,
    pub(crate) token: Slot<String>,
    pub(crate) database_url: Slot<String>,
    pub(crate) temp_dir: Slot<TempDir>,
    pub(crate) ttl_seconds: Slot<u64>,
    pub(crate) expected_metadata_path: Slot<String>,
    pub(crate) details: Slot<PullRequestDetails>,
    pub(crate) error: Slot<IntakeError>,
}
