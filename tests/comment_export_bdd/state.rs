//! Scenario state and runtime/server initialization for the comment export
//! BDD tests.

use frankie::IntakeError;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use wiremock::MockServer;

use super::runtime::SharedRuntime;

/// Scenario state for comment export tests.
#[derive(ScenarioState, Default)]
pub(crate) struct ExportState {
    pub(crate) runtime: Slot<SharedRuntime>,
    pub(crate) server: Slot<MockServer>,
    pub(crate) token: Slot<String>,
    pub(crate) output: Slot<String>,
    pub(crate) error: Slot<IntakeError>,
}

/// Ensures the runtime and server are initialized in `ExportState`.
pub(crate) fn ensure_runtime_and_server(
    export_state: &ExportState,
) -> Result<SharedRuntime, IntakeError> {
    super::runtime::ensure_runtime_and_server(&export_state.runtime, &export_state.server).map_err(
        |error| IntakeError::Api {
            message: format!("failed to create Tokio runtime: {error}"),
        },
    )
}
