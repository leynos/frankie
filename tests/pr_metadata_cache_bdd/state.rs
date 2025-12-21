//! Scenario state and shared utilities for PR metadata cache BDD tests.

use std::cell::{Ref, RefCell};
use std::rc::Rc;

use frankie::telemetry::{TelemetryEvent, TelemetrySink};
use frankie::{IntakeError, PullRequestDetails};
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, StepArgs};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use wiremock::MockServer;

/// Shared runtime wrapper that can be stored in an rstest-bdd Slot.
#[derive(Clone)]
pub(crate) struct SharedRuntime(Rc<RefCell<Runtime>>);

impl SharedRuntime {
    pub(crate) fn new(runtime: Runtime) -> Self {
        Self(Rc::new(RefCell::new(runtime)))
    }

    pub(crate) fn borrow(&self) -> Ref<'_, Runtime> {
        self.0.borrow()
    }

    pub(crate) fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.0.borrow().block_on(future)
    }
}

#[derive(Debug, Default)]
pub(crate) struct NoopTelemetry;

impl TelemetrySink for NoopTelemetry {
    fn record(&self, _event: TelemetryEvent) {}
}

#[derive(Clone, Debug, StepArgs)]
pub(crate) struct MockInvalidationConfig {
    pub(crate) pr: u64,
    pub(crate) old_title: String,
    pub(crate) new_title: String,
    pub(crate) etag1: String,
    pub(crate) etag2: String,
    pub(crate) count: u64,
}

impl MockInvalidationConfig {
    pub(crate) fn new(
        pr: u64,
        titles: (String, String),
        etag_values: (String, String),
        count: u64,
    ) -> Self {
        let (old_title, new_title) = titles;
        let (etag_one, etag_two) = etag_values;
        Self {
            pr,
            old_title,
            new_title,
            etag1: etag_one.trim_matches('"').to_owned(),
            etag2: etag_two.trim_matches('"').to_owned(),
            count,
        }
    }
}

#[derive(Clone, Debug, StepArgs)]
pub(crate) struct MockRevalidationConfig {
    pub(crate) pr: u64,
    pub(crate) title: String,
    pub(crate) etag: String,
    pub(crate) last_modified: String,
    pub(crate) count: u64,
}

impl MockRevalidationConfig {
    pub(crate) fn new(pr: u64, title: String, validators: (String, String), count: u64) -> Self {
        let (etag, last_modified) = validators;
        Self {
            pr,
            title,
            etag: etag.trim_matches('"').to_owned(),
            last_modified: last_modified.trim_matches('"').to_owned(),
            count,
        }
    }
}

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

pub(crate) fn ensure_runtime_and_server(state: &CacheState) -> SharedRuntime {
    if state.runtime.with_ref(|_| ()).is_none() {
        let runtime = Runtime::new()
            .unwrap_or_else(|error| panic!("failed to create Tokio runtime: {error}"));
        state.runtime.set(SharedRuntime::new(runtime));
    }

    let shared_runtime = state
        .runtime
        .get()
        .unwrap_or_else(|| panic!("runtime not initialised after set"));

    if state.server.with_ref(|_| ()).is_none() {
        state
            .server
            .set(shared_runtime.block_on(MockServer::start()));
    }

    shared_runtime
}
