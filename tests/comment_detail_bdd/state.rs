//! Scenario state for comment detail BDD tests.

use frankie::github::models::ReviewComment;
use frankie::tui::app::ReviewApp;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

/// State shared across steps in a comment detail scenario.
#[derive(ScenarioState, Default)]
pub(crate) struct DetailState {
    /// The TUI application model under test.
    pub(crate) app: Slot<ReviewApp>,
    /// The rendered view output.
    pub(crate) rendered_view: Slot<String>,
    /// Maximum width used for rendering.
    pub(crate) max_width: Slot<usize>,
}

/// Creates a review comment with the specified fields.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "test helper function with many optional fields"
)]
pub(crate) fn create_review_comment(
    id: u64,
    author: Option<&str>,
    file_path: Option<&str>,
    line_number: Option<u32>,
    body: Option<&str>,
    diff_hunk: Option<&str>,
) -> ReviewComment {
    ReviewComment {
        id,
        body: body.map(ToOwned::to_owned),
        author: author.map(ToOwned::to_owned),
        file_path: file_path.map(ToOwned::to_owned),
        line_number,
        original_line_number: None,
        diff_hunk: diff_hunk.map(ToOwned::to_owned),
        commit_sha: None,
        in_reply_to_id: None,
        created_at: None,
        updated_at: None,
    }
}
