//! Scenario state and runtime/server initialisation for the template export
//! BDD tests.

use frankie::IntakeError;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use wiremock::MockServer;

use super::runtime::SharedRuntime;

/// Scenario state for template export tests.
#[derive(ScenarioState, Default)]
pub(crate) struct TemplateExportState {
    /// Shared Tokio runtime for async operations.
    pub(crate) runtime: Slot<SharedRuntime>,
    /// Mock GitHub API server.
    pub(crate) server: Slot<MockServer>,
    /// Personal access token for authentication.
    pub(crate) token: Slot<String>,
    /// Template content for rendering.
    pub(crate) template: Slot<String>,
    /// Output from template rendering.
    pub(crate) output: Slot<String>,
    /// Error from template rendering.
    pub(crate) error: Slot<IntakeError>,
}

/// Ensures the runtime and server are initialised in `TemplateExportState`.
pub(crate) fn ensure_runtime_and_server(
    state: &TemplateExportState,
) -> Result<SharedRuntime, IntakeError> {
    super::runtime::ensure_runtime_and_server(&state.runtime, &state.server).map_err(|error| {
        IntakeError::Api {
            message: format!("failed to create Tokio runtime: {error}"),
        }
    })
}
