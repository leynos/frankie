//! Scenario state for full-screen diff context BDD tests.

use frankie::tui::app::ReviewApp;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

pub(crate) use frankie::tui::components::test_utils::ReviewCommentBuilder;

/// State shared across steps in a diff context scenario.
#[derive(ScenarioState, Default)]
pub(crate) struct DiffContextState {
    /// The TUI application model under test.
    pub(crate) app: Slot<ReviewApp>,
    /// The rendered view output.
    pub(crate) rendered_view: Slot<String>,
}
