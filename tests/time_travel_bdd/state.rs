//! Scenario state for time-travel BDD tests.

use frankie::tui::app::ReviewApp;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use std::sync::Arc;

use super::MockGitOperations;

/// State shared across steps in a time-travel scenario.
#[derive(ScenarioState, Default)]
pub(crate) struct TimeTravelTestState {
    /// The TUI application model under test.
    pub(crate) app: Slot<ReviewApp>,
    /// The rendered view output.
    pub(crate) rendered_view: Slot<String>,
    /// Mock Git operations.
    pub(crate) mock_git_ops: Slot<Arc<MockGitOperations>>,
    /// Whether commit should be found.
    pub(crate) commit_found: Slot<bool>,
    /// Whether repository should be available.
    pub(crate) repo_available: Slot<bool>,
}
