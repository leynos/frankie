//! Performance check for full-screen diff context rendering.

use std::time::Instant;

use bubbletea_rs::Model;
use frankie::github::models::ReviewComment;
use frankie::tui::app::ReviewApp;
use frankie::tui::messages::AppMsg;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FixtureReviewComment {
    id: u64,
    body: Option<String>,
    author: Option<String>,
    file_path: Option<String>,
    line_number: Option<u32>,
    original_line_number: Option<u32>,
    diff_hunk: Option<String>,
    commit_sha: Option<String>,
    in_reply_to_id: Option<u64>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

impl From<FixtureReviewComment> for ReviewComment {
    fn from(value: FixtureReviewComment) -> Self {
        Self {
            id: value.id,
            body: value.body,
            author: value.author,
            file_path: value.file_path,
            line_number: value.line_number,
            original_line_number: value.original_line_number,
            diff_hunk: value.diff_hunk,
            commit_sha: value.commit_sha,
            in_reply_to_id: value.in_reply_to_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[test]
#[ignore = "manual profiling check"]
fn diff_context_render_perf() {
    let raw = include_str!("fixtures/diff_context_reference.json");
    let fixture: Vec<FixtureReviewComment> =
        serde_json::from_str(raw).expect("fixture data should be valid JSON");
    let reviews: Vec<ReviewComment> = fixture.into_iter().map(ReviewComment::from).collect();

    let mut app = ReviewApp::new(reviews);
    app.handle_message(&AppMsg::ShowDiffContext);

    let start = Instant::now();
    let _view = app.view();
    let elapsed_ms = start.elapsed().as_millis();

    assert!(
        elapsed_ms < 100,
        "expected diff context render under 100ms, got {elapsed_ms}ms"
    );
}
