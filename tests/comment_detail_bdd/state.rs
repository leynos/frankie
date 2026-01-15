//! Scenario state for comment detail BDD tests.

use frankie::github::models::ReviewComment;
use frankie::tui::app::ReviewApp;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

// Re-export the shared builder for use in BDD tests
pub(crate) use frankie::tui::components::test_utils::ReviewCommentBuilder;

/// State shared across steps in a comment detail scenario.
#[derive(ScenarioState, Default)]
pub(crate) struct DetailState {
    /// The TUI application model under test.
    pub(crate) app: Slot<ReviewApp>,
    /// The rendered view output.
    pub(crate) rendered_view: Slot<String>,
    /// A standalone comment for direct component testing.
    pub(crate) standalone_comment: Slot<ReviewComment>,
}
