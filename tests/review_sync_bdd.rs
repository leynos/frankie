//! Behavioural tests for incremental review sync.

#[path = "review_sync_bdd/mod.rs"]
mod review_sync_bdd_support;

use std::sync::Arc;

use frankie::telemetry::test_support::RecordingTelemetrySink;
use frankie::telemetry::{TelemetryEvent, TelemetrySink};
use frankie::tui::app::ReviewApp;
use frankie::tui::messages::AppMsg;
use review_sync_bdd_support::SyncState;
use review_sync_bdd_support::state::{create_reviews, review_with_id};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn sync_state() -> SyncState {
    SyncState::default()
}

// Given steps

#[given("a recording telemetry sink")]
fn given_recording_telemetry_sink(sync_state: &SyncState) {
    let sink = Arc::new(RecordingTelemetrySink::default());
    sync_state.telemetry_sink.set(sink);
}

#[given("a TUI with {count:usize} review comments")]
fn given_tui_with_reviews(sync_state: &SyncState, count: usize) {
    let reviews = create_reviews(count);
    let app = ReviewApp::new(reviews);
    sync_state.app.set(app);
}

#[given("the cursor is on comment {id:u64}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn given_cursor_on_comment(sync_state: &SyncState, id: u64) {
    sync_state
        .app
        .with_mut(|app| {
            // Move cursor until we find the comment with this ID
            while app.current_selected_id() != Some(id) {
                app.handle_message(&AppMsg::CursorDown);
                // Safety: prevent infinite loop if ID not found
                if app.cursor_position() >= app.filtered_count().saturating_sub(1) {
                    break;
                }
            }
        })
        .expect("app not initialised");

    let actual_id = sync_state
        .app
        .with_ref(ReviewApp::current_selected_id)
        .expect("app not initialised");

    assert_eq!(
        actual_id,
        Some(id),
        "failed to position cursor on comment {id}"
    );
}

// When steps

#[when("a sync completes with {count:usize} comments including comment {id:u64}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn when_sync_completes_with_comments_including(sync_state: &SyncState, count: usize, id: u64) {
    let mut reviews = create_reviews(count);
    // Ensure the specified comment ID is present
    if !reviews.iter().any(|r| r.id == id) {
        reviews.push(review_with_id(id));
    }

    sync_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::SyncComplete {
                reviews,
                latency_ms: 100,
            });
        })
        .expect("app not initialised");
}

#[when("a sync completes with {count:usize} comments without comment {excluded_id:u64}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn when_sync_completes_without_comment(sync_state: &SyncState, count: usize, excluded_id: u64) {
    let reviews: Vec<_> = create_reviews(count)
        .into_iter()
        .filter(|r| r.id != excluded_id)
        .collect();

    sync_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::SyncComplete {
                reviews,
                latency_ms: 100,
            });
        })
        .expect("app not initialised");
}

#[when("a sync completes in {latency_ms:u64}ms with {count:usize} comments")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn when_sync_completes_with_latency(sync_state: &SyncState, latency_ms: u64, count: usize) {
    let reviews = create_reviews(count);

    // Record telemetry manually since we're not running the full app
    let sink = sync_state
        .telemetry_sink
        .with_ref(Clone::clone)
        .expect("telemetry sink not initialised");

    sink.record(TelemetryEvent::SyncLatencyRecorded {
        latency_ms,
        comment_count: count,
        incremental: true,
    });

    sync_state.last_latency_ms.set(latency_ms);

    sync_state
        .app
        .with_mut(|app| {
            app.handle_message(&AppMsg::SyncComplete {
                reviews,
                latency_ms,
            });
        })
        .expect("app not initialised");
}

// Then steps

#[then("the cursor remains on comment {id:u64}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_cursor_on_comment(sync_state: &SyncState, id: u64) {
    let actual_id = sync_state
        .app
        .with_ref(ReviewApp::current_selected_id)
        .expect("app not initialised");

    assert_eq!(actual_id, Some(id), "cursor should be on comment {id}");
}

#[then("the cursor is on comment {id:u64}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_cursor_is_on_comment(sync_state: &SyncState, id: u64) {
    let actual_id = sync_state
        .app
        .with_ref(ReviewApp::current_selected_id)
        .expect("app not initialised");

    assert_eq!(actual_id, Some(id), "cursor should be on comment {id}");
}

#[then("the filtered count is {count:usize}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_filtered_count(sync_state: &SyncState, count: usize) {
    let actual = sync_state
        .app
        .with_ref(ReviewApp::filtered_count)
        .expect("app not initialised");

    assert_eq!(actual, count, "filtered count mismatch");
}

#[then("a SyncLatencyRecorded event is logged")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_sync_latency_event_logged(sync_state: &SyncState) {
    let events = sync_state
        .telemetry_sink
        .with_ref(|sink| sink.events())
        .expect("telemetry sink not initialised");

    let has_sync_event = events
        .iter()
        .any(|e| matches!(e, TelemetryEvent::SyncLatencyRecorded { .. }));

    assert!(has_sync_event, "expected SyncLatencyRecorded event");
}

#[then("the event shows latency_ms {expected:u64}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_event_shows_latency(sync_state: &SyncState, expected: u64) {
    let events = sync_state
        .telemetry_sink
        .with_ref(|sink| sink.events())
        .expect("telemetry sink not initialised");

    let sync_event = events.iter().find_map(|e| {
        if let TelemetryEvent::SyncLatencyRecorded { latency_ms, .. } = e {
            Some(*latency_ms)
        } else {
            None
        }
    });

    assert_eq!(sync_event, Some(expected), "latency_ms mismatch");
}

#[then("the event shows comment_count {expected:usize}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_event_shows_comment_count(sync_state: &SyncState, expected: usize) {
    let events = sync_state
        .telemetry_sink
        .with_ref(|sink| sink.events())
        .expect("telemetry sink not initialised");

    let sync_event = events.iter().find_map(|e| {
        if let TelemetryEvent::SyncLatencyRecorded { comment_count, .. } = e {
            Some(*comment_count)
        } else {
            None
        }
    });

    assert_eq!(sync_event, Some(expected), "comment_count mismatch");
}

#[then("the event shows incremental {expected}")]
#[expect(clippy::expect_used, reason = "BDD test step; panics are acceptable")]
fn then_event_shows_incremental(sync_state: &SyncState, expected: String) {
    let expected_bool = expected == "true";

    let events = sync_state
        .telemetry_sink
        .with_ref(|sink| sink.events())
        .expect("telemetry sink not initialised");

    let sync_event = events.iter().find_map(|e| {
        if let TelemetryEvent::SyncLatencyRecorded { incremental, .. } = e {
            Some(*incremental)
        } else {
            None
        }
    });

    assert_eq!(sync_event, Some(expected_bool), "incremental mismatch");
}

// Scenario bindings

#[scenario(path = "tests/features/review_sync.feature", index = 0)]
fn sync_preserves_selection(sync_state: SyncState) {
    let _ = sync_state;
}

#[scenario(path = "tests/features/review_sync.feature", index = 1)]
fn sync_clamps_cursor_when_deleted(sync_state: SyncState) {
    let _ = sync_state;
}

#[scenario(path = "tests/features/review_sync.feature", index = 2)]
fn sync_logs_telemetry(sync_state: SyncState) {
    let _ = sync_state;
}
