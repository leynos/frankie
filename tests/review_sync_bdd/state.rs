//! Scenario state for review sync BDD tests.

use std::sync::Arc;

use frankie::github::models::ReviewComment;
use frankie::telemetry::test_support::RecordingTelemetrySink;
use frankie::tui::app::ReviewApp;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;

/// State shared across steps in a review sync scenario.
#[derive(ScenarioState, Default)]
pub(crate) struct SyncState {
    /// The TUI application model under test.
    pub(crate) app: Slot<ReviewApp>,
    /// Recording telemetry sink for capturing events.
    pub(crate) telemetry_sink: Slot<Arc<RecordingTelemetrySink>>,
    /// Last recorded latency in milliseconds.
    pub(crate) last_latency_ms: Slot<u64>,
}

/// Creates a review comment with the given ID.
pub(crate) fn review_with_id(id: u64) -> ReviewComment {
    ReviewComment {
        id,
        body: Some(format!("Comment {id}")),
        author: Some("alice".to_owned()),
        file_path: None,
        line_number: None,
        original_line_number: None,
        diff_hunk: None,
        commit_sha: None,
        in_reply_to_id: None,
        created_at: None,
        updated_at: None,
    }
}

/// Creates a vector of review comments with sequential IDs starting from 1.
pub(crate) fn create_reviews(count: usize) -> Vec<ReviewComment> {
    (1..=count).map(|i| review_with_id(i as u64)).collect()
}
